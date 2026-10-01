//! Opens the production durable client or the feature-gated test double.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed delivery diagnostics"
)]

use sc_observability_otlp::durable::DurableTelemetryClient;
use sc_observability_types::otlp::submission::{
    TelemetryClient, TelemetryClientConfig, TelemetryClientError, TelemetryConfigError,
};
#[cfg(feature = "test-double")]
use sc_observability_types::{ErrorCode, ErrorContext, Remediation};

pub(crate) fn open_client(
    config: TelemetryClientConfig,
) -> Result<Box<dyn TelemetryClient>, TelemetryClientError> {
    #[cfg(feature = "test-double")]
    if let Some(path) = std::env::var_os("SC_OTEL_TEST_DOUBLE") {
        return open_test_double(config, path);
    }

    DurableTelemetryClient::open(config).map(|client| Box::new(client) as Box<dyn TelemetryClient>)
}

#[cfg(feature = "test-double")]
fn open_test_double(
    config: TelemetryClientConfig,
    path: std::ffi::OsString,
) -> Result<Box<dyn TelemetryClient>, TelemetryClientError> {
    use sc_observability_types::otlp::submission::testing::{
        DoubleScript, InMemoryTelemetryClient,
    };

    let path = std::path::PathBuf::from(path);
    let text = std::fs::read_to_string(&path).map_err(|_| TelemetryConfigError::ConfigFile {
        path,
        context: Box::new(ErrorContext::new(
            ErrorCode::new_static("SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE"),
            "unable to read test double script",
            Remediation::not_recoverable("provide a readable SC_OTEL_TEST_DOUBLE JSON file"),
        )),
    })?;
    let script = DoubleScript::from_json(&text)?;
    Ok(Box::new(InMemoryTelemetryClient::with_script(
        config, script,
    )))
}
