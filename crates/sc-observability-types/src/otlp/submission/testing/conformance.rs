//! Shared behavior tests implemented against a caller-provided client harness.
use super::{DeliveryOutcome, Signal};
use crate::otlp::submission::{
    DeliveryError, LogInput, SpanInput, StatusQuery, SubmissionEnvelope, SubmissionInput,
    SystemIds, TelemetryClient, TelemetryClientError,
};
use std::time::Duration;
const DEADLINE: Duration = Duration::from_millis(10);
/// Supplies isolated client state and controllable export outcomes.
pub trait ConformanceHarness {
    /// Client under test.
    type Client: TelemetryClient;
    /// Opens a fresh empty store/client.
    fn open(&mut self) -> Self::Client;
    /// Selects the next export outcome for this signal.
    fn set_outcome(&mut self, signal: Signal, outcome: DeliveryOutcome);
}
fn log(key: &str) -> SubmissionEnvelope {
    let mut input = SubmissionInput::new();
    input.record_key = Some(key.parse().expect("valid fixture key"));
    input.logs.push(LogInput::new());
    SubmissionEnvelope::from_input(input, &mut SystemIds::new()).expect("valid log fixture")
}

fn duplicate_record_key_returns_original_receipt<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    let first = client.emit(log("duplicate")).unwrap();
    let duplicate = client.emit(log("duplicate")).unwrap();
    assert_eq!(first.submission_id, duplicate.submission_id);
    assert_eq!(first.admitted_at, duplicate.admitted_at);
    assert_eq!(first.signals, duplicate.signals);
    assert!(duplicate.duplicate);
    assert!(!first.duplicate);
}

fn status_reports_scripted_failure<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    harness.set_outcome(Signal::Logs, DeliveryOutcome::Fail);
    client.emit(log("failed")).unwrap();
    assert!(client.flush(DEADLINE).is_err());
    assert_eq!(client.status(StatusQuery::Summary).unwrap().failed.logs, 1);
}

fn flush_ok_when_all_delivered<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    client.emit(log("delivered")).unwrap();
    let report = client.flush(DEADLINE).unwrap();
    assert_eq!(report.delivered.logs, 1);
    assert_eq!(report.still_pending.total(), 0);
}

fn flush_deadline_when_pending<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    harness.set_outcome(Signal::Logs, DeliveryOutcome::Stall);
    client.emit(log("pending")).unwrap();
    match client.flush(DEADLINE).unwrap_err() {
        TelemetryClientError::Delivery(DeliveryError::DeadlineExceeded { report, .. }) => {
            assert_eq!(report.still_pending.logs, 1);
        }
        error => panic!("expected deadline: {error}"),
    }
}

fn flush_terminal_when_failed_during_call<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    harness.set_outcome(Signal::Logs, DeliveryOutcome::Fail);
    client.emit(log("failed")).unwrap();
    match client.flush(DEADLINE).unwrap_err() {
        TelemetryClientError::Delivery(DeliveryError::TerminalFailure { report, .. }) => {
            assert_eq!(report.failed.logs, 1);
        }
        error => panic!("expected terminal: {error}"),
    }
}

fn flush_ignores_historical_failure<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    harness.set_outcome(Signal::Logs, DeliveryOutcome::Fail);
    client.emit(log("old")).unwrap();
    assert!(client.flush(DEADLINE).is_err());
    client.emit(log("new")).unwrap();
    let report = client.flush(DEADLINE).unwrap();
    assert_eq!(report.failed.logs, 0);
    assert_eq!(report.delivered.logs, 1);
}

fn terminal_precedes_deadline<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    harness.set_outcome(Signal::Logs, DeliveryOutcome::Fail);
    harness.set_outcome(Signal::Traces, DeliveryOutcome::Stall);
    client.emit(log("failed")).unwrap();
    let mut input = SubmissionInput::new();
    let mut span = SpanInput::new("pending".into(), crate::Timestamp::UNIX_EPOCH);
    span.duration_nanos = Some(1);
    input.spans.push(span);
    client
        .emit(SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap())
        .unwrap();
    match client.flush(DEADLINE).unwrap_err() {
        TelemetryClientError::Delivery(DeliveryError::TerminalFailure { report, .. }) => {
            assert_eq!(report.failed.logs, 1);
            assert_eq!(report.still_pending.traces, 1);
        }
        error => panic!("expected terminal before deadline: {error}"),
    }
}

fn flush_submission_scoped_to_id<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    let first = client.emit(log("first")).unwrap();
    let second = client.emit(log("second")).unwrap();
    let first_report = client
        .flush_submission(&first.submission_id, DEADLINE)
        .unwrap();
    assert_eq!(first_report.delivered.logs, 1);
    let second_report = client
        .flush_submission(&second.submission_id, DEADLINE)
        .unwrap();
    assert_eq!(second_report.delivered.logs, 1);
}

fn flush_submission_reports_prior_failure_of_same_submission<H: ConformanceHarness>(
    harness: &mut H,
) {
    let client = harness.open();
    harness.set_outcome(Signal::Logs, DeliveryOutcome::Fail);
    let receipt = client.emit(log("failed")).unwrap();
    assert!(client.flush(DEADLINE).is_err());
    assert!(matches!(
        client.flush_submission(&receipt.submission_id, DEADLINE),
        Err(TelemetryClientError::Delivery(
            DeliveryError::TerminalFailure { .. }
        ))
    ));
}

fn shutdown_idempotent<H: ConformanceHarness>(harness: &mut H) {
    let client = harness.open();
    client.emit(log("shutdown")).unwrap();
    assert_eq!(client.shutdown(DEADLINE).unwrap().delivered.logs, 1);
    assert_eq!(
        client.shutdown(DEADLINE).unwrap(),
        crate::otlp::submission::FlushReport::default()
    );
    assert!(client.emit(log("closed")).is_err());
}
/// Runs all ten lifecycle cases against isolated clients.
/// # Panics
/// Panics if the client violates a lifecycle contract; intended for tests.
pub fn run_all<H: ConformanceHarness>(harness: &mut H) {
    duplicate_record_key_returns_original_receipt(harness);
    status_reports_scripted_failure(harness);
    flush_ok_when_all_delivered(harness);
    flush_deadline_when_pending(harness);
    flush_terminal_when_failed_during_call(harness);
    flush_ignores_historical_failure(harness);
    terminal_precedes_deadline(harness);
    flush_submission_scoped_to_id(harness);
    flush_submission_reports_prior_failure_of_same_submission(harness);
    shutdown_idempotent(harness);
}
