//! Opens the production durable client or the unit-test-only double.
#![expect(
    clippy::result_large_err,
    reason = "the shared telemetry error preserves typed delivery diagnostics"
)]

#[cfg(not(test))]
use sc_observability_otlp::durable::DurableTelemetryClient;
#[cfg(test)]
use sc_observability_types::otlp::submission::TelemetryConfigError;
use sc_observability_types::otlp::submission::{
    TelemetryClient, TelemetryClientConfig, TelemetryClientError,
};
#[cfg(test)]
use sc_observability_types::{
    ErrorContext, Remediation,
    otlp::submission::error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
};

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub(crate) struct UnitClientPaths {
    pub(crate) script: Option<std::path::PathBuf>,
    pub(crate) record: Option<std::path::PathBuf>,
}

pub(crate) fn open_client(
    config: TelemetryClientConfig,
    #[cfg(test)] paths: Option<&UnitClientPaths>,
) -> Result<Box<dyn TelemetryClient>, TelemetryClientError> {
    #[cfg(test)]
    {
        open_test_client(config, paths.cloned().unwrap_or_default())
    }

    #[cfg(not(test))]
    {
        DurableTelemetryClient::open(config)
            .map(|client| Box::new(client) as Box<dyn TelemetryClient>)
    }
}

pub(crate) fn open_read_only_client(
    config: TelemetryClientConfig,
    #[cfg(test)] paths: Option<&UnitClientPaths>,
) -> Result<Box<dyn TelemetryClient>, TelemetryClientError> {
    #[cfg(test)]
    {
        open_test_client(config, paths.cloned().unwrap_or_default())
    }

    #[cfg(not(test))]
    {
        DurableTelemetryClient::open_read_only(config)
            .map(|client| Box::new(client) as Box<dyn TelemetryClient>)
    }
}

#[cfg(test)]
fn open_test_client(
    config: TelemetryClientConfig,
    paths: UnitClientPaths,
) -> Result<Box<dyn TelemetryClient>, TelemetryClientError> {
    use sc_observability_types::otlp::submission::testing::{
        DoubleScript, InMemoryTelemetryClient,
    };

    let script = match paths.script {
        Some(path) => {
            let text = std::fs::read_to_string(&path).map_err(|source| {
                TelemetryConfigError::ConfigFile {
                    path,
                    context: Box::new(ErrorContext::new(
                        SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
                        format!("unable to read test double script: {source}"),
                        Remediation::not_recoverable(
                            "provide a readable unit-client script JSON file",
                        ),
                    )),
                }
            })?;
            DoubleScript::from_json(&text)?
        }
        None => DoubleScript::default(),
    };
    record_call(
        paths.record.as_deref(),
        &serde_json::json!({"method":"open", "endpoint":config.endpoint, "store_path":config.store_path}),
    );
    Ok(Box::new(RecordingTestClient {
        inner: InMemoryTelemetryClient::with_script(config, script),
        record_path: paths.record,
    }))
}

/// Unit-test witness for the exact envelope accepted by the double.
#[cfg(test)]
struct RecordingTestClient {
    inner: sc_observability_types::otlp::submission::testing::InMemoryTelemetryClient,
    record_path: Option<std::path::PathBuf>,
}

#[cfg(test)]
impl TelemetryClient for RecordingTestClient {
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError>
    where
        Self: Sized,
    {
        use sc_observability_types::otlp::submission::testing::InMemoryTelemetryClient;

        Ok(Self {
            inner: InMemoryTelemetryClient::open(config)?,
            record_path: None,
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
            std::fs::write(path, encoded).expect("unit-client envelope record writes");
        }
        self.inner.emit(envelope)
    }

    fn flush(
        &self,
        deadline: std::time::Duration,
    ) -> Result<sc_observability_types::otlp::submission::FlushReport, TelemetryClientError> {
        record_call(
            self.record_path.as_deref(),
            &serde_json::json!({"method":"flush", "deadline_ms":deadline.as_millis()}),
        );
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
        record_call(
            self.record_path.as_deref(),
            &serde_json::json!({"method":"status", "query":value}),
        );
        self.inner.status(query)
    }
}

/// Separate unit-test witness preserves the existing envelope-only record.
#[cfg(test)]
fn record_call(record_path: Option<&std::path::Path>, call: &serde_json::Value) {
    use std::io::Write;
    if let Some(path) = record_path {
        let path = path.with_extension("calls.jsonl");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("test witness opens");
        serde_json::to_writer(&mut file, call).expect("test witness serializes");
        writeln!(file).expect("test witness writes");
    }
}
