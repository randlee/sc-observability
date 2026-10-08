//! Thin synchronous OTLP/HTTP protobuf client for CLI and Python frontends
//! (ADR-023).
//!
//! [`Client`] exports native OpenTelemetry records with the official 0.33.0
//! exporters over the blocking `reqwest` transport, so an ordinary synchronous
//! `main` or a Python call needs no Tokio runtime. Native Tokio hosts use the
//! official asynchronous exporters and SDK providers directly instead.
//!
//! # Lifecycle and limits
//!
//! - One client belongs to one CLI invocation or one Python call; `&mut self`
//!   serializes its calls and it is never shared through a mutex.
//! - Every call builds and drops its own transport and exporter. The client
//!   holds no live transport between calls, so dropping it performs no network
//!   or runtime-sensitive teardown.
//! - Construction and every send reject a thread inside an entered Tokio
//!   runtime with [`error_codes::sync::RUNTIME_ENTERED`](crate::error_codes::sync::RUNTIME_ENTERED)
//!   before any blocking transport is created.
//! - The configured timeout bounds connecting and each HTTP request, and is the
//!   deadline of the exporter's native retry sequence. Upstream offers no
//!   forcible cancellation: a call returns after the transport gives up, not at
//!   an exact instant.
//! - OTLP partial-success responses are handled by the official exporter,
//!   which reports them through its internal diagnostics and returns success;
//!   the client does not reclassify them.
//!
//! # Configuration precedence
//!
//! The explicit endpoint, timeout and headers always win over
//! `OTEL_EXPORTER_OTLP_*` environment settings. The official exporter still
//! adds `OTEL_EXPORTER_OTLP_HEADERS` and the per-signal `*_HEADERS` to each
//! request; the client's transport then sets every explicit header on the
//! outgoing request, replacing an environment header with the same name.
//! Environment headers with other names are still sent. Explicit header values
//! are sent verbatim, without the environment's URL decoding. Compression
//! follows the official exporter's environment handling.
//!
//! # Example
//!
//! ```no_run
//! use sc_observability_otlp::api::logs::{AnyValue, LogRecord as _, Severity};
//! use sc_observability_otlp::api::{InstrumentationScope, KeyValue};
//! use sc_observability_otlp::sdk::Resource;
//! use sc_observability_otlp::sync::{Client, SyncError};
//!
//! fn main() -> Result<(), SyncError> {
//!     let resource = Resource::builder_empty().with_service_name("my-cli").build();
//!     let scope = InstrumentationScope::builder("my-cli").with_version("1.0.0").build();
//!     let mut client = Client::new("http://127.0.0.1:4318")?
//!         .with_header("authorization", "Bearer token-from-env")?;
//!     client.send_log(&resource, scope.clone(), |record| {
//!         record.set_severity_number(Severity::Info);
//!         record.set_severity_text("INFO");
//!         record.set_body(AnyValue::from("job finished"));
//!         Ok(())
//!     })?;
//!     client.send_metrics(&resource, scope, |meter| {
//!         let jobs = meter.u64_counter("jobs.completed").build();
//!         jobs.add(1, &[KeyValue::new("queue", "default")]);
//!         Ok(())
//!     })
//! }
//! ```

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use opentelemetry::logs::{LogRecord as _, Logger as _, LoggerProvider as _};
use opentelemetry::metrics::MeterProvider as _;
use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry_http::{Bytes, HttpClient, HttpError, Request, Response};
use opentelemetry_otlp::{
    ExporterBuildError, LogExporter, MetricExporter, Protocol, SpanExporter, WithExportConfig,
    WithHttpConfig,
};
use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
use opentelemetry_sdk::logs::{LogBatch, LogExporter as _, SdkLoggerProvider};
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider, Temporality};
use opentelemetry_sdk::trace::SpanExporter as _;

use crate::constants::{
    DEFAULT_OTLP_TIMEOUT_MS, MAX_BATCH_RECORDS, MAX_INPUT_BYTES, OTLP_HTTP_LOGS_PATH,
    OTLP_HTTP_METRICS_PATH, OTLP_HTTP_TRACES_PATH, SYNC_METRIC_READER_INTERVAL,
};
use crate::error_codes::sync as codes;
use crate::{api, sdk};
use otel_reqwest::header::{HeaderMap, HeaderValue};

#[cfg(test)]
mod tests;

/// Placeholder written in place of credentials in error text.
const REDACTED: &str = "[REDACTED]";

/// Failure of one synchronous-client operation.
///
/// `Validation` means the input or configuration was rejected before any
/// export was attempted, or the caller's closure failed. `Export` means the
/// official exporter or SDK lifecycle reported a failure. Both keep the native
/// SDK error as their source. Error text never contains configured header
/// values or endpoint user information.
#[derive(Debug)]
pub enum SyncError {
    /// Rejected input or configuration; nothing was reported as exported.
    Validation {
        /// Stable code from [`error_codes::sync`](crate::error_codes::sync).
        code: &'static str,
        /// Native SDK error describing the rejection.
        source: sdk::error::OTelSdkError,
    },
    /// The official exporter or SDK lifecycle failed.
    Export(sdk::error::OTelSdkError),
}

impl SyncError {
    /// Builds a validation failure with a registry `code` and a message.
    ///
    /// Frontend closures use this to reject unsupported input.
    #[must_use]
    pub fn validation(code: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            code,
            source: OTelSdkError::InternalFailure(message.into()),
        }
    }
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation { code, source } => {
                write!(f, "synchronous OTLP input rejected ({code}): {source}")
            }
            Self::Export(source) => write!(f, "synchronous OTLP export failed: {source}"),
        }
    }
}

impl std::error::Error for SyncError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation { source, .. } | Self::Export(source) => Some(source),
        }
    }
}

/// Rejects frontend input above [`MAX_INPUT_BYTES`] or [`MAX_BATCH_RECORDS`].
///
/// CLI and Python entry points call this before parsing or exporting input.
///
/// # Errors
///
/// Returns [`SyncError::Validation`] with
/// [`error_codes::sync::INPUT_LIMIT_EXCEEDED`](crate::error_codes::sync::INPUT_LIMIT_EXCEEDED)
/// when either limit is exceeded.
pub fn check_input_limits(input_bytes: usize, records: usize) -> Result<(), SyncError> {
    if input_bytes > MAX_INPUT_BYTES {
        return Err(SyncError::validation(
            codes::INPUT_LIMIT_EXCEEDED,
            format!("input is {input_bytes} bytes; the limit is {MAX_INPUT_BYTES}"),
        ));
    }
    if records > MAX_BATCH_RECORDS {
        return Err(SyncError::validation(
            codes::INPUT_LIMIT_EXCEEDED,
            format!("input has {records} records; the limit is {MAX_BATCH_RECORDS}"),
        ));
    }
    Ok(())
}

/// Blocking OTLP/HTTP protobuf client over the official 0.33.0 exporters.
///
/// # Examples
///
/// ```no_run
/// use sc_observability_otlp::api::InstrumentationScope;
/// use sc_observability_otlp::api::logs::{AnyValue, LogRecord as _, Severity};
/// use sc_observability_otlp::sdk::Resource;
/// use sc_observability_otlp::sync::Client;
///
/// # fn main() -> Result<(), sc_observability_otlp::sync::SyncError> {
/// let mut client = Client::new("http://localhost:4318")?;
/// let resource = Resource::builder_empty()
///     .with_service_name("my-cli")
///     .build();
/// client.send_log(&resource, InstrumentationScope::builder("my-cli").build(), |record| {
///     record.set_severity_number(Severity::Info);
///     record.set_body(AnyValue::from("started"));
///     Ok(())
/// })?;
/// # Ok(())
/// # }
/// ```
pub struct Client {
    endpoint: String,
    headers: HeaderMap,
    timeout: Duration,
    root_certificates: Vec<otel_reqwest::Certificate>,
    secrets: Vec<String>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("endpoint", &self.redact(&self.endpoint))
            .field("headers", &self.headers.keys().collect::<Vec<_>>())
            .field("timeout", &self.timeout)
            .field("root_certificates", &self.root_certificates.len())
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Creates a client for an OTLP/HTTP base endpoint such as
    /// `http://localhost:4318`; the client appends `/v1/logs`, `/v1/traces`
    /// and `/v1/metrics`.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] with
    /// [`error_codes::sync::RUNTIME_ENTERED`](crate::error_codes::sync::RUNTIME_ENTERED)
    /// inside an entered Tokio runtime, or
    /// [`error_codes::sync::INVALID_CONFIG`](crate::error_codes::sync::INVALID_CONFIG)
    /// for an endpoint that is not an absolute `http` or `https` URL without
    /// query or fragment.
    pub fn new(endpoint: &str) -> Result<Self, SyncError> {
        ensure_no_runtime()?;
        let mut secrets = Vec::new();
        let url = otel_reqwest::Url::parse(endpoint).map_err(|error| {
            SyncError::validation(
                codes::INVALID_CONFIG,
                format!("endpoint is not an absolute URL: {error}"),
            )
        })?;
        if !url.username().is_empty() {
            secrets.push(url.username().to_owned());
        }
        if let Some(password) = url.password() {
            secrets.push(password.to_owned());
        }
        if !matches!(url.scheme(), "http" | "https")
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(SyncError::validation(
                codes::INVALID_CONFIG,
                "endpoint must be an http or https URL without query or fragment",
            ));
        }
        Ok(Self {
            endpoint: url.as_str().trim_end_matches('/').to_owned(),
            headers: HeaderMap::new(),
            timeout: Duration::from_millis(DEFAULT_OTLP_TIMEOUT_MS),
            root_certificates: Vec::new(),
            secrets,
        })
    }

    /// Adds a header, such as `authorization`, sent with every export.
    ///
    /// Header values are treated as credentials and never appear in error
    /// text or `Debug` output.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] with
    /// [`error_codes::sync::INVALID_CONFIG`](crate::error_codes::sync::INVALID_CONFIG)
    /// for an invalid header name or value.
    pub fn with_header(mut self, name: &str, value: &str) -> Result<Self, SyncError> {
        let name = otel_reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            SyncError::validation(
                codes::INVALID_CONFIG,
                format!("invalid header name {name:?}"),
            )
        })?;
        let mut header_value = HeaderValue::from_str(value).map_err(|_| {
            SyncError::validation(
                codes::INVALID_CONFIG,
                format!("invalid value for header {name}"),
            )
        })?;
        header_value.set_sensitive(true);
        if !value.is_empty() {
            self.secrets.push(value.to_owned());
        }
        self.headers.insert(name, header_value);
        Ok(self)
    }

    /// Sets the connect, request and retry-sequence timeout. The default is
    /// [`DEFAULT_OTLP_TIMEOUT_MS`].
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] with
    /// [`error_codes::sync::INVALID_CONFIG`](crate::error_codes::sync::INVALID_CONFIG)
    /// for a zero timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, SyncError> {
        if timeout.is_zero() {
            return Err(SyncError::validation(
                codes::INVALID_CONFIG,
                "timeout must be greater than zero",
            ));
        }
        self.timeout = timeout;
        Ok(self)
    }

    /// Trusts the root certificates in a PEM certificate or bundle alongside
    /// the platform trust store. Certificate verification is always enabled.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] with
    /// [`error_codes::sync::INVALID_CONFIG`](crate::error_codes::sync::INVALID_CONFIG)
    /// when the PEM holds no certificate or one cannot be parsed or loaded, or with
    /// [`error_codes::sync::RUNTIME_ENTERED`](crate::error_codes::sync::RUNTIME_ENTERED)
    /// inside an entered Tokio runtime.
    pub fn with_root_certificate_pem(mut self, pem: &[u8]) -> Result<Self, SyncError> {
        ensure_no_runtime()?;
        let certificates = otel_reqwest::Certificate::from_pem_bundle(pem).map_err(|error| {
            SyncError::validation(
                codes::INVALID_CONFIG,
                format!("invalid root certificate: {error}"),
            )
        })?;
        if certificates.is_empty() {
            return Err(SyncError::validation(
                codes::INVALID_CONFIG,
                "root certificate PEM contains no certificate",
            ));
        }
        self.root_certificates.extend(certificates);
        // Load the trust store once now so a bad certificate is a
        // configuration error rather than an export failure later.
        drop(self.http_client()?);
        Ok(self)
    }

    /// Exports one native log record.
    ///
    /// The client creates the record from an exporter-less factory provider,
    /// passes it to `record` for the caller to fill through the native
    /// [`LogRecord`](api::logs::LogRecord) setters (`set_timestamp`,
    /// `set_observed_timestamp`, `set_trace_context`, severity, body and
    /// attributes), sets the observed timestamp to now when the closure left it
    /// unset, validates it and exports it in one batch.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] when the closure fails, a trace
    /// context carries an all-zero trace or span id, or a Tokio runtime is
    /// entered; returns [`SyncError::Export`] when the exporter fails.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "ADR-023 fixes an owned scope in all three send signatures; the batch borrows it after the logger takes a copy"
    )]
    pub fn send_log<F>(
        &mut self,
        resource: &sdk::Resource,
        scope: api::InstrumentationScope,
        record: F,
    ) -> Result<(), SyncError>
    where
        F: FnOnce(&mut sdk::logs::SdkLogRecord) -> Result<(), SyncError>,
    {
        ensure_no_runtime()?;
        let factory = SdkLoggerProvider::builder().build();
        let mut log_record = factory.logger_with_scope(scope.clone()).create_log_record();
        record(&mut log_record).map_err(caller_rejected)?;
        if log_record.observed_timestamp().is_none() {
            log_record.set_observed_timestamp(SystemTime::now());
        }
        if let Some(context) = log_record.trace_context()
            && (context.trace_id == TraceId::INVALID || context.span_id == SpanId::INVALID)
        {
            return Err(SyncError::validation(
                codes::INVALID_RECORD,
                "log trace context requires non-zero trace and span ids",
            ));
        }
        let mut exporter = LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(self.signal_endpoint(OTLP_HTTP_LOGS_PATH))
            .with_timeout(self.timeout)
            .with_http_client(self.http_client()?)
            .build()
            .map_err(|error| self.build_error(&error))?;
        exporter.set_resource(resource);
        let batch = [(&log_record, &scope)];
        futures_executor::block_on(exporter.export(LogBatch::new(&batch)))
            .map_err(|error| self.export_error(error))
    }

    /// Exports one completed native span.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] when the span or a link carries an
    /// all-zero trace or span id, the span name is empty, the end time precedes
    /// the start time, or a Tokio runtime is entered; returns
    /// [`SyncError::Export`] when the exporter fails.
    pub fn send_span(
        &mut self,
        resource: &sdk::Resource,
        span: sdk::trace::SpanData,
    ) -> Result<(), SyncError> {
        ensure_no_runtime()?;
        validate_span(&span)?;
        let mut exporter = SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(self.signal_endpoint(OTLP_HTTP_TRACES_PATH))
            .with_timeout(self.timeout)
            .with_http_client(self.http_client()?)
            .build()
            .map_err(|error| self.build_error(&error))?;
        exporter.set_resource(resource);
        futures_executor::block_on(exporter.export(vec![span]))
            .map_err(|error| self.export_error(error))
    }

    /// Records measurements through `record` on a native [`Meter`](api::metrics::Meter)
    /// and exports them in one flush.
    ///
    /// The client builds a native meter provider for `resource` with an
    /// official `PeriodicReader` and delta temporality, invokes `record`, then
    /// returns the SDK `force_flush` result. Exactly one export is attempted;
    /// the following provider shutdown never resends. Measurements use SDK
    /// aggregation and timestamps.
    ///
    /// The 0.33.0 SDK reports a failed flush as `InternalFailure("Failed to
    /// flush")`; the original HTTP cause is only visible in SDK diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Validation`] when the closure fails (nothing is
    /// exported), when no valid measurement was recorded (for example only
    /// instruments with invalid names), or when a Tokio runtime is entered;
    /// returns [`SyncError::Export`] when the flush or shutdown fails.
    pub fn send_metrics<F>(
        &mut self,
        resource: &sdk::Resource,
        scope: api::InstrumentationScope,
        record: F,
    ) -> Result<(), SyncError>
    where
        F: FnOnce(&api::metrics::Meter) -> Result<(), SyncError>,
    {
        ensure_no_runtime()?;
        let exporter = MetricExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(self.signal_endpoint(OTLP_HTTP_METRICS_PATH))
            .with_timeout(self.timeout)
            .with_http_client(self.http_client()?)
            .with_temporality(Temporality::Delta)
            .build()
            .map_err(|error| self.build_error(&error))?;
        let gate = Arc::new(FlushGate::default());
        let reader = PeriodicReader::builder(FlushOnlyExporter {
            inner: exporter,
            gate: Arc::clone(&gate),
        })
        .with_interval(SYNC_METRIC_READER_INTERVAL)
        .build();
        let provider = SdkMeterProvider::builder()
            .with_resource(resource.clone())
            .with_reader(reader)
            .build();
        let recorded = record(&provider.meter_with_scope(scope));
        if let Err(error) = recorded {
            // The gate stays closed, so shutdown's final collection sends nothing.
            let _ = provider.shutdown();
            return Err(caller_rejected(error));
        }
        gate.armed.store(true, Ordering::SeqCst);
        let flushed = provider.force_flush();
        gate.armed.store(false, Ordering::SeqCst);
        let shutdown = provider.shutdown();
        flushed.map_err(|error| self.export_error(error))?;
        shutdown.map_err(|error| self.export_error(error))?;
        if gate.exported.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(SyncError::validation(
                codes::INVALID_RECORD,
                "no valid measurement was recorded",
            ))
        }
    }

    fn signal_endpoint(&self, path: &str) -> String {
        format!("{}/{path}", self.endpoint)
    }

    fn http_client(&self) -> Result<ExplicitHeaders, SyncError> {
        let inner = otel_reqwest::blocking::Client::builder()
            .connect_timeout(self.timeout)
            .timeout(self.timeout)
            .tls_certs_merge(self.root_certificates.iter().cloned())
            .build()
            .map_err(|error| {
                SyncError::validation(
                    codes::INVALID_CONFIG,
                    self.redact(&format!("HTTP transport configuration failed: {error}")),
                )
            })?;
        Ok(ExplicitHeaders {
            inner,
            headers: self.headers.clone(),
        })
    }

    fn build_error(&self, error: &ExporterBuildError) -> SyncError {
        SyncError::validation(codes::INVALID_CONFIG, self.redact(&error.to_string()))
    }

    fn export_error(&self, error: OTelSdkError) -> SyncError {
        SyncError::Export(match error {
            OTelSdkError::InternalFailure(message) => {
                OTelSdkError::InternalFailure(self.redact(&message))
            }
            other => other,
        })
    }

    /// Removes configured credentials and any URL user information.
    fn redact(&self, text: &str) -> String {
        let mut redacted = text.to_owned();
        for secret in &self.secrets {
            redacted = redacted.replace(secret.as_str(), REDACTED);
        }
        redact_url_userinfo(&redacted)
    }
}

fn ensure_no_runtime() -> Result<(), SyncError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(SyncError::validation(
            codes::RUNTIME_ENTERED,
            "the synchronous client cannot run inside a Tokio runtime; use the official asynchronous exporters",
        ));
    }
    Ok(())
}

fn caller_rejected(error: SyncError) -> SyncError {
    match error {
        SyncError::Validation { .. } => error,
        SyncError::Export(source) => SyncError::Validation {
            code: codes::CALLER_REJECTED,
            source,
        },
    }
}

fn validate_span(span: &sdk::trace::SpanData) -> Result<(), SyncError> {
    let invalid = |message: &str| Err(SyncError::validation(codes::INVALID_RECORD, message));
    if !span.span_context.is_valid() {
        return invalid("span context requires non-zero trace and span ids");
    }
    if span.name.is_empty() {
        return invalid("span name must not be empty");
    }
    if span.end_time < span.start_time {
        return invalid("span end time precedes its start time");
    }
    if span
        .links
        .links
        .iter()
        .any(|link| !link.span_context.is_valid())
    {
        return invalid("span link requires non-zero trace and span ids");
    }
    Ok(())
}

/// Replaces `user:password@` after every `://` with the redaction marker.
fn redact_url_userinfo(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find("://") {
        let (head, tail) = rest.split_at(index + 3);
        output.push_str(head);
        let authority_end = tail
            .find(|c: char| {
                matches!(c, '/' | '?' | '#') || c.is_whitespace() || "\"'()<>".contains(c)
            })
            .unwrap_or(tail.len());
        let authority = &tail[..authority_end];
        if let Some(at) = authority.rfind('@') {
            output.push_str(REDACTED);
            output.push_str(&authority[at..]);
        } else {
            output.push_str(authority);
        }
        rest = &tail[authority_end..];
    }
    output.push_str(rest);
    output
}

/// Shared state between [`Client::send_metrics`] and its exporter delegate.
#[derive(Debug, Default)]
struct FlushGate {
    /// Exports pass only while the client's explicit flush runs.
    armed: AtomicBool,
    /// Whether the flush collected at least one metric and exported it.
    exported: AtomicBool,
}

/// Delegates to the official exporter only during the explicit flush, so a
/// failed closure exports nothing, shutdown never resends cumulative data and
/// an empty collection is detected.
#[derive(Debug)]
struct FlushOnlyExporter {
    inner: MetricExporter,
    gate: Arc<FlushGate>,
}

impl PushMetricExporter for FlushOnlyExporter {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        if !self.gate.armed.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.gate.exported.store(true, Ordering::SeqCst);
        self.inner.export(metrics).await
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn temporality(&self) -> Temporality {
        self.inner.temporality()
    }
}

/// The official blocking reqwest client with the explicit headers applied last.
///
/// The exporter adds `OTEL_EXPORTER_OTLP_*HEADERS` to each request before it
/// reaches the transport; setting the explicit headers here makes them replace
/// any same-name environment header, which the exporter's own `with_headers`
/// cannot do.
struct ExplicitHeaders {
    inner: otel_reqwest::blocking::Client,
    headers: HeaderMap,
}

impl fmt::Debug for ExplicitHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExplicitHeaders")
            .field("headers", &self.headers.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

// `HttpClient` is an `async_trait`; this is its expanded signature.
impl HttpClient for ExplicitHeaders {
    fn send_bytes<'life0, 'async_trait>(
        &'life0 self,
        mut request: Request<Bytes>,
    ) -> Pin<Box<dyn Future<Output = Result<Response<Bytes>, HttpError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        for (name, value) in &self.headers {
            request.headers_mut().insert(name, value.clone());
        }
        self.inner.send_bytes(request)
    }
}
