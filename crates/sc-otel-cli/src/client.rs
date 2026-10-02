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
    record_call(
        &serde_json::json!({"method":"open", "endpoint":config.endpoint, "store_path":config.store_path}),
    );
    Ok(Box::new(RecordingTestClient {
        inner: InMemoryTelemetryClient::with_script(config, script),
        record_path: std::env::var_os(constants::TEST_DOUBLE_RECORD_ENV)
            .map(std::path::PathBuf::from),
    }))
}

/// Feature-gated process-test witness for the exact envelope accepted by the double.
#[cfg(feature = "test-double")]
struct RecordingTestClient {
    inner: sc_observability_types::otlp::submission::testing::InMemoryTelemetryClient,
    record_path: Option<std::path::PathBuf>,
}

#[cfg(feature = "test-double")]
impl TelemetryClient for RecordingTestClient {
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError>
    where
        Self: Sized,
    {
        use sc_observability_types::otlp::submission::testing::InMemoryTelemetryClient;

        Ok(Self {
            inner: InMemoryTelemetryClient::open(config)?,
            record_path: std::env::var_os(constants::TEST_DOUBLE_RECORD_ENV)
                .map(std::path::PathBuf::from),
        })
    }

    fn emit(
        &self,
        envelope: sc_observability_types::otlp::submission::SubmissionEnvelope,
    ) -> Result<sc_observability_types::otlp::submission::AdmissionReceipt, TelemetryClientError>
    {
        if let Some(path) = &self.record_path {
            let encoded = serde_json::to_vec(&envelope)
                .expect("the canonical submission envelope is serializable");
            std::fs::write(path, encoded).expect("test-double envelope record writes");
        }
        self.inner.emit(envelope)
    }

    fn flush(
        &self,
        deadline: std::time::Duration,
    ) -> Result<sc_observability_types::otlp::submission::FlushReport, TelemetryClientError> {
        record_call(&serde_json::json!({"method":"flush", "deadline_ms":deadline.as_millis()}));
        self.inner.flush(deadline)
    }

    fn flush_submission(
        &self,
        id: &sc_observability_types::otlp::submission::SubmissionId,
        deadline: std::time::Duration,
    ) -> Result<sc_observability_types::otlp::submission::FlushReport, TelemetryClientError> {
        self.inner.flush_submission(id, deadline)
    }

    fn shutdown(
        &self,
        deadline: std::time::Duration,
    ) -> Result<sc_observability_types::otlp::submission::FlushReport, TelemetryClientError> {
        self.inner.shutdown(deadline)
    }

    fn status(
        &self,
        query: sc_observability_types::otlp::submission::StatusQuery,
    ) -> Result<sc_observability_types::otlp::submission::StoreStatus, TelemetryClientError> {
        use sc_observability_types::otlp::submission::StatusQuery;
        let value = match &query {
            StatusQuery::Summary => serde_json::json!({"kind":"summary"}),
            StatusQuery::RecordKeys(keys) => {
                serde_json::json!({"kind":"record_keys", "keys": keys.iter().map(ToString::to_string).collect::<Vec<_>>()})
            }
            StatusQuery::Submissions(ids) => {
                serde_json::json!({"kind":"submissions", "ids":ids.iter().map(ToString::to_string).collect::<Vec<_>>()})
            }
            _ => serde_json::json!({"kind":"unknown"}),
        };
        record_call(&serde_json::json!({"method":"status", "query":value}));
        self.inner.status(query)
    }
}

/// Separate process-test witness preserves the existing envelope-only record.
#[cfg(feature = "test-double")]
fn record_call(call: &serde_json::Value) {
    use std::io::Write;
    if let Some(path) = std::env::var_os(constants::TEST_DOUBLE_RECORD_ENV) {
        let path = std::path::PathBuf::from(path).with_extension("calls.jsonl");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("test witness opens");
        serde_json::to_writer(&mut file, call).expect("test witness serializes");
        writeln!(file).expect("test witness writes");
    }
}
