//! Python ownership and GIL release around the shared client.
use super::{
    config::{config, status_query},
    dto,
};
use pyo3::prelude::*;
use sc_observability_otlp::durable::DurableTelemetryClient;
#[cfg(feature = "test-hooks")]
use sc_observability_types::otlp::submission::testing::{DoubleScript, InMemoryTelemetryClient};
use sc_observability_types::otlp::submission::{
    SubmissionEnvelope, SubmissionId, SystemIds, TelemetryClient, TelemetryClientError,
};
use std::{str::FromStr, time::Duration};

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
pub(super) struct NativeTelemetry {
    client: ClientHandle,
    flush_deadline: Duration,
    #[cfg(feature = "test-hooks")]
    gate: super::test_gate::FlushGate,
}
impl NativeTelemetry {
    fn new(client: ClientHandle, flush_deadline: Duration) -> Self {
        Self {
            client,
            flush_deadline,
            #[cfg(feature = "test-hooks")]
            gate: super::test_gate::FlushGate::default(),
        }
    }
    fn deadline(&self, timeout_ms: Option<u64>) -> Duration {
        timeout_ms.map_or(self.flush_deadline, Duration::from_millis)
    }
}
fn envelope(input: &str) -> Result<SubmissionEnvelope, TelemetryClientError> {
    SubmissionEnvelope::from_json(input, &mut SystemIds::new()).map_err(Into::into)
}
fn factory(
    py: Python<'_>,
    action: impl FnOnce() -> Result<NativeTelemetry, TelemetryClientError> + Send,
) -> PyResult<(Option<Py<NativeTelemetry>>, String)> {
    match dto::contained(|| py.detach(action)) {
        Ok(client) => Ok((Some(Py::new(py, client)?), dto::ok(()))),
        Err(error) => Ok((None, error)),
    }
}
#[pyfunction]
pub(super) fn open(
    py: Python<'_>,
    args_json: &str,
) -> PyResult<(Option<Py<NativeTelemetry>>, String)> {
    factory(py, || {
        let config = config(args_json)?;
        let deadline = config.flush_deadline;
        Ok(NativeTelemetry::new(
            ClientHandle::Durable(DurableTelemetryClient::open(config)?),
            deadline,
        ))
    })
}
#[cfg(feature = "test-hooks")]
#[pyfunction]
pub(super) fn _open_test_double(
    py: Python<'_>,
    args_json: &str,
    script_json: Option<&str>,
) -> PyResult<(Option<Py<NativeTelemetry>>, String)> {
    factory(py, || {
        let config = config(args_json)?;
        let script =
            script_json.map_or_else(|| Ok(DoubleScript::default()), DoubleScript::from_json)?;
        let deadline = config.flush_deadline;
        Ok(NativeTelemetry::new(
            ClientHandle::Double(Box::new(InMemoryTelemetryClient::with_script(
                config, script,
            ))),
            deadline,
        ))
    })
}
#[pyfunction]
pub(super) fn build_envelope(input_json: &str) -> String {
    dto::result(|| envelope(input_json).map(|value| value.to_canonical_json()))
}
#[pymethods]
impl NativeTelemetry {
    fn emit(&self, py: Python<'_>, input_json: &str) -> String {
        dto::result(|| {
            py.detach(|| envelope(input_json).and_then(|value| call_client!(self, emit, value)))
        })
    }
    fn flush(&self, py: Python<'_>, timeout_ms: Option<u64>) -> String {
        dto::result(|| {
            py.detach(|| {
                #[cfg(feature = "test-hooks")]
                self.gate.block();
                call_client!(self, flush, self.deadline(timeout_ms))
            })
        })
    }
    fn flush_submission(
        &self,
        py: Python<'_>,
        submission_id: &str,
        timeout_ms: Option<u64>,
    ) -> String {
        dto::result(|| {
            py.detach(|| {
                let id = SubmissionId::from_str(submission_id)?;
                call_client!(self, flush_submission, &id, self.deadline(timeout_ms))
            })
        })
    }
    fn shutdown(&self, py: Python<'_>, timeout_ms: Option<u64>) -> String {
        dto::result(|| py.detach(|| call_client!(self, shutdown, self.deadline(timeout_ms))))
    }
    fn status(&self, py: Python<'_>, query_json: &str) -> String {
        dto::result(|| {
            py.detach(|| {
                status_query(query_json).and_then(|query| call_client!(self, status, query))
            })
        })
    }
    #[cfg(feature = "test-hooks")]
    fn _gate_arm(&self) {
        self.gate.arm();
    }
    #[cfg(feature = "test-hooks")]
    fn _gate_wait_entered(&self, py: Python<'_>) -> bool {
        py.detach(|| self.gate.wait_entered())
    }
    #[cfg(feature = "test-hooks")]
    fn _gate_is_blocked(&self) -> bool {
        self.gate.is_blocked()
    }
    #[cfg(feature = "test-hooks")]
    fn _gate_release(&self) {
        self.gate.release();
    }
}
