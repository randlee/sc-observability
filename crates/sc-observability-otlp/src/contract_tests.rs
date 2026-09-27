//! D.21 contract tests. They use staged private traits and checked bounds
//! directly, without requiring either production backend.

use std::sync::Arc;

use super::config::{
    BackendTransportBounds, ExporterBackend, LegacyRetryPolicy, OtelConfig, OtlpProtocol,
    validated_transport_bounds,
};
use super::constants;
use super::contracts::{CompleteSpan, ExportRecord, LogRecord};
use super::contracts::{
    ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter, MetricExporter, TraceExporter,
};
use sc_observability_types::error_codes::otlp;
use sc_observability_types::v2::MetricRecord;
use sc_observability_types::v2::{ConfigFailure, ExportError};

fn legacy_config() -> OtelConfig {
    OtelConfig {
        enabled: true,
        backend: ExporterBackend::LegacyHttpJson,
        protocol: OtlpProtocol::HttpJson,
        ..OtelConfig::default()
    }
}

#[test]
fn contract_tests_validation_order() {
    let config = OtelConfig {
        timeout_ms: Some(0_u64.into()),
        queue_capacity: Some(0),
        ..legacy_config()
    };
    let error = validated_transport_bounds(&config).expect_err("timeout is checked first");
    assert!(matches!(error, ConfigFailure::ZeroDuration { .. }));
    assert_eq!(error.diagnostic().code, otlp::OTLP_CONFIG_ZERO_DURATION);
}

#[test]
fn contract_tests_every_legacy_duration_precedes_later_validation_bullets() {
    let cases = [
        (
            "legacy_retry.initial_backoff_ms",
            LegacyRetryPolicy {
                initial_backoff_ms: Some(0_u64.into()),
                ..LegacyRetryPolicy::default()
            },
        ),
        (
            "legacy_retry.max_backoff_ms",
            LegacyRetryPolicy {
                max_backoff_ms: Some(0_u64.into()),
                ..LegacyRetryPolicy::default()
            },
        ),
        (
            "legacy_retry.retry_sequence_timeout_ms",
            LegacyRetryPolicy {
                retry_sequence_timeout_ms: Some(0_u64.into()),
                ..LegacyRetryPolicy::default()
            },
        ),
        (
            "legacy_retry.retry_after_cap_ms",
            LegacyRetryPolicy {
                retry_after_cap_ms: Some(0_u64.into()),
                ..LegacyRetryPolicy::default()
            },
        ),
    ];

    for (field, legacy_retry) in cases {
        for (later_failure, config) in [
            (
                "capacity",
                OtelConfig {
                    legacy_retry: Some(legacy_retry.clone()),
                    queue_capacity: Some(0),
                    ..legacy_config()
                },
            ),
            (
                "protocol availability",
                OtelConfig {
                    legacy_retry: Some(legacy_retry.clone()),
                    protocol: OtlpProtocol::Grpc,
                    ..legacy_config()
                },
            ),
            (
                "insecure transport",
                OtelConfig {
                    legacy_retry: Some(legacy_retry),
                    insecure_skip_verify: true,
                    ..legacy_config()
                },
            ),
        ] {
            let error = validated_transport_bounds(&config)
                .expect_err("a zero legacy duration must be rejected first");
            assert!(
                matches!(error, ConfigFailure::ZeroDuration { .. }),
                "{field} must precede {later_failure}; got {error:?}"
            );
            assert_eq!(
                error.diagnostic().details["field"].as_str(),
                Some(field),
                "{field} must precede {later_failure}"
            );
        }
    }
}

#[test]
#[allow(deprecated)]
fn contract_tests_retained_legacy_duration_precedes_capacity() {
    let error = validated_transport_bounds(&OtelConfig {
        initial_backoff_ms: Some(0_u64.into()),
        queue_capacity: Some(0),
        ..legacy_config()
    })
    .expect_err("the retained initial backoff field must keep wire ordering");

    assert!(matches!(error, ConfigFailure::ZeroDuration { .. }));
    assert_eq!(
        error.diagnostic().details["field"].as_str(),
        Some("legacy_retry.initial_backoff_ms")
    );
}

#[test]
fn contract_tests_resolved_defaults() {
    let bounds = validated_transport_bounds(&legacy_config()).expect("default legacy bounds");
    assert_eq!(bounds.queue_capacity, 1_024);
    assert_eq!(bounds.queue_byte_capacity, 16 * 1024 * 1024);
    assert_eq!(bounds.request_timeout.as_millis(), 3_000);
    assert_eq!(bounds.lifecycle_flush_timeout.as_millis(), 30_000);
    assert_eq!(bounds.lifecycle_shutdown_timeout.as_millis(), 30_000);
    let BackendTransportBounds::Legacy(retry) = bounds.backend else {
        panic!("legacy selection retains retry policy");
    };
    assert_eq!(retry.max_retries, 3);
    assert_eq!(retry.jitter_percent, 20);
    assert_eq!(retry.initial_backoff.as_millis(), 250);
    assert_eq!(retry.max_backoff.as_millis(), 5_000);
    assert_eq!(retry.sequence_timeout.as_millis(), 30_000);
    assert_eq!(retry.retry_after_cap.as_millis(), 5_000);
}

#[test]
fn contract_tests_timeout_origin_tracks_default_and_explicit_values() {
    let default_timeout = validated_transport_bounds(&OtelConfig {
        lifecycle_shutdown_timeout_ms: Some(2_999_u64.into()),
        ..legacy_config()
    })
    .expect_err("a shutdown bound below the default timeout is invalid");
    assert!(matches!(
        default_timeout,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    assert_eq!(
        default_timeout.diagnostic().details.get("origin"),
        Some(&serde_json::Value::String("default".to_owned()))
    );

    let explicit_timeout = validated_transport_bounds(&OtelConfig {
        timeout_ms: Some(3_001_u64.into()),
        lifecycle_shutdown_timeout_ms: Some(3_000_u64.into()),
        ..legacy_config()
    })
    .expect_err("a shutdown bound below an explicit timeout is invalid");
    assert!(matches!(
        explicit_timeout,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    assert_eq!(
        explicit_timeout.diagnostic().details.get("origin"),
        Some(&serde_json::Value::String("explicit".to_owned()))
    );

    let explicit_default_timeout = validated_transport_bounds(&OtelConfig {
        timeout_ms: Some(constants::DEFAULT_OTLP_TIMEOUT_MS.into()),
        lifecycle_shutdown_timeout_ms: Some(2_999_u64.into()),
        ..legacy_config()
    })
    .expect_err("an explicit default timeout still records its explicit origin");
    assert!(matches!(
        explicit_default_timeout,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    assert_eq!(
        explicit_default_timeout.diagnostic().details.get("origin"),
        Some(&serde_json::Value::String("explicit".to_owned()))
    );
}

#[test]
fn contract_tests_stable_failure_codes() {
    let zero = validated_transport_bounds(&OtelConfig {
        timeout_ms: Some(0_u64.into()),
        ..legacy_config()
    })
    .expect_err("zero timeout");
    assert_eq!(zero.diagnostic().code, otlp::OTLP_CONFIG_ZERO_DURATION);

    let not_applicable = validated_transport_bounds(&OtelConfig {
        enabled: true,
        backend: ExporterBackend::OpenTelemetrySdk,
        legacy_retry: Some(LegacyRetryPolicy::default()),
        ..OtelConfig::default()
    })
    .expect_err("SDK must not silently accept legacy fields");
    assert!(matches!(
        not_applicable,
        ConfigFailure::ConfigFieldNotApplicable { .. }
    ));
    assert_eq!(
        not_applicable.diagnostic().code,
        otlp::OTLP_CONFIG_FIELD_NOT_APPLICABLE
    );
}

fn sdk_config() -> OtelConfig {
    OtelConfig {
        enabled: true,
        backend: ExporterBackend::OpenTelemetrySdk,
        ..OtelConfig::default()
    }
}

fn assert_sdk_not_applicable_field(config: &OtelConfig, expected_field: &str) {
    let error = validated_transport_bounds(config).expect_err("SDK rejects legacy retry fields");
    assert!(matches!(
        error,
        ConfigFailure::ConfigFieldNotApplicable { .. }
    ));
    assert_eq!(
        error.diagnostic().details["field"].as_str(),
        Some(expected_field)
    );
}

#[test]
#[allow(deprecated)]
fn contract_tests_sdk_reports_max_retries_as_the_first_supplied_legacy_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            max_retries: Some(constants::DEFAULT_OTLP_MAX_RETRIES),
            ..sdk_config()
        },
        "legacy_retry.max_retries",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            max_retries: Some(constants::DEFAULT_OTLP_MAX_RETRIES + 1),
            ..sdk_config()
        },
        "legacy_retry.max_retries",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            legacy_retry: Some(LegacyRetryPolicy {
                max_retries: Some(constants::DEFAULT_OTLP_MAX_RETRIES),
                ..LegacyRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "legacy_retry.max_retries",
    );
}

#[test]
#[allow(deprecated)]
fn contract_tests_sdk_reports_initial_backoff_as_the_first_supplied_legacy_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            initial_backoff_ms: Some(constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS.into()),
            ..sdk_config()
        },
        "legacy_retry.initial_backoff_ms",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            initial_backoff_ms: Some((constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS + 1).into()),
            ..sdk_config()
        },
        "legacy_retry.initial_backoff_ms",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            legacy_retry: Some(LegacyRetryPolicy {
                initial_backoff_ms: Some(constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS.into()),
                ..LegacyRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "legacy_retry.initial_backoff_ms",
    );
}

#[test]
#[allow(deprecated)]
fn contract_tests_sdk_reports_max_backoff_as_the_first_supplied_legacy_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            max_backoff_ms: Some(constants::DEFAULT_OTLP_MAX_BACKOFF_MS.into()),
            ..sdk_config()
        },
        "legacy_retry.max_backoff_ms",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            max_backoff_ms: Some((constants::DEFAULT_OTLP_MAX_BACKOFF_MS + 1).into()),
            ..sdk_config()
        },
        "legacy_retry.max_backoff_ms",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            legacy_retry: Some(LegacyRetryPolicy {
                max_backoff_ms: Some(constants::DEFAULT_OTLP_MAX_BACKOFF_MS.into()),
                ..LegacyRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "legacy_retry.max_backoff_ms",
    );
}

#[test]
#[allow(deprecated)]
fn contract_tests_disabled_rejects_explicit_default_retained_field() {
    let error = validated_transport_bounds(&OtelConfig {
        max_retries: Some(constants::DEFAULT_OTLP_MAX_RETRIES),
        ..OtelConfig::default()
    })
    .expect_err("disabled transport must not silently accept an explicit retained field");
    assert!(matches!(
        error,
        ConfigFailure::ConfigFieldNotApplicable { .. }
    ));
    assert_eq!(
        error.diagnostic().details["field"].as_str(),
        Some("legacy_retry.max_retries")
    );
    assert_eq!(
        error.diagnostic().details["target"].as_str(),
        Some("disabled")
    );
}

#[test]
fn contract_tests_sdk_reports_retry_jitter_as_the_first_supplied_legacy_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            legacy_retry: Some(LegacyRetryPolicy {
                retry_jitter_percent: Some(constants::DEFAULT_OTLP_RETRY_JITTER_PERCENT),
                ..LegacyRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "legacy_retry.retry_jitter_percent",
    );
}

#[test]
fn contract_tests_sdk_reports_retry_sequence_timeout_before_later_wire_fields() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            legacy_retry: Some(LegacyRetryPolicy {
                retry_sequence_timeout_ms: Some(
                    constants::DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS.into(),
                ),
                retry_after_cap_ms: Some(constants::DEFAULT_OTLP_RETRY_AFTER_CAP_MS.into()),
                retry_jitter_percent: Some(constants::DEFAULT_OTLP_RETRY_JITTER_PERCENT),
                ..LegacyRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "legacy_retry.retry_sequence_timeout_ms",
    );
}

#[test]
fn contract_tests_record_and_byte_capacity() {
    let records = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(0),
        ..legacy_config()
    })
    .expect_err("zero records is invalid");
    assert!(matches!(
        records,
        ConfigFailure::InvalidQueueCapacity { .. }
    ));
    assert_eq!(records.diagnostic().code, otlp::OTLP_CONFIG_QUEUE_CAPACITY);

    let bytes = validated_transport_bounds(&OtelConfig {
        queue_byte_capacity: Some(0),
        ..legacy_config()
    })
    .expect_err("zero bytes is invalid");
    assert!(matches!(
        bytes,
        ConfigFailure::InvalidQueueByteCapacity { .. }
    ));
    assert_eq!(
        bytes.diagnostic().code,
        otlp::OTLP_CONFIG_QUEUE_BYTE_CAPACITY
    );

    let upper_bounds = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(constants::MAX_OTLP_QUEUE_CAPACITY),
        queue_byte_capacity: Some(constants::MAX_OTLP_QUEUE_BYTE_CAPACITY),
        ..legacy_config()
    })
    .expect("record and byte upper bounds are accepted");
    assert_eq!(
        upper_bounds.queue_capacity,
        constants::MAX_OTLP_QUEUE_CAPACITY
    );
    assert_eq!(
        upper_bounds.queue_byte_capacity,
        constants::MAX_OTLP_QUEUE_BYTE_CAPACITY
    );

    let records_overflow = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(constants::MAX_OTLP_QUEUE_CAPACITY + 1),
        ..legacy_config()
    })
    .expect_err("record capacity above the upper bound is invalid");
    assert!(matches!(
        records_overflow,
        ConfigFailure::InvalidQueueCapacity { .. }
    ));
    assert_eq!(
        records_overflow.diagnostic().code,
        otlp::OTLP_CONFIG_QUEUE_CAPACITY
    );

    let bytes_overflow = validated_transport_bounds(&OtelConfig {
        queue_byte_capacity: Some(constants::MAX_OTLP_QUEUE_BYTE_CAPACITY + 1),
        ..legacy_config()
    })
    .expect_err("byte capacity above the upper bound is invalid");
    assert!(matches!(
        bytes_overflow,
        ConfigFailure::InvalidQueueByteCapacity { .. }
    ));
    assert_eq!(
        bytes_overflow.diagnostic().code,
        otlp::OTLP_CONFIG_QUEUE_BYTE_CAPACITY
    );
}

fn contract_tests_bound_messages_use_named_constants() {
    let records = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(constants::MAX_OTLP_QUEUE_CAPACITY + 1),
        ..legacy_config()
    })
    .expect_err("record capacity above the named bound is invalid");
    assert_eq!(
        records.diagnostic().message,
        format!(
            "queue capacity must be in 1..={}",
            constants::MAX_OTLP_QUEUE_CAPACITY
        )
    );

    let jitter = validated_transport_bounds(&OtelConfig {
        legacy_retry: Some(LegacyRetryPolicy {
            retry_jitter_percent: Some(constants::MAX_OTLP_RETRY_JITTER_PERCENT + 1),
            ..LegacyRetryPolicy::default()
        }),
        ..legacy_config()
    })
    .expect_err("jitter above the named bound is invalid");
    assert_eq!(
        jitter.diagnostic().message,
        format!(
            "retry jitter percent must be in 0..={}",
            constants::MAX_OTLP_RETRY_JITTER_PERCENT
        )
    );
}

#[test]
fn contract_tests_remaining_validation_variants_and_bullet_order() {
    let jitter = validated_transport_bounds(&OtelConfig {
        legacy_retry: Some(LegacyRetryPolicy {
            retry_jitter_percent: Some(101),
            ..LegacyRetryPolicy::default()
        }),
        ..legacy_config()
    })
    .expect_err("jitter above 100 is invalid");
    assert!(matches!(jitter, ConfigFailure::InvalidJitterPercent { .. }));

    let insecure = validated_transport_bounds(&OtelConfig {
        insecure_skip_verify: true,
        ..legacy_config()
    })
    .expect_err("insecure verification is rejected after ordered bounds");
    assert!(matches!(
        insecure,
        ConfigFailure::InsecureTransportRejected { .. }
    ));
    assert_eq!(
        insecure.diagnostic().details["field"].as_str(),
        Some("insecure_skip_verify")
    );
    assert_eq!(
        insecure.diagnostic().details["origin"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        insecure.diagnostic().details["backend"].as_str(),
        Some("legacy_http_json")
    );

    let shutdown_before_retry_bound = validated_transport_bounds(&OtelConfig {
        lifecycle_shutdown_timeout_ms: Some(2_999_u64.into()),
        legacy_retry: Some(LegacyRetryPolicy {
            retry_sequence_timeout_ms: Some(1_u64.into()),
            ..LegacyRetryPolicy::default()
        }),
        ..legacy_config()
    })
    .expect_err("shared shutdown ordering precedes legacy retry ordering");
    assert!(matches!(
        shutdown_before_retry_bound,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    assert_eq!(
        shutdown_before_retry_bound.diagnostic().details["field"].as_str(),
        Some("timeout_ms")
    );

    let retry_after_cap = validated_transport_bounds(&OtelConfig {
        legacy_retry: Some(LegacyRetryPolicy {
            retry_sequence_timeout_ms: Some(3_000_u64.into()),
            retry_after_cap_ms: Some(3_001_u64.into()),
            ..LegacyRetryPolicy::default()
        }),
        ..legacy_config()
    })
    .expect_err("retry-after cap may not exceed retry sequence timeout");
    assert!(matches!(
        retry_after_cap,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    assert_eq!(
        retry_after_cap.diagnostic().details["field"].as_str(),
        Some("legacy_retry.retry_after_cap_ms")
    );
}

struct FakeLifecycle;
impl ExporterLifecycle for FakeLifecycle {
    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }
    fn flush_async(&self) -> LifecycleFuture {
        Box::pin(async { Ok(()) })
    }
    fn shutdown_async(&self) -> LifecycleFuture {
        Box::pin(async { Ok(()) })
    }
    fn flush_blocking(&self) -> Result<(), ExportError> {
        Ok(())
    }
    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        Ok(())
    }
}

struct FakeLog;
impl LogExporter for FakeLog {
    fn export_logs(&self, _batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        Ok(())
    }
}
struct FakeTrace;
impl TraceExporter for FakeTrace {
    fn export_spans(&self, _batch: &[ExportRecord<CompleteSpan>]) -> Result<(), ExportError> {
        Ok(())
    }
}
struct FakeMetric;
impl MetricExporter for FakeMetric {
    fn export_metrics(&self, _batch: &[ExportRecord<MetricRecord>]) -> Result<(), ExportError> {
        Ok(())
    }
}

#[test]
fn contract_tests_fake_exporter_contract() {
    let exporters = ExporterSet {
        logs: Arc::new(FakeLog),
        traces: Arc::new(FakeTrace),
        metrics: Arc::new(FakeMetric),
        lifecycle: Arc::new(FakeLifecycle),
    };
    exporters.logs.export_logs(&[]).expect("fake logs");
    exporters.traces.export_spans(&[]).expect("fake spans");
    exporters.metrics.export_metrics(&[]).expect("fake metrics");
    exporters
        .lifecycle
        .blocking_preflight()
        .expect("fake preflight");
    exporters.lifecycle.flush_blocking().expect("fake flush");
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn contract_tests_sdk_transports_and_caller_runtime_are_available() {
    // Builder methods are feature-gated upstream. This fails to compile if
    // either transport or any of the three signal exporters loses its feature.
    let _ = opentelemetry_otlp::SpanExporter::builder().with_tonic();
    let _ = opentelemetry_otlp::SpanExporter::builder().with_http();
    let _ = opentelemetry_otlp::LogExporter::builder().with_tonic();
    let _ = opentelemetry_otlp::LogExporter::builder().with_http();
    let _ = opentelemetry_otlp::MetricExporter::builder().with_tonic();
    let _ = opentelemetry_otlp::MetricExporter::builder().with_http();
    let caller = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("caller runtime");
    caller.block_on(async {
        assert!(tokio::runtime::Handle::try_current().is_ok());
        opentelemetry_sdk::runtime::Runtime::delay(
            &opentelemetry_sdk::runtime::Tokio,
            std::time::Duration::ZERO,
        )
        .await;
    });
}

#[test]
fn contract_tests_v2_exporters_retain_signal_and_context() {
    use super::contracts::{InstrumentationScope, Resource};
    use sc_observability_types::v2::{
        AggregationTemporality, AttributeValue, Attributes, FiniteF64, HistogramPoint, MetricValue,
        SpanEvent, SpanKind, SpanRecord, TraceContext, TraceFlags,
    };
    use sc_observability_types::{
        ActionName, DurationMs, MetricName, ServiceName, SpanId, SpanStatus, Timestamp, TraceId,
    };

    struct CheckMetric(ExportRecord<MetricRecord>);
    impl MetricExporter for CheckMetric {
        fn export_metrics(&self, batch: &[ExportRecord<MetricRecord>]) -> Result<(), ExportError> {
            assert_eq!(batch, std::slice::from_ref(&self.0));
            Ok(())
        }
    }
    struct CheckTrace(ExportRecord<CompleteSpan>);
    impl TraceExporter for CheckTrace {
        fn export_spans(&self, batch: &[ExportRecord<CompleteSpan>]) -> Result<(), ExportError> {
            assert_eq!(batch, std::slice::from_ref(&self.0));
            Ok(())
        }
    }
    let resource = Resource {
        attributes: Attributes::from([("host.id".into(), AttributeValue::UInt(u64::MAX))]),
        schema_url: Some("https://example.test/resource".into()),
    };
    let scope = InstrumentationScope {
        name: "contract-consumer".into(),
        version: Some("2.0".into()),
        schema_url: Some("https://example.test/scope".into()),
        attributes: Attributes::from([("scope.enabled".into(), AttributeValue::Bool(true))]),
    };
    let metric = ExportRecord {
        resource: resource.clone(),
        scope: scope.clone(),
        record: MetricRecord::try_new(
            Timestamp::UNIX_EPOCH,
            ServiceName::new("test").unwrap(),
            MetricName::new("latency").unwrap(),
            MetricValue::Histogram {
                point: HistogramPoint::try_new(
                    vec![FiniteF64::new(1.0).unwrap()],
                    vec![1, 2],
                    3,
                    FiniteF64::new(5.0).unwrap(),
                )
                .unwrap(),
                temporality: AggregationTemporality::Cumulative,
                start_time: Timestamp::UNIX_EPOCH,
            },
        )
        .unwrap(),
    };
    let trace = TraceContext::new(
        TraceId::new("1234567890abcdef1234567890abcdef").unwrap(),
        SpanId::new("1234567890abcdef").unwrap(),
        TraceFlags::new(0x81),
    );
    let span = ExportRecord {
        resource,
        scope,
        record: CompleteSpan {
            record: SpanRecord::new(
                Timestamp::UNIX_EPOCH,
                ServiceName::new("test").unwrap(),
                ActionName::new("request").unwrap(),
                trace.clone(),
                Attributes::new(),
            )
            .with_kind(SpanKind::Server)
            .end(SpanStatus::Ok, DurationMs::from(7)),
            events: vec![SpanEvent {
                timestamp: Timestamp::UNIX_EPOCH,
                trace,
                name: ActionName::new("event").unwrap(),
                attributes: Attributes::new(),
                diagnostic: None,
            }],
        },
    };
    let exporters: ExporterSet = ExporterSet {
        logs: Arc::new(FakeLog),
        traces: Arc::new(CheckTrace(span.clone())),
        metrics: Arc::new(CheckMetric(metric.clone())),
        lifecycle: Arc::new(FakeLifecycle),
    };
    exporters.traces.export_spans(&[span]).unwrap();
    exporters.metrics.export_metrics(&[metric]).unwrap();
}

#[test]
fn contract_tests_v2_log_retains_flags_and_integer_attributes() {
    use super::contracts::{InstrumentationScope, Resource};
    use sc_observability_types::v2::{AttributeValue, Attributes, TraceFlags};
    use sc_observability_types::{
        ActionName, Level, LogEvent, SchemaVersion, ServiceName, SpanId, TargetCategory, Timestamp,
        TraceContext, TraceId,
    };

    struct CheckLog(ExportRecord<LogRecord>);
    impl LogExporter for CheckLog {
        fn export_logs(&self, batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
            assert_eq!(batch, std::slice::from_ref(&self.0));
            Ok(())
        }
    }
    let log = ExportRecord {
        resource: Resource {
            attributes: Attributes::from([("resource".into(), AttributeValue::Bool(true))]),
            schema_url: Some("https://example.test/resource".into()),
        },
        scope: InstrumentationScope {
            name: "log-consumer".into(),
            ..InstrumentationScope::default()
        },
        record: LogRecord {
            event: LogEvent {
                version: SchemaVersion::new("v1").unwrap(),
                timestamp: Timestamp::UNIX_EPOCH,
                level: Level::Info,
                service: ServiceName::new("test").unwrap(),
                target: TargetCategory::new("test").unwrap(),
                action: ActionName::new("request").unwrap(),
                message: Some("preserved".into()),
                identity: sc_observability_types::ProcessIdentity::default(),
                trace: Some(TraceContext {
                    trace_id: TraceId::new("1234567890abcdef1234567890abcdef").unwrap(),
                    span_id: SpanId::new("1234567890abcdef").unwrap(),
                    parent_span_id: None,
                }),
                request_id: None,
                correlation_id: None,
                outcome: None,
                diagnostic: None,
                state_transition: None,
                fields: serde_json::Map::new(),
            },
            trace_flags: TraceFlags::new(0x81),
            attributes: Attributes::from([("unsigned".into(), AttributeValue::UInt(u64::MAX))]),
        },
    };
    let exporter: Arc<dyn LogExporter> = Arc::new(CheckLog(log.clone()));
    exporter.export_logs(&[log]).unwrap();
}
