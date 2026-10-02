//! Caller-runtime scheduling primitives for the future OTLP wire adapter.
//!
//! The pinned official SDK can construct span events and links through its
//! public surface, but cannot construct the pre-aggregated metric requests
//! required by this contract. The approved adapter therefore projects all
//! signals to OTLP protobuf requests. These primitives deliberately contain
//! no SDK-specific record construction, so either transport retains the
//! caller-runtime, admission, and resource-grouping invariants. The validated
//! protocol selects the terminal send: `Grpc` uses the tonic clients and
//! `HttpBinary` posts the same protobuf requests over OTLP/HTTP.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "D.18 composes these crate-private scheduling primitives after the transport decision"
    )
)]

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use super::retry::{RetryDecision, retry_export_bounded};
#[cfg(test)]
pub(super) use super::retry::{http_retry_action, retry_action, retry_export};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use tokio::runtime::Handle;
use tokio::sync::{Mutex, watch};
use tonic::Request;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint};

use crate::config::{OtlpProtocol, ValidatedBackendConnection, ValidatedTransportBounds};
use crate::contracts::{
    CompleteSpan, ExportRecord, ExporterLifecycle, ExporterSet, InstrumentationScope,
    LifecycleFuture, LogExporter, LogRecord, MetricExporter, Resource, TraceExporter,
};
use crate::lifecycle::{Admitted, LifecycleCore, LifecycleState, Signal};
use sc_observability_types::otlp::group_records_by_resource_and_scope;
use sc_observability_types::v2::{
    AggregationTemporality, AttributeValue, Attributes, ExportError, MetricRecord, MetricValue,
    SpanKind, SpanStatus, TelemetryError,
};
use sc_observability_types::{ErrorContext, Remediation};

use opentelemetry_proto::tonic::{
    collector::{
        logs::v1::{ExportLogsServiceRequest, logs_service_client::LogsServiceClient},
        metrics::v1::{ExportMetricsServiceRequest, metrics_service_client::MetricsServiceClient},
        trace::v1::{ExportTraceServiceRequest, trace_service_client::TraceServiceClient},
    },
    common::v1 as proto_common,
    logs::v1 as proto_logs,
    metrics::v1 as proto_metrics,
    resource::v1 as proto_resource,
    trace::v1 as proto_trace,
};

/// Caller-owned runtime used for asynchronous OTLP export futures.
#[derive(Clone)]
pub(crate) struct CallerRuntime {
    handle: Handle,
}

impl CallerRuntime {
    /// Captures the runtime already entered by the embedding application.
    ///
    /// A missing runtime is a typed construction failure for an enabled async
    /// backend: this module never creates one or blocks the caller thread.
    #[must_use]
    pub(crate) fn try_capture() -> Option<Self> {
        Handle::try_current().ok().map(|handle| Self { handle })
    }

    /// Runs a previously projected export future on the caller's runtime.
    ///
    /// The D.6 admission permit moves into the task. Its `complete` method is
    /// the success/failure terminal path; dropping it records cancellation or
    /// runtime teardown without a separate accounting counter.
    pub(crate) fn spawn_export<T, F>(
        &self,
        admitted: Admitted<T>,
        export: F,
    ) -> tokio::task::JoinHandle<()>
    where
        T: Send + 'static,
        F: Future<Output = Result<(), ExportError>> + Send + 'static,
    {
        self.handle.spawn(async move {
            let _value = admitted.complete(export.await);
        })
    }
}

/// The one backend hand-off D.18 consumes: a concrete exporter set and the
/// same D.6 core held by every signal adapter. Keeping both together prevents
/// a second admission domain from being created by composition code.
pub(crate) struct SdkAdapterSet {
    pub(crate) exporters: ExporterSet,
    pub(crate) lifecycle: LifecycleCore,
}

/// Builds the lossless protobuf SDK transport for the validated protocol on
/// the caller's Tokio runtime. Connection values are already validated; this
/// function never consults `OTEL_*` defaults and never creates a runtime.
pub(crate) fn build_exporter_set(
    connection: &ValidatedBackendConnection,
    bounds: &ValidatedTransportBounds,
) -> Result<SdkAdapterSet, ExportError> {
    let runtime = CallerRuntime::try_capture().ok_or_else(runtime_required_error)?;
    let (shutdown_cancel, shutdown) = watch::channel(false);
    let terminal = Arc::new(SdkTerminal::new(connection, bounds, shutdown)?);
    let lifecycle = LifecycleCore::from_backend(terminal.clone(), bounds)?;
    let backend = Arc::new(SdkBackend {
        runtime,
        lifecycle: lifecycle.clone(),
        terminal,
    });

    Ok(SdkAdapterSet {
        exporters: ExporterSet {
            logs: Arc::new(SdkLogExporter {
                backend: Arc::clone(&backend),
            }),
            traces: Arc::new(SdkTraceExporter {
                backend: Arc::clone(&backend),
            }),
            metrics: Arc::new(SdkMetricExporter { backend }),
            lifecycle: Arc::new(SdkLifecycle {
                lifecycle: lifecycle.clone(),
                shutdown_cancel,
            }),
        },
        lifecycle,
    })
}

struct SdkBackend {
    runtime: CallerRuntime,
    lifecycle: LifecycleCore,
    terminal: Arc<SdkTerminal>,
}

/// Terminal protocol clients. They are intentionally below the D.6 core: by
/// the time the core invokes their lifecycle methods, every admitted send has
/// reached a typed terminal outcome, so there is no recursive flush path.
pub(super) struct SdkTerminal {
    transport: SdkTransport,
    retry_deadline: Duration,
    request_timeout: Duration,
    shutdown: watch::Receiver<bool>,
}

/// The validated OTLP protocol's wire dispatch. Both variants send the same
/// projected protobuf requests under the same admission, lifecycle, retry
/// and deadline policy; only the terminal send differs.
enum SdkTransport {
    Grpc(Box<GrpcTransport>),
    HttpProtobuf(HttpProtobufTransport),
}

struct GrpcTransport {
    logs: Mutex<LogsServiceClient<Channel>>,
    traces: Mutex<TraceServiceClient<Channel>>,
    metrics: Mutex<MetricsServiceClient<Channel>>,
    auth_header: Option<MetadataValue<tonic::metadata::Ascii>>,
}

/// OTLP/HTTP with protobuf bodies.
///
/// The pinned official `opentelemetry-otlp` HTTP exporters accept only SDK
/// data types, and the pinned SDK keeps the pre-aggregated metric and log
/// record constructors crate-private, so they cannot carry the projected
/// requests losslessly. This transport therefore posts the same
/// `opentelemetry-proto` requests the gRPC path sends, encoded with `prost`,
/// through the pinned `reqwest` client (which owns HTTP and TLS).
struct HttpProtobufTransport {
    client: reqwest::Client,
    logs_url: String,
    traces_url: String,
    metrics_url: String,
    auth_header: Option<HeaderValue>,
}

impl SdkTerminal {
    fn new(
        connection: &ValidatedBackendConnection,
        bounds: &ValidatedTransportBounds,
        shutdown: watch::Receiver<bool>,
    ) -> Result<Self, ExportError> {
        if connection.ca_file().is_some() {
            return Err(transport_error(
                "custom OTLP CA files are not implemented for the SDK transport",
            ));
        }
        let transport = match bounds.protocol() {
            OtlpProtocol::Grpc => {
                SdkTransport::Grpc(Box::new(GrpcTransport::new(connection, bounds)?))
            }
            OtlpProtocol::HttpBinary => {
                SdkTransport::HttpProtobuf(HttpProtobufTransport::new(connection, bounds)?)
            }
            OtlpProtocol::HttpJson => {
                return Err(transport_error(
                    "the SDK transport supports only the Grpc and HttpBinary OTLP protocols",
                ));
            }
        };
        Ok(Self {
            transport,
            retry_deadline: bounds.lifecycle().shutdown().get(),
            request_timeout: bounds.request_timeout().get(),
            shutdown,
        })
    }

    async fn export_logs(
        &self,
        resource_logs: Vec<proto_logs::ResourceLogs>,
    ) -> Result<(), ExportError> {
        let request = ExportLogsServiceRequest { resource_logs };
        let message = "OTLP log export failed";
        match &self.transport {
            SdkTransport::Grpc(grpc) => {
                // Clone under the guard and release it before any RPC or retry
                // sleep, so concurrent exports of one signal never serialize.
                let client = grpc.logs.lock().await.clone();
                retry_export_bounded(
                    self.retry_deadline,
                    self.request_timeout,
                    self.shutdown.clone(),
                    |timeout| {
                        let mut request = grpc.request(request.clone());
                        request.set_timeout(timeout);
                        let mut client = client.clone();
                        async move { client.export(request).await.map(|_| ()) }
                    },
                    message,
                )
                .await
            }
            SdkTransport::HttpProtobuf(http) => {
                http.export(
                    self.retry_deadline,
                    self.request_timeout,
                    self.shutdown.clone(),
                    &http.logs_url,
                    &request,
                    message,
                )
                .await
            }
        }
    }

    async fn export_spans(
        &self,
        resource_spans: Vec<proto_trace::ResourceSpans>,
    ) -> Result<(), ExportError> {
        let request = ExportTraceServiceRequest { resource_spans };
        let message = "OTLP trace export failed";
        match &self.transport {
            SdkTransport::Grpc(grpc) => {
                // Clone under the guard and release it before any RPC or retry
                // sleep, so concurrent exports of one signal never serialize.
                let client = grpc.traces.lock().await.clone();
                retry_export_bounded(
                    self.retry_deadline,
                    self.request_timeout,
                    self.shutdown.clone(),
                    |timeout| {
                        let mut request = grpc.request(request.clone());
                        request.set_timeout(timeout);
                        let mut client = client.clone();
                        async move { client.export(request).await.map(|_| ()) }
                    },
                    message,
                )
                .await
            }
            SdkTransport::HttpProtobuf(http) => {
                http.export(
                    self.retry_deadline,
                    self.request_timeout,
                    self.shutdown.clone(),
                    &http.traces_url,
                    &request,
                    message,
                )
                .await
            }
        }
    }

    async fn export_metrics(
        &self,
        resource_metrics: Vec<proto_metrics::ResourceMetrics>,
    ) -> Result<(), ExportError> {
        let request = ExportMetricsServiceRequest { resource_metrics };
        let message = "OTLP metric export failed";
        match &self.transport {
            SdkTransport::Grpc(grpc) => {
                // Clone under the guard and release it before any RPC or retry
                // sleep, so concurrent exports of one signal never serialize.
                let client = grpc.metrics.lock().await.clone();
                retry_export_bounded(
                    self.retry_deadline,
                    self.request_timeout,
                    self.shutdown.clone(),
                    |timeout| {
                        let mut request = grpc.request(request.clone());
                        request.set_timeout(timeout);
                        let mut client = client.clone();
                        async move { client.export(request).await.map(|_| ()) }
                    },
                    message,
                )
                .await
            }
            SdkTransport::HttpProtobuf(http) => {
                http.export(
                    self.retry_deadline,
                    self.request_timeout,
                    self.shutdown.clone(),
                    &http.metrics_url,
                    &request,
                    message,
                )
                .await
            }
        }
    }
}

impl GrpcTransport {
    fn new(
        connection: &ValidatedBackendConnection,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, ExportError> {
        let endpoint = Endpoint::from_shared(connection.endpoint().as_str().to_owned())
            .map_err(|error| {
                transport_error_from(
                    "validated OTLP endpoint is not a usable gRPC URI",
                    Box::new(error),
                )
            })?
            .timeout(bounds.request_timeout().get());
        let channel = endpoint.connect_lazy();
        let auth_header = connection
            .auth_header()
            .map(|header| {
                header
                    .as_str()
                    .parse::<MetadataValue<tonic::metadata::Ascii>>()
            })
            .transpose()
            .map_err(|error| {
                transport_error_from(
                    "OTLP authorization header is not valid gRPC metadata",
                    Box::new(error),
                )
            })?;
        Ok(Self {
            logs: Mutex::new(LogsServiceClient::new(channel.clone())),
            traces: Mutex::new(TraceServiceClient::new(channel.clone())),
            metrics: Mutex::new(MetricsServiceClient::new(channel)),
            auth_header,
        })
    }

    fn request<T>(&self, message: T) -> Request<T> {
        let mut request = Request::new(message);
        if let Some(header) = &self.auth_header {
            request
                .metadata_mut()
                .insert("authorization", header.clone());
        }
        request
    }
}

impl HttpProtobufTransport {
    fn new(
        connection: &ValidatedBackendConnection,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, ExportError> {
        let client = reqwest::Client::builder()
            .timeout(bounds.request_timeout().get())
            .build()
            .map_err(|error| {
                transport_error_from("OTLP/HTTP client could not be constructed", Box::new(error))
            })?;
        let auth_header = connection
            .auth_header()
            .map(|header| HeaderValue::from_str(header.as_str()))
            .transpose()
            .map_err(|error| {
                transport_error_from(
                    "OTLP authorization header is not a valid HTTP header",
                    Box::new(error),
                )
            })?;
        let endpoint = connection.endpoint().as_str();
        Ok(Self {
            client,
            logs_url: signal_url(endpoint, "logs"),
            traces_url: signal_url(endpoint, "traces"),
            metrics_url: signal_url(endpoint, "metrics"),
            auth_header,
        })
    }

    async fn export<M: prost::Message>(
        &self,
        deadline: Duration,
        request_timeout: Duration,
        shutdown: watch::Receiver<bool>,
        url: &str,
        request: &M,
        message: &'static str,
    ) -> Result<(), ExportError> {
        let body = request.encode_to_vec();
        retry_export_bounded(
            deadline,
            request_timeout,
            shutdown,
            |timeout| {
                let mut send = self
                    .client
                    .post(url)
                    .timeout(timeout)
                    .header(CONTENT_TYPE, PROTOBUF_CONTENT_TYPE)
                    .body(body.clone());
                if let Some(header) = &self.auth_header {
                    send = send.header(AUTHORIZATION, header.clone());
                }
                async move {
                    let response = send.send().await.map_err(HttpError::Client)?;
                    let status = response.status();
                    if status.is_success() {
                        Ok(())
                    } else {
                        Err(HttpError::Status(status.as_u16()))
                    }
                }
            },
            message,
        )
        .await
    }
}

const PROTOBUF_CONTENT_TYPE: &str = "application/x-protobuf";

/// OTLP/HTTP signal path: the explicit endpoint with `/v1/{signal}` appended
/// unless it already ends there (the synchronous HTTP backend's rule).
fn signal_url(endpoint: &str, signal: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let suffix = format!("/v1/{signal}");
    if endpoint.ends_with(&suffix) {
        endpoint.to_owned()
    } else {
        format!("{endpoint}{suffix}")
    }
}

/// One failed OTLP/HTTP attempt, reduced to what retry classification needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HttpFailure {
    /// The collector answered with a non-success HTTP status.
    Status(u16),
    /// The connection could not be established or the request timed out.
    ConnectOrTimeout,
    /// Any other client failure (request construction, body, protocol).
    Other,
}

impl std::fmt::Display for HttpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status(status) => write!(f, "OTLP/HTTP collector answered status {status}"),
            Self::ConnectOrTimeout => f.write_str("OTLP/HTTP connect or request timeout failure"),
            Self::Other => f.write_str("OTLP/HTTP client failure"),
        }
    }
}

impl std::error::Error for HttpFailure {}

/// One failed OTLP/HTTP send, keeping the client's own error as its source.
#[derive(Debug)]
pub(super) enum HttpError {
    /// The collector answered with a non-success HTTP status.
    Status(u16),
    /// The HTTP client failed before a status was received.
    Client(reqwest::Error),
}

impl HttpError {
    pub(super) fn failure(&self) -> HttpFailure {
        match self {
            Self::Status(status) => HttpFailure::Status(*status),
            Self::Client(error) if error.is_connect() || error.is_timeout() => {
                HttpFailure::ConnectOrTimeout
            }
            Self::Client(error) => error.status().map_or(HttpFailure::Other, |status| {
                HttpFailure::Status(status.as_u16())
            }),
        }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.failure().fmt(f)
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Status(_) => None,
            Self::Client(error) => Some(error),
        }
    }
}

impl RetryClass for HttpError {
    fn retry_decision(&self) -> RetryDecision {
        self.failure().retry_decision()
    }
}

/// Protocol classification without discarding server pacing details.
pub(super) trait RetryClass {
    fn retry_decision(&self) -> RetryDecision;
}

impl RetryClass for tonic::Status {
    fn retry_decision(&self) -> RetryDecision {
        super::retry::classify_grpc(self)
    }
}

impl RetryClass for HttpFailure {
    fn retry_decision(&self) -> RetryDecision {
        match self {
            Self::Status(429 | 502 | 503 | 504) | Self::ConnectOrTimeout => {
                RetryDecision::Retryable
            }
            Self::Status(_) | Self::Other => RetryDecision::Terminal,
        }
    }
}

impl ExporterLifecycle for SdkTerminal {
    fn is_shutdown(&self) -> bool {
        false
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        // D.6 has already waited for every admitted RPC before reaching this
        // terminal transport. Neither protocol client has an independent batch
        // processor to flush, so completion here means the client is usable.
        Box::pin(async { Ok(()) })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        Box::pin(async { Ok(()) })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        Err(async_lifecycle_required_error())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        Err(async_lifecycle_required_error())
    }
}

struct SdkLifecycle {
    lifecycle: LifecycleCore,
    shutdown_cancel: watch::Sender<bool>,
}

impl ExporterLifecycle for SdkLifecycle {
    fn is_shutdown(&self) -> bool {
        self.lifecycle.health().phase != LifecycleState::Open
    }

    fn lifecycle_health(&self) -> Option<crate::lifecycle::LifecycleHealth> {
        Some(self.lifecycle.health())
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn blocking_lifecycle_preflight(&self) -> Result<(), ExportError> {
        Err(async_lifecycle_required_error())
    }

    fn flush_async(&self) -> LifecycleFuture {
        let lifecycle = self.lifecycle.clone();
        Box::pin(async move { lifecycle.flush_async().await })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        let lifecycle = self.lifecycle.clone();
        let shutdown_cancel = self.shutdown_cancel.clone();
        Box::pin(async move {
            // The core waits for admitted work through its shutdown deadline.
            // Only after it completes (including a timeout) do we interrupt
            // residual RPCs/backoff, preserving normal drain-first shutdown.
            let result = lifecycle.shutdown_async().await;
            shutdown_cancel.send_replace(true);
            result
        })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        Err(async_lifecycle_required_error())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        Err(async_lifecycle_required_error())
    }
}

struct SdkLogExporter {
    backend: Arc<SdkBackend>,
}

impl LogExporter for SdkLogExporter {
    fn export_logs(&self, batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        schedule(
            &self.backend,
            Signal::Logs,
            batch,
            project_logs,
            |terminal, request| async move { terminal.export_logs(request).await },
        )
    }
}

struct SdkTraceExporter {
    backend: Arc<SdkBackend>,
}

impl TraceExporter for SdkTraceExporter {
    fn export_spans(&self, batch: &[ExportRecord<CompleteSpan>]) -> Result<(), ExportError> {
        schedule(
            &self.backend,
            Signal::Traces,
            batch,
            project_spans,
            |terminal, request| async move { terminal.export_spans(request).await },
        )
    }
}

struct SdkMetricExporter {
    backend: Arc<SdkBackend>,
}

impl MetricExporter for SdkMetricExporter {
    fn export_metrics(&self, batch: &[ExportRecord<MetricRecord>]) -> Result<(), ExportError> {
        schedule(
            &self.backend,
            Signal::Metrics,
            batch,
            project_metrics,
            |terminal, request| async move { terminal.export_metrics(request).await },
        )
    }
}

fn schedule<T, R, P, E, F>(
    backend: &Arc<SdkBackend>,
    signal: Signal,
    batch: &[ExportRecord<T>],
    project: P,
    export: E,
) -> Result<(), ExportError>
where
    T: Clone + Send + 'static,
    P: Fn(&[ExportRecord<T>]) -> Vec<R>,
    R: Send + 'static,
    E: Fn(Arc<SdkTerminal>, Vec<R>) -> F,
    F: Future<Output = Result<(), ExportError>> + Send + 'static,
{
    // The shared admission core tracks a conservative bounded payload budget
    // before projection; the transport owns the exact protobuf allocation.
    let bytes = batch.len().saturating_mul(1_024);
    let admitted = backend
        .lifecycle
        .admit(signal, batch.to_vec(), bytes)
        .map_err(telemetry_error_to_export_error)?;
    let request = project(admitted.get());
    let terminal = Arc::clone(&backend.terminal);
    backend
        .runtime
        .spawn_export(admitted, export(terminal, request));
    Ok(())
}

fn runtime_required_error() -> ExportError {
    ExportError::RuntimeTerminated {
        context: Box::new(ErrorContext::new(
            crate::error_codes::OTLP_EXPORT_TERMINAL,
            "the OTLP SDK backend requires an entered caller Tokio runtime",
            Remediation::recoverable(
                "construct the SDK adapter inside the host Tokio runtime",
                [] as [&str; 0],
            ),
        )),
    }
}

fn async_lifecycle_required_error() -> ExportError {
    ExportError::AsyncLifecycleRequired {
        context: Box::new(ErrorContext::new(
            crate::error_codes::OTLP_EXPORT_TERMINAL,
            "the Tokio SDK backend lifecycle must be awaited",
            Remediation::recoverable("use the asynchronous lifecycle operation", [] as [&str; 0]),
        )),
    }
}

fn transport_error(message: &str) -> ExportError {
    ExportError::Transport {
        context: Box::new(transport_context(message)),
    }
}

/// [`transport_error`] that keeps the underlying failure as the error source.
pub(super) fn transport_error_from(
    message: &str,
    source: Box<dyn std::error::Error + Send + Sync + 'static>,
) -> ExportError {
    ExportError::Transport {
        context: Box::new(transport_context(message).source(source)),
    }
}

fn transport_context(message: &str) -> ErrorContext {
    ErrorContext::new(
        crate::error_codes::OTLP_EXPORT_TERMINAL,
        message,
        Remediation::recoverable(
            "verify the explicit OTLP endpoint and collector availability",
            [] as [&str; 0],
        ),
    )
}

pub(super) fn shutdown_cancelled_error() -> ExportError {
    ExportError::ShutdownCancelledRetry {
        context: Box::new(ErrorContext::new(
            sc_observability_types::error_codes::otlp::OTLP_SHUTDOWN_CANCELLED_RETRY,
            "OTLP SDK retry was cancelled after shutdown exceeded its drain deadline",
            Remediation::not_recoverable(
                "the OTLP SDK exporter is shutting down and cannot retry this batch",
            ),
        )),
    }
}

fn telemetry_error_to_export_error(error: TelemetryError) -> ExportError {
    match error {
        TelemetryError::ExportFailure(error) => error,
        TelemetryError::Shutdown { context } => ExportError::TerminalExportFailure { context },
        _ => ExportError::TerminalExportFailure {
            context: Box::new(ErrorContext::new(
                crate::error_codes::OTLP_EXPORT_TERMINAL,
                "OTLP admission failed before export scheduling",
                Remediation::recoverable("inspect telemetry lifecycle health", [] as [&str; 0]),
            )),
        },
    }
}

/// One resource-homogeneous export group.
///
/// A collector request may carry only one resource. Grouping therefore occurs
/// before admission/scheduling; record order remains stable inside each group
/// and group order is the resource's first appearance in the input batch.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResourceGroup<T> {
    /// Shared resource for every record in this group.
    pub(crate) resource: Resource,
    /// Ordered instrumentation-scope subgroups for that resource.
    pub(crate) scopes: Vec<ScopeGroup<T>>,
}

/// One instrumentation-scope-homogeneous export group.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScopeGroup<T> {
    /// Shared instrumentation scope for every record in the group.
    pub(crate) scope: InstrumentationScope,
    /// Records belonging to that scope, preserving input order.
    pub(crate) records: Vec<T>,
}

/// Groups neutral records by their exact resource without reordering within a
/// resource. This is intentionally transport-neutral so it applies to logs,
/// spans, and metrics alike.
pub(crate) fn group_by_resource<T: Clone>(batch: &[ExportRecord<T>]) -> Vec<ResourceGroup<T>> {
    group_records_by_resource_and_scope(batch)
        .into_iter()
        .map(|group| ResourceGroup {
            resource: group.resource,
            scopes: group
                .scopes
                .into_iter()
                .map(|scope| ScopeGroup {
                    scope: scope.scope,
                    records: scope.records,
                })
                .collect(),
        })
        .collect()
}

/// Projects resource- and scope-tagged logs into OTLP wire messages.
///
/// This deliberately projects protobuf messages instead of constructing the
/// official SDK's private log records. D.18 owns transport activation; this
/// layer owns preserving every representable neutral field on that hand-off.
pub(crate) fn project_logs(batch: &[ExportRecord<LogRecord>]) -> Vec<proto_logs::ResourceLogs> {
    group_by_resource(batch)
        .into_iter()
        .map(|group| proto_logs::ResourceLogs {
            resource: Some(project_resource(&group.resource)),
            schema_url: group.resource.schema_url.unwrap_or_default(),
            scope_logs: group
                .scopes
                .into_iter()
                .map(|scope| proto_logs::ScopeLogs {
                    scope: Some(project_scope(&scope.scope)),
                    schema_url: scope.scope.schema_url.unwrap_or_default(),
                    log_records: scope.records.into_iter().map(project_log).collect(),
                })
                .collect(),
        })
        .collect()
}

/// Projects completed spans, including links, events, status, and temporal
/// bounds, into resource/scope-homogeneous OTLP messages.
pub(crate) fn project_spans(
    batch: &[ExportRecord<CompleteSpan>],
) -> Vec<proto_trace::ResourceSpans> {
    group_by_resource(batch)
        .into_iter()
        .map(|group| proto_trace::ResourceSpans {
            resource: Some(project_resource(&group.resource)),
            schema_url: group.resource.schema_url.unwrap_or_default(),
            scope_spans: group
                .scopes
                .into_iter()
                .map(|scope| proto_trace::ScopeSpans {
                    scope: Some(project_scope(&scope.scope)),
                    schema_url: scope.scope.schema_url.unwrap_or_default(),
                    spans: scope.records.into_iter().map(project_span).collect(),
                })
                .collect(),
        })
        .collect()
}

/// Projects neutral metric points without asking the SDK to recreate their
/// aggregation. Histogram buckets, temporality and interval start are carried
/// directly into their OTLP counterparts.
pub(crate) fn project_metrics(
    batch: &[ExportRecord<MetricRecord>],
) -> Vec<proto_metrics::ResourceMetrics> {
    group_by_resource(batch)
        .into_iter()
        .map(|group| proto_metrics::ResourceMetrics {
            resource: Some(project_resource(&group.resource)),
            schema_url: group.resource.schema_url.unwrap_or_default(),
            scope_metrics: group
                .scopes
                .into_iter()
                .map(|scope| proto_metrics::ScopeMetrics {
                    scope: Some(project_scope(&scope.scope)),
                    schema_url: scope.scope.schema_url.unwrap_or_default(),
                    metrics: scope.records.iter().map(project_metric).collect(),
                })
                .collect(),
        })
        .collect()
}

fn project_resource(resource: &Resource) -> proto_resource::Resource {
    proto_resource::Resource {
        attributes: project_attributes(&resource.attributes),
        dropped_attributes_count: 0,
        entity_refs: Vec::new(),
    }
}

fn project_scope(scope: &InstrumentationScope) -> proto_common::InstrumentationScope {
    proto_common::InstrumentationScope {
        name: scope.name.clone(),
        version: scope.version.clone().unwrap_or_default(),
        attributes: project_attributes(&scope.attributes),
        dropped_attributes_count: 0,
    }
}

fn project_log(log: LogRecord) -> proto_logs::LogRecord {
    let event = log.event;
    let mut attributes = log.attributes;
    attributes.insert(
        "service.name".to_owned(),
        AttributeValue::String(event.service.as_str().to_owned()),
    );
    attributes.insert(
        "log.target".to_owned(),
        AttributeValue::String(event.target.as_str().to_owned()),
    );
    attributes.insert(
        "event.name".to_owned(),
        AttributeValue::String(event.action.as_str().to_owned()),
    );
    attributes.insert(
        "sc.observability.log.version".to_owned(),
        json_attribute(serde_json::to_value(&event.version).expect("log version serializes")),
    );
    attributes.insert(
        "sc.observability.log.identity".to_owned(),
        json_attribute(serde_json::to_value(&event.identity).expect("identity serializes")),
    );
    if let Some(value) = &event.request_id {
        attributes.insert(
            "sc.observability.log.request_id".to_owned(),
            json_attribute(serde_json::to_value(value).expect("request id serializes")),
        );
    }
    if let Some(value) = &event.correlation_id {
        attributes.insert(
            "sc.observability.log.correlation_id".to_owned(),
            json_attribute(serde_json::to_value(value).expect("correlation id serializes")),
        );
    }
    if let Some(value) = &event.outcome {
        attributes.insert(
            "sc.observability.log.outcome".to_owned(),
            json_attribute(serde_json::to_value(value).expect("outcome serializes")),
        );
    }
    if let Some(value) = &event.diagnostic {
        attributes.insert(
            "sc.observability.log.diagnostic".to_owned(),
            json_attribute(serde_json::to_value(value).expect("diagnostic serializes")),
        );
    }
    if let Some(value) = &event.state_transition {
        attributes.insert(
            "sc.observability.log.state_transition".to_owned(),
            json_attribute(serde_json::to_value(value).expect("state transition serializes")),
        );
    }
    for (key, value) in event.fields {
        attributes.insert(key, json_attribute(value));
    }

    let (trace_id, span_id) = event.trace.as_ref().map_or_else(
        || (Vec::new(), Vec::new()),
        |trace| {
            if let Some(parent_span_id) = trace.parent_span_id.as_ref() {
                attributes.insert(
                    "sc.observability.log.parent_span_id".to_owned(),
                    AttributeValue::String(parent_span_id.as_str().to_owned()),
                );
            }
            (
                decode_hex(trace.trace_id.as_str()),
                decode_hex(trace.span_id.as_str()),
            )
        },
    );
    proto_logs::LogRecord {
        time_unix_nano: unix_nanos(event.timestamp),
        observed_time_unix_nano: unix_nanos(event.timestamp),
        severity_number: project_severity(event.level),
        severity_text: crate::severity::fields(event.level).1.to_owned(),
        body: event.message.map(string_value),
        attributes: project_attributes(&attributes),
        dropped_attributes_count: 0,
        flags: u32::from(log.trace_flags.bits()),
        trace_id,
        span_id,
        event_name: event.action.as_str().to_owned(),
    }
}

fn project_span(span: CompleteSpan) -> proto_trace::Span {
    let record = span.record;
    let start = unix_nanos(record.timestamp());
    let end = start.saturating_add(record.duration_ms().as_u64().saturating_mul(1_000_000));
    let mut attributes = record.attributes().clone();
    attributes.insert(
        "service.name".to_owned(),
        AttributeValue::String(record.service().as_str().to_owned()),
    );
    if let Some(diagnostic) = record.diagnostic() {
        attributes.insert(
            "sc.observability.span.diagnostic".to_owned(),
            json_attribute(serde_json::to_value(diagnostic).expect("diagnostic serializes")),
        );
    }
    proto_trace::Span {
        trace_id: decode_hex(record.trace().trace_id.as_str()),
        span_id: decode_hex(record.trace().span_id.as_str()),
        trace_state: String::new(),
        parent_span_id: record
            .trace()
            .parent_span_id
            .as_ref()
            .map_or_else(Vec::new, |parent| decode_hex(parent.as_str())),
        flags: u32::from(record.trace().flags.bits()),
        name: record.name().as_str().to_owned(),
        kind: project_span_kind(record.kind()),
        start_time_unix_nano: start,
        end_time_unix_nano: end,
        attributes: project_attributes(&attributes),
        dropped_attributes_count: 0,
        events: span
            .events
            .into_iter()
            .map(|event| {
                let mut attributes = event.attributes;
                attributes.insert(
                    "sc.observability.span_event.trace_id".to_owned(),
                    AttributeValue::String(event.trace.trace_id.as_str().to_owned()),
                );
                attributes.insert(
                    "sc.observability.span_event.span_id".to_owned(),
                    AttributeValue::String(event.trace.span_id.as_str().to_owned()),
                );
                attributes.insert(
                    "sc.observability.span_event.trace_flags".to_owned(),
                    AttributeValue::UInt(u64::from(event.trace.flags.bits())),
                );
                if let Some(parent) = event.trace.parent_span_id {
                    attributes.insert(
                        "sc.observability.span_event.parent_span_id".to_owned(),
                        AttributeValue::String(parent.as_str().to_owned()),
                    );
                }
                if let Some(diagnostic) = event.diagnostic {
                    attributes.insert(
                        "sc.observability.span_event.diagnostic".to_owned(),
                        json_attribute(
                            serde_json::to_value(diagnostic).expect("diagnostic serializes"),
                        ),
                    );
                }
                proto_trace::span::Event {
                    time_unix_nano: unix_nanos(event.timestamp),
                    name: event.name.as_str().to_owned(),
                    attributes: project_attributes(&attributes),
                    dropped_attributes_count: 0,
                }
            })
            .collect(),
        dropped_events_count: 0,
        links: record
            .links()
            .iter()
            .map(|link| proto_trace::span::Link {
                trace_id: decode_hex(link.trace_id.as_str()),
                span_id: decode_hex(link.span_id.as_str()),
                trace_state: String::new(),
                attributes: project_attributes(&link.attributes),
                dropped_attributes_count: 0,
                flags: u32::from(link.flags.bits()),
            })
            .collect(),
        dropped_links_count: 0,
        status: Some(proto_trace::Status {
            message: record
                .diagnostic()
                .map_or_else(String::new, |diagnostic| diagnostic.message.clone()),
            code: project_span_status(record.status()),
        }),
    }
}

fn project_metric(record: &MetricRecord) -> proto_metrics::Metric {
    let common = || proto_metrics::NumberDataPoint {
        attributes: project_attributes(record.attributes()),
        start_time_unix_nano: 0,
        time_unix_nano: unix_nanos(record.timestamp()),
        exemplars: Vec::new(),
        flags: 0,
        value: None,
    };
    let data = match record.value() {
        MetricValue::Gauge(value) => {
            let mut point = common();
            point.value = Some(proto_metrics::number_data_point::Value::AsDouble(
                value.get(),
            ));
            proto_metrics::metric::Data::Gauge(proto_metrics::Gauge {
                data_points: vec![point],
            })
        }
        MetricValue::Sum {
            value,
            monotonic,
            temporality,
            start_time,
        } => {
            let mut point = common();
            point.start_time_unix_nano = unix_nanos(*start_time);
            point.value = Some(proto_metrics::number_data_point::Value::AsDouble(
                value.get(),
            ));
            proto_metrics::metric::Data::Sum(proto_metrics::Sum {
                data_points: vec![point],
                aggregation_temporality: project_temporality(*temporality),
                is_monotonic: *monotonic,
            })
        }
        MetricValue::Histogram {
            point,
            temporality,
            start_time,
        } => proto_metrics::metric::Data::Histogram(proto_metrics::Histogram {
            data_points: vec![proto_metrics::HistogramDataPoint {
                attributes: project_attributes(record.attributes()),
                start_time_unix_nano: unix_nanos(*start_time),
                time_unix_nano: unix_nanos(record.timestamp()),
                count: point.count(),
                sum: Some(point.sum().get()),
                bucket_counts: point.bucket_counts().to_vec(),
                explicit_bounds: point
                    .explicit_bounds()
                    .iter()
                    .map(|bound| bound.get())
                    .collect(),
                exemplars: Vec::new(),
                flags: 0,
                min: None,
                max: None,
            }],
            aggregation_temporality: project_temporality(*temporality),
        }),
        // The neutral enum is intentionally non-exhaustive. A new aggregation
        // must receive an explicit OTLP representation rather than silently
        // becoming a gauge or being dropped from an export request.
        _ => unreachable!("new neutral metric aggregation requires an OTLP projection"),
    };
    proto_metrics::Metric {
        name: record.name().as_str().to_owned(),
        description: String::new(),
        unit: record
            .unit()
            .map_or_else(String::new, |unit| unit.as_str().to_owned()),
        metadata: vec![key_value(
            "service.name",
            AttributeValue::String(record.service().as_str().to_owned()),
        )],
        data: Some(data),
    }
}

fn project_attributes(attributes: &Attributes) -> Vec<proto_common::KeyValue> {
    attributes
        .iter()
        .map(|(key, value)| key_value(key, value.clone()))
        .collect()
}

fn key_value(key: impl Into<String>, value: AttributeValue) -> proto_common::KeyValue {
    proto_common::KeyValue {
        key: key.into(),
        value: Some(attribute_value(value)),
        key_strindex: 0,
    }
}

fn string_value(value: String) -> proto_common::AnyValue {
    proto_common::AnyValue {
        value: Some(proto_common::any_value::Value::StringValue(value)),
    }
}

fn attribute_value(value: AttributeValue) -> proto_common::AnyValue {
    use proto_common::any_value::Value;
    let value = match value {
        AttributeValue::Bool(value) => Value::BoolValue(value),
        AttributeValue::Int(value) => Value::IntValue(value),
        // OTLP has no unsigned scalar. Decimal encoding is exact and is paired
        // with the neutral envelope until a receiver applies its own schema.
        AttributeValue::UInt(value) => Value::StringValue(value.to_string()),
        AttributeValue::Float(value) => Value::DoubleValue(value.get()),
        AttributeValue::String(value) => Value::StringValue(value),
        AttributeValue::Array(values) => Value::ArrayValue(proto_common::ArrayValue {
            values: values.into_iter().map(attribute_value).collect(),
        }),
        AttributeValue::Object(values) => Value::KvlistValue(proto_common::KeyValueList {
            values: project_attributes(&values),
        }),
        AttributeValue::Null => return proto_common::AnyValue { value: None },
        // New neutral values must be mapped explicitly: a lossy fallback here
        // would turn a source-model extension into an invisible export change.
        _ => unreachable!("new neutral attribute value requires an OTLP projection"),
    };
    proto_common::AnyValue { value: Some(value) }
}

fn json_attribute(value: serde_json::Value) -> AttributeValue {
    match value {
        serde_json::Value::Null => AttributeValue::Null,
        serde_json::Value::Bool(value) => AttributeValue::Bool(value),
        serde_json::Value::String(value) => AttributeValue::String(value),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                AttributeValue::Int(value)
            } else if let Some(value) = value.as_u64() {
                AttributeValue::UInt(value)
            } else {
                AttributeValue::Float(
                    sc_observability_types::v2::FiniteF64::new(value.as_f64().unwrap_or(0.0))
                        .expect("JSON numbers are finite"),
                )
            }
        }
        serde_json::Value::Array(values) => {
            AttributeValue::Array(values.into_iter().map(json_attribute).collect())
        }
        serde_json::Value::Object(values) => AttributeValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, json_attribute(value)))
                .collect(),
        ),
    }
}

fn unix_nanos(timestamp: sc_observability_types::Timestamp) -> u64 {
    u64::try_from(timestamp.into_inner().unix_timestamp_nanos()).unwrap_or_default()
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

const fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => 0,
    }
}

pub(crate) const fn project_severity(level: sc_observability_types::Level) -> i32 {
    crate::severity::fields(level).0 as i32
}

fn project_span_kind(kind: SpanKind) -> i32 {
    match kind {
        SpanKind::Internal => proto_trace::span::SpanKind::Internal as i32,
        SpanKind::Server => proto_trace::span::SpanKind::Server as i32,
        SpanKind::Client => proto_trace::span::SpanKind::Client as i32,
        SpanKind::Producer => proto_trace::span::SpanKind::Producer as i32,
        SpanKind::Consumer => proto_trace::span::SpanKind::Consumer as i32,
        _ => unreachable!("new neutral span kind requires an OTLP projection"),
    }
}

const fn project_span_status(status: SpanStatus) -> i32 {
    match status {
        SpanStatus::Unset => proto_trace::status::StatusCode::Unset as i32,
        SpanStatus::Ok => proto_trace::status::StatusCode::Ok as i32,
        SpanStatus::Error => proto_trace::status::StatusCode::Error as i32,
    }
}

fn project_temporality(temporality: AggregationTemporality) -> i32 {
    match temporality {
        AggregationTemporality::Delta => proto_metrics::AggregationTemporality::Delta as i32,
        AggregationTemporality::Cumulative => {
            proto_metrics::AggregationTemporality::Cumulative as i32
        }
        _ => unreachable!("new neutral temporality requires an OTLP projection"),
    }
}
