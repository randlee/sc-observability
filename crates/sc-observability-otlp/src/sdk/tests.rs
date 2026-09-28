//! Focused caller-runtime tests for the official SDK adapter.

use super::build_exporter_set;
use super::implementation::{
    CallerRuntime, group_by_resource, project_logs, project_metrics, retry_action,
};
use crate::config::{
    ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol, validated_backend_connection,
    validated_transport_bounds,
};
use crate::contracts::{ExportRecord, InstrumentationScope, LogRecord, Resource};
use crate::lifecycle::{LifecycleCore, SignalKind};
use crate::testing::RecordingLifecycle;
use sc_observability_types::v2::{
    AttributeValue, Attributes, FiniteF64, MetricRecord, MetricValue, TraceFlags,
};
use sc_observability_types::{
    ActionName, Level, LogEvent, MetricName, ProcessIdentity, SchemaVersion, ServiceName, SpanId,
    TargetCategory, Timestamp, TraceContext, TraceId,
};
use std::sync::Arc;
use std::time::Duration;
use tonic::Code;

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
) -> (usize, Option<String>, [u64; 3]) {
    let statuses = statuses.to_vec();
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
        .admit(SignalKind::Logs, (), 1_024)
        .expect("retry admission");
    let result = super::implementation::retry_export(
        deadline,
        move || {
            let response_code = statuses.get(next).copied().unwrap_or(Code::Unavailable);
            next += 1;
            attempts_for_retry.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move {
                if response_code == Code::Ok {
                    Ok(())
                } else {
                    Err(tonic::Status::new(response_code, "scripted fixture"))
                }
            })
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
        lifecycle_core.health().dropped_by_signal,
    )
}

#[tokio::test(flavor = "current_thread")]
async fn sdk_transport_retry_loop_exercises_attempts_and_terminal_modes() {
    assert_eq!(
        run_retry_script(&[Code::Unavailable, Code::Ok], Duration::from_secs(30)).await,
        (2, None, [0, 0, 0]),
        "transient failure must be retried once before success"
    );
    assert_eq!(
        run_retry_script(&[Code::Internal], Duration::from_secs(30)).await,
        (1, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0]),
        "permanent failure must not be retried"
    );
    assert_eq!(
        run_retry_script(&[Code::Unavailable], Duration::from_millis(100)).await,
        (1, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0]),
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
        (4, Some("OTLP_EXPORT_TERMINAL".to_owned()), [1, 0, 0]),
        "max retry budget must terminate after the initial attempt plus three retries"
    );
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
