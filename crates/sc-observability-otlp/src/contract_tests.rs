//! D.21 contract tests. They use staged private traits and checked bounds
//! directly, without requiring either production backend.

use std::sync::Arc;

use super::config::{
    BackendTransportBounds, ExporterBackend, OtelConfig, OtlpProtocol, SyncHttpRetryPolicy,
    validated_transport_bounds,
};
use super::constants;
use super::contracts::{CompleteSpan, ExportRecord, LogRecord};
use super::contracts::{ExporterSet, LogExporter, MetricExporter, TraceExporter};
use super::testing::{LifecycleCall, recording_exporter_set};
use sc_observability_types::error_codes::otlp;
use sc_observability_types::v2::MetricRecord;
use sc_observability_types::v2::{ConfigFailure, ExportError};

fn sync_http_config() -> OtelConfig {
    OtelConfig {
        enabled: true,
        backend: ExporterBackend::SyncHttp,
        protocol: OtlpProtocol::HttpJson,
        ..OtelConfig::default()
    }
}

#[test]
fn contract_tests_validation_order() {
    let config = OtelConfig {
        timeout_ms: Some(0_u64.into()),
        queue_capacity: Some(0),
        ..sync_http_config()
    };
    let error = validated_transport_bounds(&config).expect_err("timeout is checked first");
    assert!(matches!(error, ConfigFailure::ZeroDuration { .. }));
    assert_eq!(error.diagnostic().code, otlp::OTLP_CONFIG_ZERO_DURATION);
}

#[test]
fn contract_tests_every_sync_http_duration_precedes_later_validation_bullets() {
    let cases = [
        (
            "sync_http_retry.initial_backoff_ms",
            SyncHttpRetryPolicy {
                initial_backoff_ms: Some(0_u64.into()),
                ..SyncHttpRetryPolicy::default()
            },
        ),
        (
            "sync_http_retry.max_backoff_ms",
            SyncHttpRetryPolicy {
                max_backoff_ms: Some(0_u64.into()),
                ..SyncHttpRetryPolicy::default()
            },
        ),
        (
            "sync_http_retry.retry_sequence_timeout_ms",
            SyncHttpRetryPolicy {
                retry_sequence_timeout_ms: Some(0_u64.into()),
                ..SyncHttpRetryPolicy::default()
            },
        ),
        (
            "sync_http_retry.retry_after_cap_ms",
            SyncHttpRetryPolicy {
                retry_after_cap_ms: Some(0_u64.into()),
                ..SyncHttpRetryPolicy::default()
            },
        ),
    ];

    for (field, sync_http_retry) in cases {
        for (later_failure, config) in [
            (
                "capacity",
                OtelConfig {
                    sync_http_retry: Some(sync_http_retry.clone()),
                    queue_capacity: Some(0),
                    ..sync_http_config()
                },
            ),
            (
                "protocol availability",
                OtelConfig {
                    sync_http_retry: Some(sync_http_retry.clone()),
                    protocol: OtlpProtocol::Grpc,
                    ..sync_http_config()
                },
            ),
            (
                "insecure transport",
                OtelConfig {
                    sync_http_retry: Some(sync_http_retry),
                    insecure_skip_verify: true,
                    ..sync_http_config()
                },
            ),
        ] {
            let error = validated_transport_bounds(&config)
                .expect_err("a zero sync-http duration must be rejected first");
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
fn contract_tests_sync_http_retry_duration_precedes_capacity() {
    let error = validated_transport_bounds(&OtelConfig {
        sync_http_retry: Some(SyncHttpRetryPolicy {
            initial_backoff_ms: Some(0_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        queue_capacity: Some(0),
        ..sync_http_config()
    })
    .expect_err("initial backoff validation precedes capacity");

    assert!(matches!(error, ConfigFailure::ZeroDuration { .. }));
    assert_eq!(
        error.diagnostic().details["field"].as_str(),
        Some("sync_http_retry.initial_backoff_ms")
    );
}

#[test]
fn contract_tests_resolved_defaults() {
    let bounds = validated_transport_bounds(&sync_http_config()).expect("default sync-http bounds");
    assert_eq!(bounds.queue_capacity().get(), 1_024);
    assert_eq!(bounds.queue_byte_capacity().get(), 16 * 1024 * 1024);
    assert_eq!(bounds.request_timeout().get().as_millis(), 3_000);
    assert_eq!(bounds.lifecycle().flush().get().as_millis(), 30_000);
    assert_eq!(bounds.lifecycle().shutdown().get().as_millis(), 30_000);
    let BackendTransportBounds::SyncHttp(retry) = bounds.backend() else {
        panic!("sync-http selection retains retry policy");
    };
    assert_eq!(retry.max_retries(), 3);
    assert_eq!(retry.jitter().get(), 20);
    assert_eq!(retry.initial_backoff().get().as_millis(), 250);
    assert_eq!(retry.max_backoff().get().as_millis(), 5_000);
    assert_eq!(retry.sequence_timeout().get().as_millis(), 30_000);
    assert_eq!(retry.retry_after_cap().get().as_millis(), 5_000);
}

#[test]
fn contract_tests_timeout_origin_tracks_default_and_explicit_values() {
    let default_timeout = validated_transport_bounds(&OtelConfig {
        lifecycle_shutdown_timeout_ms: Some(2_999_u64.into()),
        ..sync_http_config()
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
        ..sync_http_config()
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
        ..sync_http_config()
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
        ..sync_http_config()
    })
    .expect_err("zero timeout");
    assert_eq!(zero.diagnostic().code, otlp::OTLP_CONFIG_ZERO_DURATION);

    let not_applicable = validated_transport_bounds(&OtelConfig {
        enabled: true,
        backend: ExporterBackend::OpenTelemetrySdk,
        sync_http_retry: Some(SyncHttpRetryPolicy::default()),
        ..OtelConfig::default()
    })
    .expect_err("SDK must not silently accept sync-http fields");
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
    let error = validated_transport_bounds(config).expect_err("SDK rejects sync-http retry fields");
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
fn contract_tests_sdk_reports_max_retries_as_the_first_supplied_sync_http_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                max_retries: Some(constants::DEFAULT_OTLP_MAX_RETRIES),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.max_retries",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                max_retries: Some(constants::DEFAULT_OTLP_MAX_RETRIES + 1),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.max_retries",
    );
}

#[test]
fn contract_tests_sdk_reports_initial_backoff_as_the_first_supplied_sync_http_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                initial_backoff_ms: Some(constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS.into()),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.initial_backoff_ms",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                initial_backoff_ms: Some((constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS + 1).into()),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.initial_backoff_ms",
    );
}

#[test]
fn contract_tests_sdk_reports_max_backoff_as_the_first_supplied_sync_http_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                max_backoff_ms: Some(constants::DEFAULT_OTLP_MAX_BACKOFF_MS.into()),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.max_backoff_ms",
    );
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                max_backoff_ms: Some((constants::DEFAULT_OTLP_MAX_BACKOFF_MS + 1).into()),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.max_backoff_ms",
    );
}

#[test]
fn contract_tests_disabled_rejects_explicit_default_retained_field() {
    let error = validated_transport_bounds(&OtelConfig {
        sync_http_retry: Some(SyncHttpRetryPolicy {
            max_retries: Some(constants::DEFAULT_OTLP_MAX_RETRIES),
            ..SyncHttpRetryPolicy::default()
        }),
        ..OtelConfig::default()
    })
    .expect_err("disabled transport must not silently accept an explicit retry policy");
    assert!(matches!(
        error,
        ConfigFailure::ConfigFieldNotApplicable { .. }
    ));
    assert_eq!(
        error.diagnostic().details["field"].as_str(),
        Some("sync_http_retry.max_retries")
    );
    assert_eq!(
        error.diagnostic().details["target"].as_str(),
        Some("disabled")
    );
}

#[test]
fn contract_tests_sdk_reports_retry_jitter_as_the_first_supplied_sync_http_field() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                retry_jitter_percent: Some(constants::DEFAULT_OTLP_RETRY_JITTER_PERCENT),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.retry_jitter_percent",
    );
}

#[test]
fn contract_tests_sdk_reports_retry_sequence_timeout_before_later_wire_fields() {
    assert_sdk_not_applicable_field(
        &OtelConfig {
            sync_http_retry: Some(SyncHttpRetryPolicy {
                retry_sequence_timeout_ms: Some(
                    constants::DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS.into(),
                ),
                retry_after_cap_ms: Some(constants::DEFAULT_OTLP_RETRY_AFTER_CAP_MS.into()),
                retry_jitter_percent: Some(constants::DEFAULT_OTLP_RETRY_JITTER_PERCENT),
                ..SyncHttpRetryPolicy::default()
            }),
            ..sdk_config()
        },
        "sync_http_retry.retry_sequence_timeout_ms",
    );
}

#[test]
fn contract_tests_record_and_byte_capacity() {
    let records = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(0),
        ..sync_http_config()
    })
    .expect_err("zero records is invalid");
    assert!(matches!(
        records,
        ConfigFailure::InvalidQueueCapacity { .. }
    ));
    assert_eq!(records.diagnostic().code, otlp::OTLP_CONFIG_QUEUE_CAPACITY);

    let bytes = validated_transport_bounds(&OtelConfig {
        queue_byte_capacity: Some(0),
        ..sync_http_config()
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
        ..sync_http_config()
    })
    .expect("record and byte upper bounds are accepted");
    assert_eq!(
        upper_bounds.queue_capacity().get(),
        constants::MAX_OTLP_QUEUE_CAPACITY
    );
    assert_eq!(
        upper_bounds.queue_byte_capacity().get(),
        constants::MAX_OTLP_QUEUE_BYTE_CAPACITY
    );

    let records_overflow = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(constants::MAX_OTLP_QUEUE_CAPACITY + 1),
        ..sync_http_config()
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
        ..sync_http_config()
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

#[test]
fn contract_tests_bound_messages_use_named_constants() {
    let records = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(constants::MAX_OTLP_QUEUE_CAPACITY + 1),
        ..sync_http_config()
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
        sync_http_retry: Some(SyncHttpRetryPolicy {
            retry_jitter_percent: Some(constants::MAX_OTLP_RETRY_JITTER_PERCENT + 1),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
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
        sync_http_retry: Some(SyncHttpRetryPolicy {
            retry_jitter_percent: Some(101),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("jitter above 100 is invalid");
    assert!(matches!(jitter, ConfigFailure::InvalidJitterPercent { .. }));

    let insecure = validated_transport_bounds(&OtelConfig {
        insecure_skip_verify: true,
        ..sync_http_config()
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
        Some("sync_http")
    );

    let shutdown_before_retry_bound = validated_transport_bounds(&OtelConfig {
        lifecycle_shutdown_timeout_ms: Some(2_999_u64.into()),
        sync_http_retry: Some(SyncHttpRetryPolicy {
            retry_sequence_timeout_ms: Some(1_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("shared shutdown ordering precedes sync-http retry ordering");
    assert!(matches!(
        shutdown_before_retry_bound,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    assert_eq!(
        shutdown_before_retry_bound.diagnostic().details["field"].as_str(),
        Some("timeout_ms")
    );
    assert_eq!(
        shutdown_before_retry_bound.diagnostic().details["upper_field"].as_str(),
        Some("lifecycle_shutdown_timeout_ms")
    );

    let sequence_below_timeout = validated_transport_bounds(&OtelConfig {
        sync_http_retry: Some(SyncHttpRetryPolicy {
            retry_sequence_timeout_ms: Some(2_999_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("retry sequence timeout may not be below the request timeout");
    assert!(matches!(
        sequence_below_timeout,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    let details = &sequence_below_timeout.diagnostic().details;
    assert_eq!(details["field"].as_str(), Some("timeout_ms"));
    assert_eq!(
        details["upper_field"].as_str(),
        Some("sync_http_retry.retry_sequence_timeout_ms")
    );
    assert_eq!(details["lower_value"].as_u64(), Some(3_000));
    assert_eq!(details["upper_value"].as_u64(), Some(2_999));

    let retry_after_cap = validated_transport_bounds(&OtelConfig {
        sync_http_retry: Some(SyncHttpRetryPolicy {
            retry_sequence_timeout_ms: Some(3_000_u64.into()),
            retry_after_cap_ms: Some(3_001_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("retry-after cap may not exceed retry sequence timeout");
    assert!(matches!(
        retry_after_cap,
        ConfigFailure::InvalidBoundOrdering { .. }
    ));
    assert_eq!(
        retry_after_cap.diagnostic().details["field"].as_str(),
        Some("sync_http_retry.retry_after_cap_ms")
    );
}

#[test]
fn contract_tests_bracketed_ipv6_endpoint_host_is_validated() {
    let error = super::config::OtlpEndpoint::new_typed("https://[not-an-ip]:4318")
        .expect_err("a bracketed non-IPv6 host must be rejected");
    assert!(matches!(error, ConfigFailure::InvalidEndpoint { .. }));
    assert_eq!(error.diagnostic().code, otlp::OTLP_CONFIG_INVALID_ENDPOINT);

    super::config::OtlpEndpoint::new_typed("https://[::1]:4318")
        .expect("a bracketed IPv6 literal is valid");
}

fn retry(policy: SyncHttpRetryPolicy) -> Option<SyncHttpRetryPolicy> {
    Some(policy)
}

fn assert_bound_ordering(error: &ConfigFailure, field: &str, upper_field: &str) {
    assert!(
        matches!(error, ConfigFailure::InvalidBoundOrdering { .. }),
        "expected InvalidBoundOrdering; got {error:?}"
    );
    let details = &error.diagnostic().details;
    assert_eq!(details["field"].as_str(), Some(field));
    assert_eq!(details["upper_field"].as_str(), Some(upper_field));
}

/// One case per adjacent pair of the owned first-failure order in
/// docs/api-design.md; each config violates both neighbours and asserts the
/// earlier one is reported.
#[test]
fn contract_tests_first_failure_order_for_each_adjacent_pair() {
    // 1. retry positivity precedes the shutdown ordering.
    let error = validated_transport_bounds(&OtelConfig {
        lifecycle_shutdown_timeout_ms: Some(2_999_u64.into()),
        sync_http_retry: retry(SyncHttpRetryPolicy {
            initial_backoff_ms: Some(0_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("zero initial backoff is reported first");
    assert!(matches!(error, ConfigFailure::ZeroDuration { .. }));

    // 2. the shutdown ordering precedes the flush ordering.
    let error = validated_transport_bounds(&OtelConfig {
        lifecycle_shutdown_timeout_ms: Some(2_999_u64.into()),
        lifecycle_flush_timeout_ms: Some(2_999_u64.into()),
        ..sync_http_config()
    })
    .expect_err("shutdown ordering is reported first");
    assert_bound_ordering(&error, "timeout_ms", "lifecycle_shutdown_timeout_ms");

    // 3. the flush ordering precedes queue capacity.
    let error = validated_transport_bounds(&OtelConfig {
        lifecycle_flush_timeout_ms: Some(2_999_u64.into()),
        queue_capacity: Some(0),
        ..sync_http_config()
    })
    .expect_err("flush ordering is reported first");
    assert_bound_ordering(&error, "timeout_ms", "lifecycle_flush_timeout_ms");

    // 4. queue capacity precedes the backoff ordering.
    let error = validated_transport_bounds(&OtelConfig {
        queue_capacity: Some(0),
        sync_http_retry: retry(SyncHttpRetryPolicy {
            max_backoff_ms: Some(100_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("queue capacity is reported first");
    assert!(matches!(error, ConfigFailure::InvalidQueueCapacity { .. }));

    // 5. the backoff ordering precedes the sequence ordering.
    let error = validated_transport_bounds(&OtelConfig {
        sync_http_retry: retry(SyncHttpRetryPolicy {
            max_backoff_ms: Some(100_u64.into()),
            retry_sequence_timeout_ms: Some(2_999_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("backoff ordering is reported first");
    assert_bound_ordering(
        &error,
        "sync_http_retry.initial_backoff_ms",
        "sync_http_retry.max_backoff_ms",
    );

    // 6. the sequence ordering precedes the Retry-After cap ordering.
    let error = validated_transport_bounds(&OtelConfig {
        sync_http_retry: retry(SyncHttpRetryPolicy {
            retry_sequence_timeout_ms: Some(2_999_u64.into()),
            retry_after_cap_ms: Some(3_001_u64.into()),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("sequence ordering is reported first");
    assert_bound_ordering(
        &error,
        "timeout_ms",
        "sync_http_retry.retry_sequence_timeout_ms",
    );

    // 7. the Retry-After cap ordering precedes the jitter bound.
    let error = validated_transport_bounds(&OtelConfig {
        sync_http_retry: retry(SyncHttpRetryPolicy {
            retry_sequence_timeout_ms: Some(3_000_u64.into()),
            retry_after_cap_ms: Some(3_001_u64.into()),
            retry_jitter_percent: Some(101),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("cap ordering is reported first");
    assert_bound_ordering(
        &error,
        "sync_http_retry.retry_after_cap_ms",
        "sync_http_retry.retry_sequence_timeout_ms",
    );

    // 8. the jitter bound precedes the insecure-transport rejection.
    let error = validated_transport_bounds(&OtelConfig {
        insecure_skip_verify: true,
        sync_http_retry: retry(SyncHttpRetryPolicy {
            retry_jitter_percent: Some(101),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sync_http_config()
    })
    .expect_err("jitter bound is reported first");
    assert!(matches!(error, ConfigFailure::InvalidJitterPercent { .. }));

    // 9. field applicability precedes the insecure-transport rejection.
    let error = validated_transport_bounds(&OtelConfig {
        insecure_skip_verify: true,
        sync_http_retry: retry(SyncHttpRetryPolicy {
            max_retries: Some(1),
            ..SyncHttpRetryPolicy::default()
        }),
        ..sdk_config()
    })
    .expect_err("applicability is reported first");
    assert!(matches!(
        error,
        ConfigFailure::ConfigFieldNotApplicable { .. }
    ));
}

#[test]
fn contract_tests_fake_exporter_contract() {
    let fixture = recording_exporter_set::<
        ExportRecord<LogRecord>,
        ExportRecord<CompleteSpan>,
        ExportRecord<MetricRecord>,
    >();
    fixture
        .exporters
        .logs
        .export_logs(&[])
        .expect("record logs");
    fixture
        .exporters
        .traces
        .export_spans(&[])
        .expect("record spans");
    fixture
        .exporters
        .metrics
        .export_metrics(&[])
        .expect("record metrics");
    fixture
        .exporters
        .lifecycle
        .blocking_preflight()
        .expect("record preflight");
    fixture
        .exporters
        .lifecycle
        .flush_blocking()
        .expect("record flush");

    assert_eq!(*fixture.logs.calls.lock().expect("calls poisoned"), vec![0]);
    assert_eq!(
        *fixture.traces.calls.lock().expect("calls poisoned"),
        vec![0]
    );
    assert_eq!(
        *fixture.metrics.calls.lock().expect("calls poisoned"),
        vec![0]
    );
    assert_eq!(
        fixture.logs.batches.lock().expect("batches poisoned").len(),
        1
    );
    assert_eq!(
        fixture
            .traces
            .batches
            .lock()
            .expect("batches poisoned")
            .len(),
        1
    );
    assert_eq!(
        fixture
            .metrics
            .batches
            .lock()
            .expect("batches poisoned")
            .len(),
        1
    );
    assert_eq!(
        *fixture.lifecycle.calls.lock().expect("calls poisoned"),
        vec![
            LifecycleCall::BlockingPreflight,
            LifecycleCall::FlushBlocking
        ]
    );
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
    let recording = recording_exporter_set::<
        ExportRecord<LogRecord>,
        ExportRecord<CompleteSpan>,
        ExportRecord<MetricRecord>,
    >();
    let exporters: ExporterSet = ExporterSet {
        logs: recording.exporters.logs,
        traces: Arc::new(CheckTrace(span.clone())),
        metrics: Arc::new(CheckMetric(metric.clone())),
        lifecycle: recording.exporters.lifecycle,
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

#[test]
fn released_checked_delays_preserve_zero_without_weakening_canonical_validation() {
    for timeout in [1_000_u64, 60_001] {
        let budget = timeout.max(30_000);
        let transport = OtelConfig {
            timeout_ms: Some(timeout.into()),
            lifecycle_flush_timeout_ms: Some(budget.into()),
            lifecycle_shutdown_timeout_ms: Some(budget.into()),
            sync_http_retry: Some(SyncHttpRetryPolicy {
                initial_backoff_ms: Some(0_u64.into()),
                max_backoff_ms: Some(0_u64.into()),
                retry_sequence_timeout_ms: Some(budget.into()),
                ..SyncHttpRetryPolicy::default()
            }),
            endpoint: Some(
                super::config::OtlpEndpoint::new_typed("http://127.0.0.1:4318").unwrap(),
            ),
            ..sync_http_config()
        };
        assert!(matches!(
            validated_transport_bounds(&transport),
            Err(ConfigFailure::ZeroDuration { .. })
        ));
        let config = super::config::TelemetryConfig {
            service_name: sc_observability_types::ServiceName::new("released-bounds").unwrap(),
            resource: super::config::ResourceAttributes::default(),
            transport,
            logs: Some(super::config::LogsConfig::default()),
            traces: None,
            metrics: None,
        };
        let bounds = super::config::validated_released_telemetry_bounds(&config).unwrap();
        let BackendTransportBounds::SyncHttp(retry) = bounds.backend() else {
            panic!("sync-http bounds")
        };
        assert_eq!(retry.initial_backoff().get(), std::time::Duration::ZERO);
        assert_eq!(retry.max_backoff().get(), std::time::Duration::ZERO);
        assert_eq!(
            bounds.request_timeout().get(),
            std::time::Duration::from_millis(timeout)
        );
        assert_eq!(
            bounds.lifecycle().flush().get(),
            std::time::Duration::from_millis(budget)
        );
        assert_eq!(
            bounds.lifecycle().shutdown().get(),
            std::time::Duration::from_millis(budget)
        );
        assert_eq!(
            retry.sequence_timeout().get(),
            std::time::Duration::from_millis(budget)
        );
        assert!(super::config::prepared_backend_connection(&config.transport, &bounds).is_ok());
    }
}
