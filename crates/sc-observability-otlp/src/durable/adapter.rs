//! Configuration adapter signature staged for D33.
use crate::config::OtelConfig;
use sc_observability_types::otlp::submission::{
    TelemetryClientConfig, TelemetryConfigError, error_codes,
};
pub(crate) fn otel_config_from(
    _config: &TelemetryClientConfig,
) -> Result<OtelConfig, TelemetryConfigError> {
    Err(TelemetryConfigError::InvalidField {
        field: "otlp",
        context: super::context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
            "durable OTLP adapter is not implemented yet",
        ),
    })
}
