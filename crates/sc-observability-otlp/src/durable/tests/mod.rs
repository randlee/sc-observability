//! Tests inject a scripted exporter at the same seam used by production.
use super::*;
use crate::contracts::submission::SubmissionExportFailure;
use sc_observability_types::otlp::submission::*;
use sc_observability_types::{otlp::submission::testing::DeliveryOutcome, v2::ExportError};
use std::{
    collections::{HashMap, VecDeque},
    io::Write,
    path::{Path, PathBuf},
};
mod backpressure;
mod capability;
pub(super) mod conformance;
mod drain;
const DEADLINE: Duration = Duration::from_secs(3);

#[derive(Default)]
struct ScriptedExporter {
    outcomes: Mutex<HashMap<Signal, VecDeque<DeliveryOutcome>>>,
    released: Mutex<bool>,
    gate: Condvar,
    deliveries: PathBuf,
    conformance_shared: std::sync::Weak<Shared>,
}
impl ScriptedExporter {
    fn new(path: &Path) -> Self {
        Self {
            deliveries: path.join("deliveries.jsonl"),
            ..Self::default()
        }
    }
    fn set_outcome(&self, signal: Signal, outcome: DeliveryOutcome) {
        self.outcomes
            .lock()
            .unwrap()
            .entry(signal)
            .or_default()
            .push_back(outcome);
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.gate.notify_all();
    }
}
fn export_error() -> ExportError {
    ExportError::Transport {
        context: context(
            error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
            "scripted failure",
        ),
    }
}
impl SubmissionExporter for ScriptedExporter {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure> {
        let outcome = self
            .outcomes
            .lock()
            .unwrap()
            .get_mut(&signal)
            .and_then(VecDeque::pop_front)
            .unwrap_or(DeliveryOutcome::Deliver);
        match outcome {
            DeliveryOutcome::Fail => return Err(SubmissionExportFailure::Terminal(export_error())),
            DeliveryOutcome::Stall => {
                if let Some(shared) = self.conformance_shared.upgrade() {
                    shared.stalled_signals.lock().unwrap().insert(signal);
                    shared.notify();
                }
                let _guard = self
                    .gate
                    .wait_while(self.released.lock().unwrap(), |released| !*released)
                    .unwrap();
            }
            _ => {}
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.deliveries)
            .unwrap();
        for envelope in envelopes {
            let line = serde_json::json!({"signal": store::signal_name(signal), "key": envelope.record_key}).to_string() + "\n";
            file.write_all(line.as_bytes()).unwrap();
        }
        file.sync_all().unwrap();
        Ok(())
    }
}
fn config(path: &Path) -> TelemetryClientConfig {
    let mut overrides = ConfigOverrides::default();
    overrides.store_path = Some(path.join("telemetry.db"));
    overrides.lease_duration = Some(Duration::from_millis(300));
    resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap()
}
fn log(key: &str) -> SubmissionEnvelope {
    let mut input = SubmissionInput::new();
    input.record_key = Some(key.parse().unwrap());
    input.logs.push(LogInput::new());
    SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap()
}
fn fixture(name: &str) -> SubmissionEnvelope {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sc-observability-types/tests/fixtures/otlp_submission/golden")
        .join(name)
        .join("expected.envelope.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn wait_until(mut predicate: impl FnMut() -> bool) {
    let start = Instant::now();
    while !predicate() {
        assert!(start.elapsed() < DEADLINE, "condition did not become true");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn duplicate_record_key() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let first = store::admit(&mut db, &config, &log("same")).unwrap();
    let duplicate = store::admit(&mut db, &config, &log("same")).unwrap();
    assert_eq!(first.submission_id, duplicate.submission_id);
    assert_eq!(first.admitted_at, duplicate.admitted_at);
    assert!(duplicate.duplicate);
    assert_eq!(
        db.query_row("SELECT count(*) FROM submissions", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn each_signal_alone_and_combined_gets_rows() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let mut combined = fixture("logs");
    combined.record_key = None;
    combined.spans = fixture("traces").spans;
    combined.metrics = fixture("metric_gauge").metrics;
    combined.profiles = fixture("profiles").profiles;
    for mut envelope in [
        fixture("logs"),
        fixture("traces"),
        fixture("metric_gauge"),
        fixture("profiles"),
        combined,
    ] {
        envelope.record_key = None;
        let expected = envelope.signals().iter().count();
        let receipt = store::admit(&mut db, &config, &envelope).unwrap();
        assert_eq!(
            query::snapshot(&db, Some(&receipt.submission_id))
                .unwrap()
                .len(),
            expected
        );
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM signal_deliveries", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        8
    );
}
#[test]
fn schema_too_new_rejected_unmodified() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.db");
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA user_version=99; CREATE TABLE untouched(value TEXT); INSERT INTO untouched VALUES('keep');").unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        store::open(&path),
        Err(TelemetryClientError::Admission(
            AdmissionError::SchemaTooNew { found: 99, .. }
        ))
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn newer_envelope_skipped_and_counted() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let receipt = store::admit(&mut db, &config, &log("newer")).unwrap();
    db.execute(
        "UPDATE submissions SET envelope_version=99,envelope=x'ffff'",
        [],
    )
    .unwrap();
    drop(db);
    let exporter = Arc::new(ScriptedExporter::new(dir.path()));
    let client = DurableTelemetryClient::open_with_exporter(config, exporter.clone()).unwrap();
    assert!(matches!(
        client.flush_submission(&receipt.submission_id, Duration::from_millis(20)),
        Err(TelemetryClientError::Delivery(
            DeliveryError::DeadlineExceeded { .. }
        ))
    ));
    assert_eq!(
        client
            .status(StatusQuery::Summary)
            .unwrap()
            .unreadable_newer_envelopes,
        1
    );
    assert!(!exporter.deliveries.exists());
}
