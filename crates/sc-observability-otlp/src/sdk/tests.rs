//! Focused caller-runtime tests for the official SDK adapter.

use super::build_exporter_set;
use super::implementation::{
    CallerRuntime, HttpError, HttpFailure, RetryClass, group_by_resource, http_retry_action,
    project_logs, project_metrics, retry_action,
};
use crate::config::{
    ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol, validated_backend_connection,
    validated_transport_bounds,
};
use crate::contracts::{ExportRecord, InstrumentationScope, LogRecord, Resource};
use crate::lifecycle::{LifecycleCore, Signal};
use crate::testing::RecordingLifecycle;
use sc_observability_types::v2::{
    AttributeValue, Attributes, FiniteF64, MetricRecord, MetricValue, TraceFlags,
};
use sc_observability_types::{
    ActionName, Level, LogEvent, MetricName, ProcessIdentity, SchemaVersion, ServiceName, SpanId,
    TargetCategory, Timestamp, TraceContext, TraceId,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tonic::Code;

fn never_cancelled_shutdown() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    watch::channel(false)
}

#[test]
fn sdk_adapter_requires_an_entered_caller_runtime() {
    assert!(CallerRuntime::try_capture().is_none());
}

#[test]
fn sdk_retry_transient_failure_gets_bounded_retry_then_success_can_stop() {
    let wait = retry_action(
        Code::Unavailable,
        0,
        Duration::from_millis(1),
        Duration::from_secs(30),
        Duration::from_millis(250),
    );
    assert_eq!(wait, Some(Duration::from_millis(250)));
    assert_eq!(
        retry_action(
            Code::Ok,
            0,
            Duration::from_millis(1),
            Duration::from_secs(30),
            Duration::from_millis(250),
        ),
        None
    );
}

#[test]
fn sdk_retry_permanent_failure_never_retries() {
    assert_eq!(
        retry_action(
            Code::Internal,
            0,
            Duration::from_millis(1),
            Duration::from_secs(30),
            Duration::from_millis(250),
        ),
        None
    );
}

#[test]
fn sdk_retry_exhaustion_and_deadline_are_terminal() {
    assert_eq!(
        retry_action(
            Code::Unavailable,
            crate::constants::DEFAULT_OTLP_MAX_RETRIES,
            Duration::from_millis(1),
            Duration::from_secs(30),
            Duration::from_millis(250),
        ),
        None
    );
    assert_eq!(
        retry_action(
            Code::Unavailable,
            0,
            Duration::from_secs(30),
            Duration::from_secs(30),
            Duration::from_millis(250),
        ),
        None
    );
}

async fn run_retry_script(
    statuses: &[Code],
    deadline: Duration,
) -> (usize, Option<String>, [u64; 4]) {
    let outcomes = statuses
        .iter()
        .map(|&code| {
            if code == Code::Ok {
                Ok(())
            } else {
                Err(tonic::Status::new(code, "scripted fixture"))
            }
        })
        .collect();
    run_scripted_retry(
        outcomes,
        || tonic::Status::new(Code::Unavailable, "scripted fixture"),
        deadline,
    )
    .await
}

async fn run_http_retry_script(
    outcomes: &[Result<(), HttpFailure>],
    deadline: Duration,
) -> (usize, Option<String>, [u64; 4]) {
    run_scripted_retry(outcomes.to_vec(), || HttpFailure::Status(503), deadline).await
}

async fn run_scripted_retry<E, D>(
    outcomes: Vec<Result<(), E>>,
    exhausted: D,
    deadline: Duration,
) -> (usize, Option<String>, [u64; 4])
where
    E: RetryClass + std::error::Error + Clone + Send + Sync + 'static,
    D: Fn() -> E + Send + 'static,
{
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_for_retry = Arc::clone(&attempts);
    let mut next = 0;
    let mut config = OtelConfig::new(ExporterBackend::OpenTelemetrySdk, OtlpProtocol::Grpc);
    config.enabled = true;
    config.timeout_ms = Some(sc_observability_types::DurationMs::from(100));
    config.lifecycle_shutdown_timeout_ms = Some(sc_observability_types::DurationMs::from(
        u64::try_from(deadline.as_millis()).expect("test deadline fits u64"),
    ));
    let bounds = validated_transport_bounds(&config).expect("retry bounds");
    let lifecycle_core =
        LifecycleCore::from_backend(Arc::new(RecordingLifecycle::default()), &bounds)
            .expect("retry lifecycle");
    let admitted = lifecycle_core
        .admit(Signal::Logs, (), 1_024)
        .expect("retry admission");
    let (_shutdown, shutdown_rx) = never_cancelled_shutdown();
    let result = super::implementation::retry_export(
        deadline,
        shutdown_rx,
        move || {
            let outcome = outcomes
                .get(next)
                .cloned()
                .unwrap_or_else(|| Err(exhausted()));
            next += 1;
            attempts_for_retry.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move { outcome })
        },
        "scripted retry fixture",
    );
    let runtime = super::implementation::CallerRuntime::try_capture().expect("runtime");
    let join = runtime.spawn_export(admitted, result);
    join.await.expect("retry task");
    let flush = lifecycle_core.flush_async().await;
    let status_code = flush.err().map(|error| error.code().to_string());
    (
        attempts.load(std::sync::atomic::Ordering::SeqCst),
        status_code,
        lifecycle_core.health().dropped_by_signal.into_inner(),
    )
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn sdk_transport_retry_loop_exercises_attempts_and_terminal_modes() {
    assert_eq!(
        run_retry_script(&[Code::Unavailable, Code::Ok], Duration::from_secs(30)).await,
        (2, None, [0, 0, 0, 0]),
        "transient failure must be retried once before success"
    );
    assert_eq!(
        run_retry_script(&[Code::Internal], Duration::from_secs(30)).await,
        (1, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0, 0]),
        "permanent failure must not be retried"
    );
    assert_eq!(
        run_retry_script(&[Code::Unavailable], Duration::from_millis(100)).await,
        (1, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0, 0]),
        "retry deadline must prevent a second attempt"
    );
    assert_eq!(
        run_retry_script(
            &[
                Code::Unavailable,
                Code::Unavailable,
                Code::Unavailable,
                Code::Unavailable
            ],
            Duration::from_secs(30),
        )
        .await,
        (4, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0, 0]),
        "max retry budget must terminate after the initial attempt plus three retries"
    );
}

#[test]
fn sdk_http_retry_classifies_throttling_gateway_connect_and_timeout_as_retryable() {
    for failure in [
        HttpFailure::Status(429),
        HttpFailure::Status(502),
        HttpFailure::Status(503),
        HttpFailure::Status(504),
        HttpFailure::ConnectOrTimeout,
    ] {
        assert_eq!(
            http_retry_action(
                failure,
                0,
                Duration::from_millis(1),
                Duration::from_secs(30),
                Duration::from_millis(250),
            ),
            Some(Duration::from_millis(250)),
            "{failure:?} must be retried"
        );
    }
}

#[test]
fn sdk_http_retry_treats_every_other_failure_as_terminal() {
    for failure in [
        HttpFailure::Status(400),
        HttpFailure::Status(401),
        HttpFailure::Status(403),
        HttpFailure::Status(404),
        HttpFailure::Status(413),
        HttpFailure::Status(500),
        HttpFailure::Status(501),
        HttpFailure::Other,
    ] {
        assert_eq!(
            http_retry_action(
                failure,
                0,
                Duration::from_millis(1),
                Duration::from_secs(30),
                Duration::from_millis(250),
            ),
            None,
            "{failure:?} must be terminal"
        );
    }
}

#[test]
fn sdk_http_retry_exhaustion_and_deadline_are_terminal() {
    assert_eq!(
        http_retry_action(
            HttpFailure::Status(503),
            crate::constants::DEFAULT_OTLP_MAX_RETRIES,
            Duration::from_millis(1),
            Duration::from_secs(30),
            Duration::from_millis(250),
        ),
        None
    );
    assert_eq!(
        http_retry_action(
            HttpFailure::ConnectOrTimeout,
            0,
            Duration::from_secs(30),
            Duration::from_secs(30),
            Duration::from_millis(250),
        ),
        None
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn sdk_http_retry_loop_uses_the_shared_attempt_and_deadline_policy() {
    assert_eq!(
        run_http_retry_script(
            &[Err(HttpFailure::Status(429)), Ok(())],
            Duration::from_secs(30)
        )
        .await,
        (2, None, [0, 0, 0, 0]),
        "throttled request must be retried once before success"
    );
    assert_eq!(
        run_http_retry_script(&[Err(HttpFailure::Status(400))], Duration::from_secs(30)).await,
        (1, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0, 0]),
        "client error must not be retried"
    );
    assert_eq!(
        run_http_retry_script(
            &[Err(HttpFailure::ConnectOrTimeout)],
            Duration::from_millis(100)
        )
        .await,
        (1, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0, 0]),
        "retry deadline must prevent a second attempt"
    );
    assert_eq!(
        run_http_retry_script(&[], Duration::from_secs(30)).await,
        (4, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0, 0]),
        "max retry budget must terminate after the initial attempt plus three retries"
    );
}

/// Walks the `source()` chain of `error` for a `T`.
fn find_source<'a, T: std::error::Error + 'static>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a T> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(found) = error.downcast_ref::<T>() {
            return Some(found);
        }
        current = error.source();
    }
    None
}

#[tokio::test(flavor = "current_thread")]
async fn sdk_terminal_grpc_failure_keeps_the_tonic_status_as_source() {
    let (_shutdown, shutdown_rx) = never_cancelled_shutdown();
    let error = super::implementation::retry_export(
        Duration::from_secs(30),
        shutdown_rx,
        || async { Err::<(), _>(tonic::Status::new(Code::Internal, "collector rejected")) },
        "OTLP log export failed",
    )
    .await
    .expect_err("terminal status");
    assert_eq!(error.code().to_string(), "OTLP_EXPORT_TERMINAL");
    assert!(error.to_string().starts_with("OTLP log export failed"));
    let status = find_source::<tonic::Status>(&error).expect("tonic status source");
    assert_eq!(status.code(), Code::Internal);
    assert_eq!(status.message(), "collector rejected");
}

#[tokio::test(flavor = "current_thread")]
async fn sdk_terminal_http_failure_keeps_the_client_error_as_source() {
    let client = reqwest::Client::new();
    let (_shutdown, shutdown_rx) = never_cancelled_shutdown();
    let error = super::implementation::retry_export(
        Duration::from_millis(100),
        shutdown_rx,
        || {
            let send = client.post("http://127.0.0.1:1/v1/logs").send();
            async move { send.await.map(|_| ()).map_err(HttpError::Client) }
        },
        "OTLP log export failed",
    )
    .await
    .expect_err("refused connection");
    assert_eq!(error.code().to_string(), "OTLP_EXPORT_TERMINAL");
    let http = find_source::<HttpError>(&error).expect("HTTP send error source");
    assert_eq!(http.failure(), HttpFailure::ConnectOrTimeout);
    let client_error = find_source::<reqwest::Error>(&error).expect("reqwest error source");
    assert!(client_error.is_connect());

    let (_shutdown, shutdown_rx) = never_cancelled_shutdown();
    let status = super::implementation::retry_export(
        Duration::from_secs(30),
        shutdown_rx,
        || async { Err::<(), _>(HttpError::Status(400)) },
        "OTLP log export failed",
    )
    .await
    .expect_err("terminal status");
    assert!(matches!(
        find_source::<HttpError>(&status),
        Some(HttpError::Status(400))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn sdk_retry_shutdown_interrupts_an_in_flight_rpc() {
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let mut started_tx = Some(started_tx);
    let retry = tokio::spawn(super::implementation::retry_export(
        Duration::from_secs(30),
        shutdown_rx,
        move || {
            let started_tx = started_tx.take();
            async move {
                if let Some(started_tx) = started_tx {
                    let _ = started_tx.send(());
                }
                std::future::pending::<Result<(), tonic::Status>>().await
            }
        },
        "scripted in-flight RPC",
    ));

    started_rx.await.expect("RPC started");
    shutdown.send_replace(true);
    let error = retry
        .await
        .expect("retry task")
        .expect_err("shutdown cancels RPC");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::ShutdownCancelledRetry { .. }
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn sdk_retry_shutdown_interrupts_backoff_sleep() {
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (attempt_tx, attempt_rx) = tokio::sync::oneshot::channel();
    let mut attempt_tx = Some(attempt_tx);
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed_attempts = Arc::clone(&attempts);
    let retry = tokio::spawn(super::implementation::retry_export(
        Duration::from_secs(30),
        shutdown_rx,
        move || -> Pin<Box<dyn Future<Output = Result<(), tonic::Status>> + Send>> {
            observed_attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(attempt_tx) = attempt_tx.take() {
                Box::pin(async move {
                    let _ = attempt_tx.send(());
                    Err(tonic::Status::new(Code::Unavailable, "retry fixture"))
                })
            } else {
                Box::pin(async { Err(tonic::Status::new(Code::Internal, "unexpected retry")) })
            }
        },
        "scripted retry backoff",
    ));

    attempt_rx.await.expect("first attempt started");
    tokio::task::yield_now().await;
    assert!(
        !retry.is_finished(),
        "retry is waiting in its backoff sleep"
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);

    shutdown.send_replace(true);
    let error = retry
        .await
        .expect("retry task")
        .expect_err("shutdown cancels backoff");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::ShutdownCancelledRetry { .. }
    ));
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// gRPC logs collector whose every export waits (bounded) for a second export
/// to arrive, counting the exports that met a concurrent partner.
#[derive(Clone)]
struct Rendezvous {
    barrier: Arc<tokio::sync::Barrier>,
    met: Arc<std::sync::atomic::AtomicUsize>,
}

#[tonic::async_trait]
impl opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::LogsService
    for Rendezvous
{
    async fn export(
        &self,
        _request: tonic::Request<
            opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest,
        >,
    ) -> Result<
        tonic::Response<opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceResponse>,
        tonic::Status,
    > {
        if tokio::time::timeout(Duration::from_secs(2), self.barrier.wait())
            .await
            .is_ok()
        {
            self.met.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(tonic::Response::new(
            opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceResponse {
                partial_success: None,
            },
        ))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn sdk_grpc_exports_of_one_signal_are_in_flight_together() {
    let incoming = tonic::transport::server::TcpIncoming::bind(
        "127.0.0.1:0".parse().expect("loopback address"),
    )
    .expect("bind collector");
    let address = incoming.local_addr().expect("collector address");
    let collector = Rendezvous {
        barrier: Arc::new(tokio::sync::Barrier::new(2)),
        met: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    let met = Arc::clone(&collector.met);
    let server = tokio::spawn(tonic::transport::Server::builder().serve_with_incoming(
        opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::LogsServiceServer::new(
            collector,
        ),
        incoming,
    ));

    let mut config = OtelConfig::new(ExporterBackend::OpenTelemetrySdk, OtlpProtocol::Grpc);
    config.enabled = true;
    config.endpoint = Some(OtlpEndpoint::new_typed(format!("http://{address}")).expect("endpoint"));
    let bounds = validated_transport_bounds(&config).expect("bounds");
    let connection = validated_backend_connection(&config).expect("connection");
    let adapter = build_exporter_set(&connection, &bounds).expect("SDK adapter set");

    adapter
        .exporters
        .logs
        .export_logs(&[log_record(None)])
        .expect("first export admitted");
    adapter
        .exporters
        .logs
        .export_logs(&[log_record(None)])
        .expect("second export admitted");
    adapter
        .lifecycle
        .flush_async()
        .await
        .expect("both exports complete");
    server.abort();

    assert_eq!(
        met.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "the second export must reach the collector while the first is in flight"
    );
    assert_eq!(adapter.lifecycle.health().dropped_by_signal, [0, 0, 0, 0]);
}

#[test]
fn sdk_adapter_captures_the_entered_caller_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");

    runtime.block_on(async {
        assert!(CallerRuntime::try_capture().is_some());
    });
}

#[test]
fn sdk_constructor_builds_one_shared_admission_core_from_explicit_connection() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let mut config = OtelConfig::new(ExporterBackend::OpenTelemetrySdk, OtlpProtocol::Grpc);
    config.enabled = true;
    config.endpoint = Some(OtlpEndpoint::new_typed("http://127.0.0.1:4317").expect("endpoint"));
    let bounds = validated_transport_bounds(&config).expect("bounds");
    let connection = validated_backend_connection(&config).expect("connection");

    runtime.block_on(async move {
        let adapter = build_exporter_set(&connection, &bounds).expect("SDK adapter set");
        assert_eq!(adapter.lifecycle.health().admitted_records, 0);
        // The handoff returns both capabilities and the exact core that the
        // capabilities delegate to; no second core is constructed here.
        let _ = adapter.exporters.lifecycle.flush_async().await;
    });
}

#[test]
fn resource_grouping_keeps_each_resource_and_its_record_order() {
    let first = Resource {
        attributes: Attributes::from([(
            "resource.id".to_owned(),
            AttributeValue::String("first".to_owned()),
        )]),
        schema_url: Some("https://example.test/one".to_owned()),
    };
    let second = Resource {
        attributes: Attributes::from([(
            "resource.id".to_owned(),
            AttributeValue::String("second".to_owned()),
        )]),
        schema_url: Some("https://example.test/two".to_owned()),
    };
    let scope = InstrumentationScope::default();
    let groups = group_by_resource(&[
        ExportRecord {
            resource: first.clone(),
            scope: scope.clone(),
            record: 1_u8,
        },
        ExportRecord {
            resource: second.clone(),
            scope: scope.clone(),
            record: 2_u8,
        },
        ExportRecord {
            resource: first.clone(),
            scope: scope.clone(),
            record: 3_u8,
        },
    ]);

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].resource, first);
    assert_eq!(groups[0].scopes[0].scope, scope);
    assert_eq!(groups[0].scopes[0].records, vec![1, 3]);
    assert_eq!(groups[1].resource, second);
    assert_eq!(groups[1].scopes[0].records, vec![2]);
}

fn log_record(parent_span_id: Option<SpanId>) -> ExportRecord<LogRecord> {
    ExportRecord {
        resource: Resource {
            attributes: Attributes::from([(
                "resource.id".to_owned(),
                AttributeValue::String("logs".to_owned()),
            )]),
            schema_url: None,
        },
        scope: InstrumentationScope::default(),
        record: LogRecord {
            event: LogEvent {
                version: SchemaVersion::new("v1").expect("valid schema version"),
                timestamp: Timestamp::UNIX_EPOCH,
                level: Level::Info,
                service: ServiceName::new("sdk-test").expect("valid service"),
                target: TargetCategory::new("sdk.test").expect("valid target"),
                action: ActionName::new("log.emitted").expect("valid action"),
                message: Some("preserve correlation".to_owned()),
                identity: ProcessIdentity::default(),
                trace: Some(TraceContext {
                    trace_id: TraceId::new("0123456789abcdef0123456789abcdef")
                        .expect("valid trace id"),
                    span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
                    parent_span_id,
                }),
                request_id: None,
                correlation_id: None,
                outcome: None,
                diagnostic: None,
                state_transition: None,
                fields: serde_json::Map::new(),
            },
            trace_flags: TraceFlags::new(0x01),
            attributes: Attributes::new(),
        },
    }
}

#[test]
fn log_projection_preserves_parent_span_id_and_absent_parent_behavior() {
    let parent = SpanId::new("fedcba9876543210").expect("valid parent span id");
    let with_parent = project_logs(&[log_record(Some(parent.clone()))]);
    let with_parent = &with_parent[0].scope_logs[0].log_records[0];
    let parent_attribute = with_parent
        .attributes
        .iter()
        .find(|attribute| attribute.key == "sc.observability.log.parent_span_id")
        .expect("parent span id attribute");
    assert_eq!(
        parent_attribute
            .value
            .as_ref()
            .and_then(|value| value.value.as_ref()),
        Some(
            &opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(
                parent.as_str().to_owned(),
            ),
        )
    );
    assert_eq!(
        with_parent.trace_id,
        vec![
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef
        ]
    );
    assert_eq!(
        with_parent.span_id,
        vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
    );

    let without_parent = project_logs(&[log_record(None)]);
    let without_parent = &without_parent[0].scope_logs[0].log_records[0];
    assert!(
        !without_parent
            .attributes
            .iter()
            .any(|attribute| attribute.key == "sc.observability.log.parent_span_id")
    );
    assert_eq!(without_parent.trace_id, with_parent.trace_id);
    assert_eq!(without_parent.span_id, with_parent.span_id);
}

#[test]
fn metric_projection_keeps_resource_scope_and_histogram_distribution() {
    let resource = Resource {
        attributes: Attributes::from([(
            "service.instance.id".to_owned(),
            AttributeValue::String("blue-1".to_owned()),
        )]),
        schema_url: Some("https://schema.example/resource".to_owned()),
    };
    let scope = InstrumentationScope {
        name: "checkout".to_owned(),
        version: Some("1.2.3".to_owned()),
        schema_url: Some("https://schema.example/scope".to_owned()),
        attributes: Attributes::from([(
            "library.language".to_owned(),
            AttributeValue::String("rust".to_owned()),
        )]),
    };
    let histogram = sc_observability_types::v2::HistogramPoint::try_new(
        vec![FiniteF64::new(10.0).expect("finite bound")],
        vec![2, 1],
        3,
        FiniteF64::new(18.0).expect("finite sum"),
    )
    .expect("valid histogram");
    let metric = MetricRecord::try_new(
        Timestamp::UNIX_EPOCH,
        ServiceName::new("checkout").expect("service"),
        MetricName::new("request.duration").expect("metric name"),
        MetricValue::Histogram {
            point: histogram,
            temporality: sc_observability_types::v2::AggregationTemporality::Cumulative,
            start_time: Timestamp::UNIX_EPOCH,
        },
    )
    .expect("metric")
    .with_attributes(Attributes::from([(
        "http.route".to_owned(),
        AttributeValue::String("/checkout".to_owned()),
    )]));

    let projected = project_metrics(&[ExportRecord {
        resource,
        scope,
        record: metric,
    }]);

    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].schema_url, "https://schema.example/resource");
    assert_eq!(
        projected[0].scope_metrics[0]
            .scope
            .as_ref()
            .expect("scope")
            .name,
        "checkout"
    );
    assert_eq!(
        projected[0].scope_metrics[0].metrics[0].name,
        "request.duration"
    );
    let Some(opentelemetry_proto::tonic::metrics::v1::metric::Data::Histogram(histogram)) =
        &projected[0].scope_metrics[0].metrics[0].data
    else {
        panic!("expected histogram projection");
    };
    assert_eq!(histogram.data_points[0].bucket_counts, vec![2, 1]);
    assert_eq!(histogram.data_points[0].explicit_bounds, vec![10.0]);
    assert_eq!(histogram.data_points[0].count, 3);
    assert_eq!(histogram.data_points[0].sum, Some(18.0));
}

/// Real wire fixture: reject the first export, accept the second.
#[derive(Clone)]
struct RetryCollector(Arc<std::sync::atomic::AtomicUsize>);
#[tonic::async_trait]
impl opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::LogsService
    for RetryCollector
{
    async fn export(
        &self,
        _: tonic::Request<
            opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest,
        >,
    ) -> Result<
        tonic::Response<opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceResponse>,
        tonic::Status,
    > {
        if self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            Err(tonic::Status::unavailable("transient collector failure"))
        } else {
            Ok(tonic::Response::new(
                opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceResponse {
                    partial_success: None,
                },
            ))
        }
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn sdk_retry_executor_recovers_over_real_grpc_with_two_requests() {
    use opentelemetry_proto::tonic::collector::logs::v1::{
        ExportLogsServiceRequest, logs_service_client::LogsServiceClient,
        logs_service_server::LogsServiceServer,
    };
    let incoming =
        tonic::transport::server::TcpIncoming::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = incoming.local_addr().unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let server = tokio::spawn(tonic::transport::Server::builder().serve_with_incoming(
        LogsServiceServer::new(RetryCollector(calls.clone())),
        incoming,
    ));
    // Keep virtual time stationary while OS sockets make progress. Only the
    // acknowledged retry below advances it; no timer can race network readiness.
    let clock_guard = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    let client = LogsServiceClient::connect(format!("http://{address}"))
        .await
        .unwrap();
    let (_tx, rx) = never_cancelled_shutdown();
    let (waiting_tx, waiting_rx) = tokio::sync::oneshot::channel();
    let mut waiting_tx = Some(waiting_tx);
    let task = tokio::spawn(super::retry::retry_with_jitter(
        Duration::from_secs(5),
        Duration::from_secs(1),
        rx,
        move |timeout| {
            let mut client = client.clone();
            async move {
                let mut request = tonic::Request::new(ExportLogsServiceRequest {
                    resource_logs: vec![],
                });
                request.set_timeout(timeout);
                client.export(request).await.map(|_| ())
            }
        },
        "collector fixture",
        move || {
            if let Some(tx) = waiting_tx.take() {
                let _ = tx.send(());
            }
            Duration::ZERO
        },
    ));
    waiting_rx.await.unwrap();
    tokio::time::advance(Duration::from_millis(
        crate::constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS,
    ))
    .await;
    task.await.unwrap().unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    server.abort();
    clock_guard.abort();
    let _ = server.await;
    let _ = clock_guard.await;
}
