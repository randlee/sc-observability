//! D.21 contract tests. They use staged private traits and checked bounds
//! directly, without requiring either production backend.

use std::sync::Arc;

use super::config::{
    BackendTransportBounds, ExporterBackend, LegacyRetryPolicy, OtelConfig, OtlpProtocol,
    validated_transport_bounds,
};
use super::contracts::{
    ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter, MetricExporter, TraceExporter,
};
use super::{CompleteSpan, LogEvent, MetricRecord};
use sc_observability_types::error_codes::otlp;
use sc_observability_types::typed::ExportFailure;
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
        timeout_ms: 0_u64.into(),
        queue_capacity: Some(0),
        ..legacy_config()
    };
    let error = validated_transport_bounds(&config).expect_err("timeout is checked first");
    assert!(matches!(error, ConfigFailure::ZeroDuration { .. }));
    assert_eq!(error.diagnostic().code, otlp::OTLP_CONFIG_ZERO_DURATION);
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
fn contract_tests_stable_failure_codes() {
    let zero = validated_transport_bounds(&OtelConfig {
        timeout_ms: 0_u64.into(),
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
    fn export_logs(&self, _batch: &[LogEvent]) -> Result<(), ExportFailure> {
        Ok(())
    }
}
struct FakeTrace;
impl TraceExporter for FakeTrace {
    fn export_spans(&self, _batch: &[CompleteSpan]) -> Result<(), ExportFailure> {
        Ok(())
    }
}
struct FakeMetric;
impl MetricExporter for FakeMetric {
    fn export_metrics(&self, _batch: &[MetricRecord]) -> Result<(), ExportFailure> {
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
