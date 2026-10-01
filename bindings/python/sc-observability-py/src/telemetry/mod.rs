//! Python transport for the shared telemetry submission contract.

use pyo3::prelude::*;
use sc_observability_otlp::durable::{DurableTelemetryClient, load_telemetry_file};
#[cfg(feature = "test-hooks")]
use sc_observability_types::otlp::submission::testing::{DoubleScript, InMemoryTelemetryClient};
use sc_observability_types::otlp::submission::{
    AdmissionError, ConfigOverrides, ConfigSources, DeliveryError, StatusQuery, SubmissionEnvelope,
    SubmissionError, SubmissionId, SystemIds, TelemetryClient, TelemetryClientError,
    TelemetryConfigError, resolve_config,
};
use serde::Deserialize;
use serde_json::json;
use std::{path::PathBuf, str::FromStr, time::Duration};

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

enum ClientHandle {
    Durable(DurableTelemetryClient),
    #[cfg(feature = "test-hooks")]
    Double(Box<InMemoryTelemetryClient>),
}

macro_rules! call_client {
    ($self:expr, $method:ident $(, $arg:expr )* ) => {
        match &$self.client {
            ClientHandle::Durable(client) => client.$method($($arg),*),
            #[cfg(feature = "test-hooks")]
            ClientHandle::Double(client) => client.$method($($arg),*),
        }
    };
}

#[pyclass(frozen)]
struct NativeTelemetry {
    client: ClientHandle,
    flush_deadline: Duration,
}

fn ok(value: impl serde::Serialize) -> String {
    serde_json::to_string(&json!({"kind": "ok", "value": value}))
        .unwrap_or_else(|_| internal("failed to serialize telemetry result"))
}

fn internal(message: &str) -> String {
    json!({"kind":"error","error":{"kind":"internal","variant":"panic","code":"SC_OBSERVABILITY_BINDING_INTERNAL","message":message,"path":null,"report":null}}).to_string()
}

fn failure(error: &TelemetryClientError) -> String {
    let (kind, variant, path, report) = match &error {
        TelemetryClientError::Submission(error) => (
            "submission",
            submission_variant(error),
            submission_path(error),
            None,
        ),
        TelemetryClientError::Admission(error) => {
            ("admission", admission_variant(error), None, None)
        }
        TelemetryClientError::Delivery(error) => (
            "delivery",
            delivery_variant(error),
            None,
            delivery_report(error),
        ),
        TelemetryClientError::Config(error) => ("config", config_variant(error), None, None),
        _ => ("internal", "unknown", None, None),
    };
    json!({"kind":"error","error":{"kind":kind,"variant":variant,"code":error.code().as_str(),"message":error.to_string(),"path":path,"report":report}}).to_string()
}

fn submission_variant(error: &SubmissionError) -> &'static str {
    match error {
        SubmissionError::InvalidJson { .. } => "invalid_json",
        SubmissionError::UnsupportedVersion { .. } => "unsupported_version",
        SubmissionError::EmptySubmission { .. } => "empty_submission",
        SubmissionError::Validation { .. } => "validation",
        SubmissionError::ValueOutOfRange { .. } => "value_out_of_range",
        SubmissionError::CorrelationConflict { .. } => "correlation_conflict",
        SubmissionError::TimingConflict { .. } => "timing_conflict",
        SubmissionError::DictionaryReference { .. } => "dictionary_reference",
        _ => "unknown",
    }
}
fn submission_path(error: &SubmissionError) -> Option<&str> {
    match error {
        SubmissionError::Validation { path, .. }
        | SubmissionError::ValueOutOfRange { path, .. }
        | SubmissionError::DictionaryReference { path, .. } => Some(path),
        _ => None,
    }
}
fn admission_variant(error: &AdmissionError) -> &'static str {
    match error {
        AdmissionError::StoreUnavailable { .. } => "store_unavailable",
        AdmissionError::DiskBoundExceeded { .. } => "disk_bound_exceeded",
        AdmissionError::Persistence { .. } => "persistence",
        AdmissionError::SchemaTooNew { .. } => "schema_too_new",
        AdmissionError::Closed { .. } => "closed",
        _ => "unknown",
    }
}
fn delivery_variant(error: &DeliveryError) -> &'static str {
    match error {
        DeliveryError::DeadlineExceeded { .. } => "deadline_exceeded",
        DeliveryError::TerminalFailure { .. } => "terminal_failure",
        _ => "unknown",
    }
}
fn delivery_report(
    error: &DeliveryError,
) -> Option<&sc_observability_types::otlp::submission::FlushReport> {
    match error {
        DeliveryError::DeadlineExceeded { report, .. }
        | DeliveryError::TerminalFailure { report, .. } => Some(report),
        _ => None,
    }
}
fn config_variant(error: &TelemetryConfigError) -> &'static str {
    match error {
        TelemetryConfigError::ConfigFile { .. } => "config_file",
        TelemetryConfigError::MissingField { .. } => "missing_field",
        TelemetryConfigError::InvalidField { .. } => "invalid_field",
        TelemetryConfigError::UnsupportedCombination { .. } => "unsupported_combination",
        _ => "unknown",
    }
}

fn config(
    args_json: &str,
) -> Result<sc_observability_types::otlp::submission::TelemetryClientConfig, TelemetryClientError> {
    let args: ConstructorArgs =
        serde_json::from_str(args_json).map_err(|_| TelemetryConfigError::InvalidField {
            field: "args",
            context: Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::ErrorCode::new_static(
                    "SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID",
                ),
                "invalid telemetry constructor arguments",
                sc_observability_types::Remediation::not_recoverable(
                    "correct constructor arguments",
                ),
            )),
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

fn envelope(input_json: &str) -> Result<SubmissionEnvelope, TelemetryClientError> {
    let mut ids = SystemIds::new();
    SubmissionEnvelope::from_json(input_json, &mut ids).map_err(Into::into)
}

#[pyfunction]
fn open(py: Python<'_>, args_json: &str) -> PyResult<(Option<Py<NativeTelemetry>>, String)> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        py.detach(|| {
            let config = config(args_json)?;
            let flush_deadline = config.flush_deadline;
            let client = DurableTelemetryClient::open(config)?;
            Ok::<_, TelemetryClientError>((client, flush_deadline))
        })
    }));
    match result {
        Ok(Ok((client, flush_deadline))) => Ok((
            Some(Py::new(
                py,
                NativeTelemetry {
                    client: ClientHandle::Durable(client),
                    flush_deadline,
                },
            )?),
            ok(()),
        )),
        Ok(Err(error)) => Ok((None, failure(&error))),
        Err(_) => Ok((None, internal("native telemetry open panicked"))),
    }
}

#[cfg(feature = "test-hooks")]
#[pyfunction]
fn _open_test_double(
    py: Python<'_>,
    args_json: &str,
    script_json: Option<&str>,
) -> PyResult<(Option<Py<NativeTelemetry>>, String)> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        py.detach(|| {
            let config = config(args_json)?;
            let script = script_json
                .map_or_else(|| Ok(DoubleScript::default()), DoubleScript::from_json)
                .map_err(TelemetryClientError::from)?;
            let flush_deadline = config.flush_deadline;
            Ok::<_, TelemetryClientError>((
                InMemoryTelemetryClient::with_script(config, script),
                flush_deadline,
            ))
        })
    }));
    match result {
        Ok(Ok((client, flush_deadline))) => Ok((
            Some(Py::new(
                py,
                NativeTelemetry {
                    client: ClientHandle::Double(Box::new(client)),
                    flush_deadline,
                },
            )?),
            ok(()),
        )),
        Ok(Err(error)) => Ok((None, failure(&error))),
        Err(_) => Ok((None, internal("native telemetry test factory panicked"))),
    }
}

#[pyfunction]
fn build_envelope(input_json: &str) -> String {
    std::panic::catch_unwind(|| envelope(input_json).map(|value| value.to_canonical_json()))
        .map_or_else(
            |_| internal("native envelope conversion panicked"),
            |result| result.map_or_else(|error| failure(&error), ok),
        )
}

fn status_query(query_json: &str) -> Result<StatusQuery, TelemetryClientError> {
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
        _ => Err(TelemetryConfigError::InvalidField {
            field: "query",
            context: Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::ErrorCode::new_static(
                    "SC_OBSERVABILITY_TELEMETRY_QUERY_INVALID",
                ),
                "invalid telemetry status query",
                sc_observability_types::Remediation::not_recoverable(
                    "supply summary, submissions, or record_keys",
                ),
            )),
        }
        .into()),
    }
}

#[pymethods]
impl NativeTelemetry {
    fn emit(&self, py: Python<'_>, input_json: &str) -> String {
        let input = input_json.to_owned();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            py.detach(|| envelope(&input).and_then(|value| call_client!(self, emit, value)))
        }))
        .map_or_else(
            |_| internal("native emit panicked"),
            |result| result.map_or_else(|error| failure(&error), ok),
        )
    }
    fn flush(&self, py: Python<'_>, timeout_ms: Option<u64>) -> String {
        let deadline = Duration::from_millis(timeout_ms.unwrap_or_else(|| {
            self.flush_deadline
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
        }));
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            py.detach(|| match &self.client {
                ClientHandle::Durable(client) => client.flush(deadline),
                #[cfg(feature = "test-hooks")]
                ClientHandle::Double(client) => client.flush(deadline),
            })
        }))
        .map_or_else(
            |_| internal("native flush panicked"),
            |result| result.map_or_else(|error| failure(&error), ok),
        )
    }
    fn flush_submission(
        &self,
        py: Python<'_>,
        submission_id: &str,
        timeout_ms: Option<u64>,
    ) -> String {
        let id = SubmissionId::from_str(submission_id).map_err(TelemetryClientError::from);
        let deadline = Duration::from_millis(timeout_ms.unwrap_or_else(|| {
            self.flush_deadline
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
        }));
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            py.detach(|| id.and_then(|id| call_client!(self, flush_submission, &id, deadline)))
        }))
        .map_or_else(
            |_| internal("native submission flush panicked"),
            |result| result.map_or_else(|error| failure(&error), ok),
        )
    }
    fn shutdown(&self, py: Python<'_>, timeout_ms: Option<u64>) -> String {
        let deadline = Duration::from_millis(timeout_ms.unwrap_or_else(|| {
            self.flush_deadline
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
        }));
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            py.detach(|| match &self.client {
                ClientHandle::Durable(client) => client.shutdown(deadline),
                #[cfg(feature = "test-hooks")]
                ClientHandle::Double(client) => client.shutdown(deadline),
            })
        }))
        .map_or_else(
            |_| internal("native shutdown panicked"),
            |result| result.map_or_else(|error| failure(&error), ok),
        )
    }
    fn status(&self, py: Python<'_>, query_json: &str) -> String {
        match status_query(query_json) {
            Ok(query) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                py.detach(|| match &self.client {
                    ClientHandle::Durable(client) => client.status(query),
                    #[cfg(feature = "test-hooks")]
                    ClientHandle::Double(client) => client.status(query),
                })
            }))
            .map_or_else(
                |_| internal("native status panicked"),
                |result| result.map_or_else(|error| failure(&error), ok),
            ),
            Err(error) => failure(&error),
        }
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeTelemetry>()?;
    module.add_function(wrap_pyfunction!(open, module)?)?;
    module.add_function(wrap_pyfunction!(build_envelope, module)?)?;
    #[cfg(feature = "test-hooks")]
    module.add_function(wrap_pyfunction!(_open_test_double, module)?)?;
    Ok(())
}
