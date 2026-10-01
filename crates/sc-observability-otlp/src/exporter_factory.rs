//! Exporter factory: selects the one exporter set for the validated backend,
//! including the explicit disabled-transport exporters.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sc_observability_types::v2::{
    ConfigFailure, ExportError, MetricRecord as CanonicalMetricRecord,
};
use sc_observability_types::{ErrorContext, Remediation};
use serde_json::Value;

use crate::config::{
    self, BackendTransportBounds, TelemetryConfig as RuntimeTelemetryConfig,
    ValidatedBackendConnection, ValidatedTransportBounds, prepared_backend_connection,
};
use crate::contracts::{
    self, ExportRecord, ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter, LogRecord,
    MetricExporter, TraceExporter,
};
#[cfg(feature = "legacy-http-json")]
use crate::legacy_http_json;
#[allow(
    unused_imports,
    reason = "transport construction failures are mapped only by enabled backends"
)]
use crate::legacy_projection::transport_construction_failure;
#[cfg(feature = "otlp-sdk")]
use crate::sdk;

/// Explicit disabled-transport exporters. They are never selected for an
/// enabled backend; the factory rejects enabled selections until D.6-D.8
/// supply their concrete exporter sets.
struct DisabledLogExporter;
struct DisabledTraceExporter;
struct DisabledMetricExporter;
struct DisabledLifecycle {
    shutdown: AtomicBool,
}

impl LogExporter for DisabledLogExporter {
    fn export_logs(&self, _batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        Ok(())
    }
}

impl TraceExporter for DisabledTraceExporter {
    fn export_spans(
        &self,
        _batch: &[ExportRecord<contracts::CompleteSpan>],
    ) -> Result<(), ExportError> {
        Ok(())
    }
}

impl MetricExporter for DisabledMetricExporter {
    fn export_metrics(
        &self,
        _batch: &[ExportRecord<CanonicalMetricRecord>],
    ) -> Result<(), ExportError> {
        Ok(())
    }
}

impl ExporterLifecycle for DisabledLifecycle {
    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        Box::pin(async { Ok(()) })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        self.shutdown.store(true, Ordering::Release);
        Box::pin(async { Ok(()) })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        self.shutdown.store(true, Ordering::Release);
        Ok(())
    }
}

/// Consumes only fully validated transport bounds before selecting one common
/// exporter shape. Protocol, feature, and caller-runtime availability are
/// deliberately checked here, after the configuration's normative ordered
/// validation, so an unavailable backend cannot mask a malformed config.
#[cfg(test)]
pub(crate) fn exporter_factory(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
) -> Result<ExporterSet, ConfigFailure> {
    exporter_factory_prepared(config, bounds)
}

pub(crate) fn exporter_factory_prepared(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
) -> Result<ExporterSet, ConfigFailure> {
    match bounds.backend() {
        BackendTransportBounds::Disabled => Ok(ExporterSet {
            logs: Arc::new(DisabledLogExporter),
            traces: Arc::new(DisabledTraceExporter),
            metrics: Arc::new(DisabledMetricExporter),
            lifecycle: Arc::new(DisabledLifecycle {
                shutdown: AtomicBool::new(false),
            }),
        }),
        BackendTransportBounds::Sdk => {
            let connection = prepared_backend_connection(&config.transport, bounds)?;
            sdk_exporter_factory(config, bounds, &connection)
        }
        BackendTransportBounds::Legacy(_) => {
            let connection = prepared_backend_connection(&config.transport, bounds)?;
            legacy_exporter_factory(config, bounds, &connection)
        }
    }
}

#[allow(unused_variables)]
fn sdk_exporter_factory(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
    connection: &ValidatedBackendConnection,
) -> Result<ExporterSet, ConfigFailure> {
    if !matches!(
        bounds.protocol(),
        config::OtlpProtocol::Grpc | config::OtlpProtocol::HttpBinary
    ) {
        return Err(unsupported_protocol(
            config::ExporterBackend::OpenTelemetrySdk,
            bounds.protocol(),
            "Grpc, HttpBinary",
        ));
    }

    #[cfg(feature = "otlp-sdk")]
    {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(ConfigFailure::TokioRuntimeRequired {
                context: Box::new(
                    ErrorContext::new(
                        sc_observability_types::error_codes::otlp::OTLP_TOKIO_RUNTIME_REQUIRED,
                        "the OpenTelemetry SDK backend must be constructed inside a Tokio runtime",
                        Remediation::recoverable(
                            "construct telemetry from the host Tokio runtime",
                            ["enable the otlp-sdk feature", "enter a Tokio runtime first"],
                        ),
                    )
                    .detail(
                        "backend",
                        Value::String(
                            config::ExporterBackend::OpenTelemetrySdk
                                .stable_name()
                                .to_owned(),
                        ),
                    )
                    .detail("feature", Value::String("otlp-sdk".to_owned()))
                    .detail("runtime", Value::String("caller-tokio".to_owned())),
                ),
            });
        }
        sdk::build_exporter_set(connection, bounds)
            .map(|adapter| adapter.exporters)
            .map_err(transport_construction_failure)
    }

    #[cfg(not(feature = "otlp-sdk"))]
    Err(unsupported_backend(
        config::ExporterBackend::OpenTelemetrySdk,
        "otlp-sdk",
        "the otlp-sdk feature is disabled",
    ))
}

#[allow(unused_variables)]
fn legacy_exporter_factory(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
    connection: &ValidatedBackendConnection,
) -> Result<ExporterSet, ConfigFailure> {
    if bounds.protocol() != config::OtlpProtocol::HttpJson {
        return Err(unsupported_protocol(
            config::ExporterBackend::LegacyHttpJson,
            bounds.protocol(),
            "HttpJson",
        ));
    }

    #[cfg(feature = "legacy-http-json")]
    {
        legacy_http_json::build_exporter_set(connection, bounds)
            .map_err(transport_construction_failure)
    }

    #[cfg(not(feature = "legacy-http-json"))]
    Err(unsupported_backend(
        config::ExporterBackend::LegacyHttpJson,
        "legacy-http-json",
        "the legacy-http-json feature is disabled",
    ))
}

#[allow(dead_code)]
fn unsupported_protocol(
    backend: config::ExporterBackend,
    protocol: config::OtlpProtocol,
    supported_protocols: &str,
) -> ConfigFailure {
    ConfigFailure::UnsupportedProtocol {
        context: Box::new(
            ErrorContext::new(
                sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_PROTOCOL,
                "the selected exporter backend does not support the configured protocol",
                Remediation::recoverable(
                    "select a protocol supported by the selected exporter backend",
                    ["select a documented backend/protocol combination"],
                ),
            )
            .detail("backend", Value::String(backend.stable_name().to_owned()))
            .detail("protocol", Value::String(protocol.stable_name().to_owned()))
            .detail(
                "supported_protocols",
                Value::String(supported_protocols.to_owned()),
            ),
        ),
    }
}

#[cfg(any(not(feature = "otlp-sdk"), not(feature = "legacy-http-json")))]
fn unsupported_backend(
    backend: config::ExporterBackend,
    feature: &str,
    availability: &str,
) -> ConfigFailure {
    ConfigFailure::UnsupportedBackend {
        context: Box::new(
            ErrorContext::new(
                sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_BACKEND,
                "enabled exporter backend is unavailable",
                Remediation::recoverable(
                    "enable the selected backend feature or select disabled telemetry",
                    ["enable the named feature", "disable telemetry"],
                ),
            )
            .detail("backend", Value::String(backend.stable_name().to_owned()))
            .detail("feature", Value::String(feature.to_owned()))
            .detail("availability", Value::String(availability.to_owned())),
        ),
    }
}
