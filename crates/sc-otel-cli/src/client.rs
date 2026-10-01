//! Opens the production durable client or the feature-gated test double.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed delivery diagnostics"
)]

#[cfg(not(feature = "test-double"))]
use sc_observability_otlp::durable::DurableTelemetryClient;
#[cfg(feature = "test-double")]
use sc_observability_types::otlp::submission::TelemetryConfigError;
use sc_observability_types::otlp::submission::{
    TelemetryClient, TelemetryClientConfig, TelemetryClientError,
};
#[cfg(feature = "test-double")]
use sc_observability_types::{
    ErrorContext, Remediation,
    otlp::submission::error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
};

#[cfg(feature = "test-double")]
use crate::constants;

pub(crate) fn open_client(
    config: TelemetryClientConfig,
) -> Result<Box<dyn TelemetryClient>, TelemetryClientError> {
    #[cfg(feature = "test-double")]
    {
        open_test_double(config)
    }

    #[cfg(not(feature = "test-double"))]
    {
        DurableTelemetryClient::open(config)
            .map(|client| Box::new(client) as Box<dyn TelemetryClient>)
    }
}

#[cfg(feature = "test-double")]
fn open_test_double(
    config: TelemetryClientConfig,
) -> Result<Box<dyn TelemetryClient>, TelemetryClientError> {
    use sc_observability_types::otlp::submission::testing::{
        DoubleScript, InMemoryTelemetryClient,
    };

    let script = match std::env::var_os(constants::TEST_DOUBLE_ENV) {
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            let text = std::fs::read_to_string(&path).map_err(|source| {
                TelemetryConfigError::ConfigFile {
                    path,
                    context: Box::new(ErrorContext::new(
                        SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
                        format!("unable to read test double script: {source}"),
                        Remediation::not_recoverable(
                            "provide a readable SC_OTEL_TEST_DOUBLE JSON file",
                        ),
                    )),
                }
            })?;
            DoubleScript::from_json(&text)?
        }
        None => DoubleScript::default(),
    };
    Ok(Box::new(InMemoryTelemetryClient::with_script(
        config, script,
    )))
}
