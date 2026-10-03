//! Deterministic scheduling around the real flush snapshot, not fabricated reports.
use super::*;
use sc_observability_types::otlp::submission::testing::conformance::ConformanceHarness;

thread_local! { static ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(in crate::durable) fn enabled() -> bool {
    ENABLED.get()
}

// Opens a client without autonomous workers: its drains run only inside its own
// flush calls, on this thread, under the frozen test clock, so a lease can never
// lapse between a claim and its result transaction.
pub(super) fn open_manual(
    open: impl FnOnce() -> Result<DurableTelemetryClient, TelemetryClientError>,
) -> Result<DurableTelemetryClient, TelemetryClientError> {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ENABLED.set(false);
        }
    }
    ENABLED.set(true);
    let _reset = Reset;
    open()
}

pub(super) fn open_gated(
    config: TelemetryClientConfig,
    exporter: &Arc<ScriptedExporter>,
) -> DurableTelemetryClient {
    let client =
        open_manual(|| DurableTelemetryClient::open_with_exporter(config, exporter.clone()))
            .unwrap();
    *exporter.conformance_shared.lock().unwrap() = Arc::downgrade(&client.owner.shared);
    client
}

pub(in crate::durable) fn await_scripted_outcomes(
    shared: &Shared,
    exporter: &dyn SubmissionExporter,
    reader: &rusqlite::Connection,
    scope: &query::Scope,
) {
    let _clock = FrozenClock::new();
    // The real flush snapshot has already been taken. Drive only its signals;
    // scripted Stall leaves its real admitted rows pending without a live sleeper.
    for signal in [
        Signal::Logs,
        Signal::Traces,
        Signal::Metrics,
        Signal::Profiles,
    ] {
        if shared.stalled_signals.lock().unwrap().contains(&signal) {
            continue;
        }
        let selected: query::Scope = scope
            .iter()
            .filter(|(_, candidate)| *candidate == signal)
            .cloned()
            .collect();
        while query::report(reader, &selected)
            .unwrap()
            .still_pending
            .total()
            != 0
        {
            assert!(
                drain_once_bounded(shared, exporter, signal),
                "scripted drain made no progress for {signal:?}"
            );
        }
    }
}

// The second run supplies zero instead of ten milliseconds to the same real methods.
// It proves all shared cases work with the planned zero-deadline suite as well.
struct Client<const ZERO_DEADLINE: bool>(DurableTelemetryClient, Option<Arc<ScriptedExporter>>);
impl<const ZERO_DEADLINE: bool> Drop for Client<ZERO_DEADLINE> {
    fn drop(&mut self) {
        // Release this case's blocked exporter before later cases run. The harness
        // retains exporters for cleanup, but must not retain the blocking lifetime.
        if let Some(exporter) = &self.1 {
            exporter.release();
        }
    }
}
impl<const ZERO_DEADLINE: bool> Client<ZERO_DEADLINE> {
    fn deadline(value: Duration) -> Duration {
        if ZERO_DEADLINE { Duration::ZERO } else { value }
    }
}
impl<const ZERO_DEADLINE: bool> TelemetryClient for Client<ZERO_DEADLINE> {
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> {
        DurableTelemetryClient::open(config).map(|client| Self(client, None))
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
        let client = open_gated(config(dir.path()), &exporter);
        self.dirs.push(dir);
        self.exporters.push(exporter.clone());
        Client(client, Some(exporter))
    }
    fn set_outcome(&mut self, signal: Signal, outcome: DeliveryOutcome) {
        let exporter = self.exporters.last().unwrap();
        exporter.set_outcome(signal, outcome);
        if matches!(outcome, DeliveryOutcome::Stall) {
            exporter
                .conformance_shared
                .lock()
                .unwrap()
                .upgrade()
                .unwrap()
                .stalled_signals
                .lock()
                .unwrap()
                .insert(signal);
        }
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
