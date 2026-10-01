//! Deterministic scheduling around the real flush snapshot, not fabricated reports.
use super::*;
use sc_observability_types::otlp::submission::testing::conformance::ConformanceHarness;

thread_local! { static ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(in crate::durable) fn enabled() -> bool {
    ENABLED.get()
}

pub(in crate::durable) fn await_scripted_outcomes(
    shared: &Shared,
    reader: &rusqlite::Connection,
    scope: &query::Scope,
) {
    let start = Instant::now();
    loop {
        let generation = shared.generation();
        let non_stalled: query::Scope = {
            let stalled = shared.stalled_signals.lock().unwrap();
            scope
                .iter()
                .filter(|(_, signal)| !stalled.contains(signal))
                .cloned()
                .collect()
        };
        if query::report(reader, &non_stalled)
            .unwrap()
            .still_pending
            .total()
            == 0
        {
            return;
        }
        let wake = shared.wake.lock().unwrap();
        let (_guard, result) = shared
            .changed
            .wait_timeout_while(wake, DEADLINE.saturating_sub(start.elapsed()), |value| {
                *value == generation
            })
            .unwrap();
        assert!(
            !result.timed_out(),
            "conformance drain timed out: scope={scope:?}; non-stalled={non_stalled:?}"
        );
    }
}

// The second run supplies zero instead of ten milliseconds to the same real methods.
// It proves all shared cases work with the planned zero-deadline suite as well.
struct Client<const ZERO_DEADLINE: bool>(DurableTelemetryClient);
impl<const ZERO_DEADLINE: bool> Client<ZERO_DEADLINE> {
    fn deadline(value: Duration) -> Duration {
        if ZERO_DEADLINE { Duration::ZERO } else { value }
    }
}
impl<const ZERO_DEADLINE: bool> TelemetryClient for Client<ZERO_DEADLINE> {
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> {
        DurableTelemetryClient::open(config).map(Self)
    }
    fn emit(&self, envelope: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError> {
        self.0.emit(envelope)
    }
    fn flush(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        self.0.flush(Self::deadline(deadline))
    }
    fn flush_submission(
        &self,
        id: &SubmissionId,
        deadline: Duration,
    ) -> Result<FlushReport, TelemetryClientError> {
        self.0.flush_submission(id, Self::deadline(deadline))
    }
    fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        self.0.shutdown(Self::deadline(deadline))
    }
    fn status(&self, query: StatusQuery) -> Result<StoreStatus, TelemetryClientError> {
        self.0.status(query)
    }
}

struct Harness<const ZERO_DEADLINE: bool> {
    dirs: Vec<tempfile::TempDir>,
    exporters: Vec<Arc<ScriptedExporter>>,
}
impl<const ZERO_DEADLINE: bool> Drop for Harness<ZERO_DEADLINE> {
    fn drop(&mut self) {
        for exporter in &self.exporters {
            exporter.release();
        }
        for exporter in &self.exporters {
            if let Some(shared) = exporter
                .conformance_shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .upgrade()
            {
                worker::join(&shared, DEADLINE);
            }
        }
    }
}
impl<const ZERO_DEADLINE: bool> ConformanceHarness for Harness<ZERO_DEADLINE> {
    type Client = Client<ZERO_DEADLINE>;
    fn open(&mut self) -> Self::Client {
        let dir = tempfile::tempdir().unwrap();
        let exporter = Arc::new(ScriptedExporter::new(dir.path()));
        ENABLED.set(true);
        let opened =
            DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter.clone());
        ENABLED.set(false);
        let client = opened.unwrap();
        *exporter.conformance_shared.lock().unwrap() = Arc::downgrade(&client.owner.shared);
        self.dirs.push(dir);
        self.exporters.push(exporter);
        Client(client)
    }
    fn set_outcome(&mut self, signal: Signal, outcome: DeliveryOutcome) {
        self.exporters.last().unwrap().set_outcome(signal, outcome);
    }
}
#[test]
fn shared_cases() {
    sc_observability_types::otlp::submission::testing::conformance::run_all(
        &mut Harness::<false> {
            dirs: vec![],
            exporters: vec![],
        },
    );
}

#[test]
fn shared_cases_with_zero_deadline() {
    sc_observability_types::otlp::submission::testing::conformance::run_all(&mut Harness::<true> {
        dirs: vec![],
        exporters: vec![],
    });
}
