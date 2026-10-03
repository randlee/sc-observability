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
    ValidatedBackendConnection, ValidatedTransportBounds, invalid_endpoint,
    prepared_backend_connection,
};
use crate::contracts::{
    self, ExportRecord, ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter, LogRecord,
    MetricExporter, TraceExporter,
};
#[allow(
    unused_imports,
    reason = "transport construction failures are mapped only by enabled backends"
)]
use crate::legacy_projection::transport_construction_failure;
#[cfg(feature = "otlp-sdk")]
use crate::sdk;
#[cfg(feature = "sync-http")]
use crate::sync_http;

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
    let connection = prepare_backend_connection(config, bounds)?;
    exporter_factory_prepared(bounds, connection.as_ref())
}

/// Prepares the raw connection fields exactly once, after bounds validation.
/// Disabled telemetry deliberately bypasses endpoint, authentication, and CA
/// inspection because it constructs no backend connection.
pub(crate) fn prepare_backend_connection(
    config: &RuntimeTelemetryConfig,
    bounds: &ValidatedTransportBounds,
) -> Result<Option<ValidatedBackendConnection>, ConfigFailure> {
    match bounds.backend() {
        BackendTransportBounds::Disabled => Ok(None),
        BackendTransportBounds::Sdk | BackendTransportBounds::SyncHttp(_) => {
            prepared_backend_connection(&config.transport, bounds).map(Some)
        }
    }
}

pub(crate) fn exporter_factory_prepared(
    bounds: &ValidatedTransportBounds,
    connection: Option<&ValidatedBackendConnection>,
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
            sdk_exporter_factory(bounds, connection.ok_or_else(missing_endpoint)?)
        }
        BackendTransportBounds::SyncHttp(_) => {
            sync_http_exporter_factory(bounds, connection.ok_or_else(missing_endpoint)?)
        }
    }
}

fn missing_endpoint() -> ConfigFailure {
    invalid_endpoint(
        "enabled telemetry requires an endpoint",
        "set OtelConfig.endpoint before constructing the backend",
    )
}

#[cfg_attr(
    not(feature = "otlp-sdk"),
    expect(
        unused_variables,
        reason = "the prepared connection is consumed only when the otlp-sdk feature is enabled"
    )
)]
fn sdk_exporter_factory(
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

#[cfg_attr(
    not(feature = "sync-http"),
    expect(
        unused_variables,
        reason = "the prepared connection is consumed only when the sync-http feature is enabled"
    )
)]
fn sync_http_exporter_factory(
    bounds: &ValidatedTransportBounds,
    connection: &ValidatedBackendConnection,
) -> Result<ExporterSet, ConfigFailure> {
    if bounds.protocol() != config::OtlpProtocol::HttpJson {
        return Err(unsupported_protocol(
            config::ExporterBackend::SyncHttp,
            bounds.protocol(),
            "HttpJson",
        ));
    }

    #[cfg(feature = "sync-http")]
    {
        sync_http::build_exporter_set(connection, bounds).map_err(transport_construction_failure)
    }

    #[cfg(not(feature = "sync-http"))]
    Err(unsupported_backend(
        config::ExporterBackend::SyncHttp,
        "sync-http",
        "the sync-http feature is disabled",
    ))
}

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

#[cfg(any(not(feature = "otlp-sdk"), not(feature = "sync-http")))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_factory_signatures_accept_validated_inputs_only() {
        let _: fn(
            &ValidatedTransportBounds,
            Option<&ValidatedBackendConnection>,
        ) -> Result<ExporterSet, ConfigFailure> = exporter_factory_prepared;

        #[cfg(feature = "otlp-sdk")]
        let _: fn(
            &ValidatedTransportBounds,
            &ValidatedBackendConnection,
        ) -> Result<ExporterSet, ConfigFailure> = sdk_exporter_factory;

        #[cfg(feature = "sync-http")]
        let _: fn(
            &ValidatedTransportBounds,
            &ValidatedBackendConnection,
        ) -> Result<ExporterSet, ConfigFailure> = sync_http_exporter_factory;
    }

    #[test]
    fn enabled_bounds_without_a_prepared_connection_use_the_canonical_endpoint_failure() {
        let mut transport = config::OtelConfig::new(
            config::ExporterBackend::SyncHttp,
            config::OtlpProtocol::HttpJson,
        );
        transport.enabled = true;
        let bounds = config::validated_transport_bounds(&transport)
            .expect("enabled transport bounds do not inspect the endpoint");

        let Err(actual) = exporter_factory_prepared(&bounds, None) else {
            panic!("an enabled backend requires a prepared connection");
        };
        let expected = missing_endpoint();
        assert_eq!(actual.diagnostic().code, expected.diagnostic().code);
        assert_eq!(actual.diagnostic().message, expected.diagnostic().message);
        assert_eq!(
            actual.diagnostic().remediation,
            expected.diagnostic().remediation
        );
    }
}
