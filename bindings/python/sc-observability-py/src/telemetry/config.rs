//! Constructor and selector JSON transport into shared Rust configuration.
use sc_observability_otlp::durable::load_telemetry_file;
use sc_observability_types::otlp::submission::{
    ConfigOverrides, ConfigSources, StatusQuery, SubmissionError, SubmissionId,
    TelemetryClientError, TelemetryConfigError, resolve_config,
};
use serde::Deserialize;
use std::{path::PathBuf, str::FromStr};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructorArgs {
    config: Option<PathBuf>,
    store_path: Option<PathBuf>,
    endpoint: Option<String>,
    service_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StatusArgs {
    Summary(String),
    Submissions { submissions: Vec<String> },
    RecordKeys { record_keys: Vec<String> },
}
pub(super) fn config(
    args_json: &str,
) -> Result<sc_observability_types::otlp::submission::TelemetryClientConfig, TelemetryClientError> {
    let args: ConstructorArgs =
        serde_json::from_str(args_json).map_err(|error| TelemetryConfigError::InvalidField {
            field: "args",
            context: Box::new(
                sc_observability_types::ErrorContext::new(
                    sc_observability_types::error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
                    "invalid telemetry constructor arguments",
                    sc_observability_types::Remediation::not_recoverable(
                        "correct constructor arguments",
                    ),
                )
                .cause(error.to_string()),
            ),
        })?;
    let file = args
        .config
        .as_deref()
        .map(load_telemetry_file)
        .transpose()?;
    let mut overrides = ConfigOverrides::default();
    overrides.store_path = args.store_path;
    overrides.endpoint = args.endpoint;
    overrides.service_name = args.service_name;
    let env = |key: &str| std::env::var(key).ok();
    resolve_config(ConfigSources::new(&overrides, file.as_ref(), &env)).map_err(Into::into)
}
pub(super) fn status_query(query_json: &str) -> Result<StatusQuery, TelemetryClientError> {
    match serde_json::from_str::<StatusArgs>(query_json) {
        Ok(StatusArgs::Summary(value)) if value == "summary" => Ok(StatusQuery::Summary),
        Ok(StatusArgs::Submissions { submissions }) => submissions
            .into_iter()
            .map(|value| SubmissionId::from_str(&value))
            .collect::<Result<Vec<_>, _>>()
            .map(StatusQuery::Submissions)
            .map_err(Into::into),
        Ok(StatusArgs::RecordKeys { record_keys }) => record_keys
            .into_iter()
            .map(|value| value.parse())
            .collect::<Result<Vec<_>, SubmissionError>>()
            .map(StatusQuery::RecordKeys)
            .map_err(Into::into),
        Ok(StatusArgs::Summary(value)) => {
            Err(invalid_query(format!("unknown status selector: {value}")))
        }
        Err(error) => Err(invalid_query(error.to_string())),
    }
}
fn invalid_query(cause: String) -> TelemetryClientError {
    TelemetryConfigError::InvalidField {
        field: "query",
        context: Box::new(
            sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
                "invalid telemetry status query",
                sc_observability_types::Remediation::not_recoverable(
                    "supply summary, submissions, or record_keys",
                ),
            )
            .cause(cause),
        ),
    }
    .into()
}
