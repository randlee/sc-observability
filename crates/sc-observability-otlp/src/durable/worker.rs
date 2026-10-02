//! Lease-fenced per-signal workers; transport calls never hold a database lock.
//! The transport owns jitter, Retry-After and its sequence time budget. Durable
//! retry scheduling only bounds persisted attempts and exponential backoff;
//! it does not repeat the transport's in-sequence timing policy.
use super::{
    Shared, persistence,
    row::{LeaseHolder, RowState, UnixNanos},
    store,
};
use crate::contracts::submission::{SubmissionExportFailure, SubmissionExporter};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use sc_observability_types::otlp::submission::{
    EnvelopeVersion, Signal, SubmissionEnvelope, SubmissionId, SyncHttpRetryPolicyDto,
    TelemetryClientError,
};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

pub(super) fn start(
    shared: &Arc<Shared>,
    exporter: Arc<dyn SubmissionExporter>,
) -> Result<(), TelemetryClientError> {
    let heartbeat = Arc::clone(shared);
    let finished = Completion::new(shared);
    let handle = std::thread::Builder::new()
        .name("telemetry-lease".into())
        .spawn(move || {
            let _finished = finished;
            let mut failures = 0u32;
            while !heartbeat.stop.load(Ordering::Acquire) {
                #[cfg(test)]
                if heartbeat.drain_on_flush_only.load(Ordering::Acquire)
                    && heartbeat.active_flushes.load(Ordering::Acquire) == 0
                {
                    let generation = heartbeat.generation();
                    heartbeat.wait_since(generation, Shared::poll_interval());
                    continue;
                }
                let delay = match lease(&heartbeat, true) {
                    Ok(_) => {
                        failures = 0;
                        heartbeat.config.lease_duration / crate::constants::LEASE_RENEWAL_DIVISOR
                    }
                    Err(error) => {
                        heartbeat.record_error(&error);
                        failures = failures.saturating_add(1);
                        error_backoff(failures)
                    }
                };
                heartbeat.wait(delay);
            }
        })
        .map_err(persistence)?;
    shared
        .workers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(handle);
    let exporters = [
        Arc::clone(&exporter),
        Arc::clone(&exporter),
        Arc::clone(&exporter),
        exporter,
    ];
    for (signal, exporter) in [
        Signal::Logs,
        Signal::Traces,
        Signal::Metrics,
        Signal::Profiles,
    ]
    .into_iter()
    .zip(exporters)
    {
        let worker_shared = Arc::clone(shared);
        let finished = Completion::new(shared);
        let handle = std::thread::Builder::new()
            .name(format!("telemetry-{}", store::signal_name(signal)?))
            .spawn(move || {
                let _finished = finished;
                let shared = worker_shared;
                let mut failures = 0u32;
                while !shared.stop.load(Ordering::Acquire) {
                    let generation = shared.generation();
                    match drain(&shared, exporter.as_ref(), signal) {
                        Ok(progress) => {
                            failures = 0;
                            match progress {
                                DrainProgress::Exported => shared.notify(),
                                DrainProgress::Idle => {
                                    shared.wait_since(generation, Shared::poll_interval());
                                }
                                DrainProgress::Interrupted => {
                                    shared.notify();
                                    break;
                                }
                            }
                        }
                        Err(error) => {
                            shared.record_error(&error);
                            failures = failures.saturating_add(1);
                            shared.wait(error_backoff(failures));
                        }
                    }
                }
            })
            .map_err(persistence)?;
        shared
            .workers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(handle);
    }
    Ok(())
}
// The final worker releases its lease only after every in-flight export has had
// an opportunity to commit its result under the ownership fence.
struct Completion(Arc<Shared>);
impl Completion {
    fn new(shared: &Arc<Shared>) -> Self {
        shared.live_workers.fetch_add(1, Ordering::AcqRel);
        Self(Arc::clone(shared))
    }
}
impl Drop for Completion {
    fn drop(&mut self) {
        let mut wake = self
            .0
            .wake
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.0.live_workers.fetch_sub(1, Ordering::AcqRel) == 1 {
            release(&self.0);
        }
        *wake = wake.wrapping_add(1);
        self.0.changed.notify_all();
    }
}
pub(super) fn join(shared: &Shared, remaining: Duration) {
    let wake = shared
        .wake
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (wake, _) = shared
        .changed
        .wait_timeout_while(wake, remaining, |_| {
            shared.live_workers.load(Ordering::Acquire) != 0
        })
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    drop(wake);
    let mut handles = shared
        .workers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut index = 0;
    while index < handles.len() {
        if handles[index].is_finished() {
            let _ = handles.swap_remove(index).join();
        } else {
            index += 1;
        }
    }
}
pub(super) fn release(shared: &Shared) {
    if let Ok(db) = shared.db.try_lock() {
        // Best effort under the caller's deadline; a busy lease expires naturally.
        // The fencing predicate prevents an old holder deleting a successor's lease.
        let _ = db.busy_timeout(Duration::ZERO);
        let _ = db.execute(
            "DELETE FROM drain_lease WHERE id=1 AND holder=?1",
            [&shared.holder],
        );
    }
}
fn lease(shared: &Shared, renew: bool) -> Result<bool, TelemetryClientError> {
    let mut db = shared.db.lock()?;
    if !renew {
        let owned: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM drain_lease WHERE holder=?1 AND expires_at_unix_nano>=?2)", params![shared.holder,store::now()], |r| r.get(0)).map_err(persistence)?;
        if owned {
            return Ok(true);
        }
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(persistence)?;
    let now = store::now();
    let old: Option<(LeaseHolder, UnixNanos)> = tx
        .query_row(
            "SELECT holder,expires_at_unix_nano FROM drain_lease WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(persistence)?;
    if old
        .as_ref()
        .is_some_and(|(holder, expiry)| holder != &shared.holder && *expiry >= now)
    {
        return Ok(false);
    }
    if shared.stop.load(Ordering::Acquire) {
        return Ok(false);
    }
    let expiry = now.saturating_add(store::nanos(shared.config.lease_duration));
    if old
        .as_ref()
        .is_none_or(|(holder, expiry)| holder != &shared.holder || *expiry < now)
    {
        #[cfg(test)]
        tx.execute("INSERT INTO store_counters(name,value) VALUES('test_lease_acquisitions',1) ON CONFLICT(name) DO UPDATE SET value=value+1", []).map_err(persistence)?;
        // An absent lease can still have claims from a cleanly stopped worker.
        tx.execute(&super::row::sql("UPDATE signal_deliveries SET state='{pending}',claimed_by=NULL,claim_expires_at_unix_nano=NULL WHERE state='{claimed}'"), []).map_err(persistence)?;
    }
    tx.execute("INSERT INTO drain_lease VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET acquired_at_unix_nano=CASE WHEN drain_lease.holder!=excluded.holder OR drain_lease.expires_at_unix_nano<excluded.acquired_at_unix_nano THEN excluded.acquired_at_unix_nano ELSE drain_lease.acquired_at_unix_nano END,holder=excluded.holder,expires_at_unix_nano=excluded.expires_at_unix_nano", params![shared.holder,now,expiry]).map_err(persistence)?;
    tx.execute(&super::row::sql("UPDATE signal_deliveries SET claim_expires_at_unix_nano=?2 WHERE state='{claimed}' AND claimed_by=?1"), params![shared.holder,expiry]).map_err(persistence)?;
    if renew {
        store::maintain(&tx, &shared.config, now).map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    Ok(true)
}
struct Claimed {
    id: SubmissionId,
    envelope: SubmissionEnvelope,
    bytes: usize,
    attempts: u32,
}
fn claim(shared: &Shared, signal: Signal) -> Result<Vec<Claimed>, TelemetryClientError> {
    {
        let db = shared.db.lock()?;
        let ready: bool = db.query_row(&super::row::sql("SELECT EXISTS(SELECT 1 FROM signal_deliveries d JOIN submissions s USING(submission_id) WHERE signal=?1 AND state IN ('{pending}','{retry}') AND coalesce(next_attempt_at_unix_nano,0)<=?2 AND envelope_version<=?3)"), params![store::signal_name(signal)?,store::now(),EnvelopeVersion::CURRENT.get()], |r|r.get(0)).map_err(persistence)?;
        if !ready {
            return Ok(vec![]);
        }
    }
    if !lease(shared, false)? {
        return Ok(vec![]);
    }
    let mut db = shared.db.lock()?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(persistence)?;
    let now = store::now();
    let expiry: Option<UnixNanos> = tx.query_row("SELECT expires_at_unix_nano FROM drain_lease WHERE holder=?1 AND expires_at_unix_nano>=?2", params![shared.holder,now], |r| r.get(0)).optional().map_err(persistence)?;
    let Some(expiry) = expiry else {
        return Ok(vec![]);
    };
    let mut batch = vec![];
    let mut loaded_bytes = 0usize;
    let mut failed = vec![];
    {
        // Inspect lengths before fetching BLOBs, bounding each signal batch in memory.
        let mut stmt = tx.prepare(&super::row::sql("SELECT s.submission_id,s.envelope_bytes,d.attempts,length(s.envelope) FROM submissions s JOIN signal_deliveries d USING(submission_id) WHERE d.signal=?1 AND d.state IN ('{pending}','{retry}') AND coalesce(d.next_attempt_at_unix_nano,0)<=?2 AND s.envelope_version<=?3 ORDER BY s.admitted_at_unix_nano,s.submission_id LIMIT ?4")).map_err(persistence)?;
        let rows = stmt
            .query_map(
                params![
                    store::signal_name(signal)?,
                    now,
                    EnvelopeVersion::CURRENT.get(),
                    crate::constants::DRAIN_BATCH_SIZE_SQL
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .map_err(persistence)?;
        for row in rows {
            let (id, bytes, attempts, actual_bytes) = row.map_err(persistence)?;
            let id: SubmissionId = id.parse().map_err(persistence)?;
            if bytes != actual_bytes {
                failed.push((id, crate::error_codes::DURABLE_CORRUPT));
                continue;
            }
            let Ok(bytes) = usize::try_from(bytes) else {
                failed.push((id, crate::error_codes::DURABLE_CORRUPT));
                continue;
            };
            if bytes > shared.credits.byte_capacity() {
                failed.push((id, crate::error_codes::DURABLE_OVERSIZE));
                continue;
            }
            if loaded_bytes.saturating_add(bytes) > shared.credits.byte_capacity() {
                break;
            }
            let encoded: Vec<u8> = tx
                .query_row(
                    "SELECT envelope FROM submissions WHERE submission_id=?1",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .map_err(persistence)?;
            match serde_json::from_slice::<SubmissionEnvelope>(&encoded) {
                Ok(envelope) if envelope.validate().is_ok() => {
                    loaded_bytes += bytes;
                    batch.push(Claimed {
                        id,
                        envelope,
                        bytes,
                        attempts: attempts.saturating_add(1),
                    });
                }
                _ => failed.push((id, crate::error_codes::DURABLE_CORRUPT)),
            }
        }
    }
    for (id, code) in failed {
        tx.execute(&super::row::sql("UPDATE signal_deliveries SET state='{failed}',last_error_code=?3 WHERE submission_id=?1 AND signal=?2"), params![id.to_string(),store::signal_name(signal)?,code.as_str()]).map_err(persistence)?;
    }
    for row in &batch {
        tx.execute(&super::row::sql("UPDATE signal_deliveries SET state='{claimed}',claimed_by=?3,claim_expires_at_unix_nano=?4,attempts=?5 WHERE submission_id=?1 AND signal=?2"), params![row.id.to_string(),store::signal_name(signal)?,shared.holder,expiry,row.attempts]).map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    Ok(batch)
}
enum DrainProgress {
    Idle,
    Exported,
    Interrupted,
}
fn drain(
    shared: &Shared,
    exporter: &dyn SubmissionExporter,
    signal: Signal,
) -> Result<DrainProgress, TelemetryClientError> {
    #[cfg(test)]
    if shared.drain_on_flush_only.load(Ordering::Acquire)
        && shared.active_flushes.load(Ordering::Acquire) == 0
    {
        return Ok(DrainProgress::Idle);
    }
    drain_ready(shared, exporter, signal)
}

#[cfg(test)]
pub(super) fn drain_once_for_test(
    shared: &Shared,
    exporter: &dyn SubmissionExporter,
    signal: Signal,
) -> bool {
    assert!(shared.drain_on_flush_only.load(Ordering::Acquire));
    assert_eq!(shared.active_flushes.load(Ordering::Acquire), 0);
    matches!(
        drain_ready(shared, exporter, signal).unwrap(),
        DrainProgress::Exported
    )
}

fn drain_ready(
    shared: &Shared,
    exporter: &dyn SubmissionExporter,
    signal: Signal,
) -> Result<DrainProgress, TelemetryClientError> {
    let mut batch = claim(shared, signal)?;
    if batch.is_empty() {
        return Ok(DrainProgress::Idle);
    }
    let mut credits = vec![];
    'reserve: for row in &batch {
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(DrainProgress::Idle);
            }
            if let Ok(credit) = shared.credits.reserve_for(signal, row.bytes) {
                credits.push(credit);
                break;
            }
            if !credits.is_empty() {
                break 'reserve;
            }
            #[cfg(test)]
            {
                shared.waiting_for_credits.lock().unwrap().insert(signal);
                shared.notify();
            }
            shared.credits.wait_for_release(Shared::poll_interval());
        }
    }
    // Never wait for credits held by this same batch. Return its unreserved tail
    // to the store and export the prefix that fits the shared budget.
    let deferred = batch.split_off(credits.len());
    {
        let db = shared.db.lock()?;
        for row in deferred {
            db.execute(&super::row::sql("UPDATE signal_deliveries SET state='{pending}',claimed_by=NULL,claim_expires_at_unix_nano=NULL,attempts=attempts-1 WHERE submission_id=?1 AND signal=?2 AND state='{claimed}' AND claimed_by=?3 AND attempts=?4"), params![row.id.to_string(),store::signal_name(signal)?,shared.holder,row.attempts]).map_err(persistence)?;
        }
        let owns: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM drain_lease WHERE holder=?1 AND expires_at_unix_nano>=?2)", params![shared.holder,store::now()], |r|r.get(0)).map_err(persistence)?;
        if !owns || shared.stop.load(Ordering::Acquire) {
            return Ok(DrainProgress::Idle);
        }
    }
    let envelopes: Vec<_> = batch.iter().map(|row| row.envelope.clone()).collect();
    let result = exporter.export(signal, &envelopes);
    let mut db = shared.db.lock()?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(persistence)?;
    let now = store::now();
    let owns: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM drain_lease WHERE holder=?1 AND expires_at_unix_nano>=?2)",
            params![shared.holder, now],
            |r| r.get(0),
        )
        .map_err(persistence)?;
    if !owns {
        return Ok(DrainProgress::Idle);
    }
    // Explicit transport shutdown interrupts delivery; it does not exhaust a
    // network retry budget. Release these claims without charging an attempt
    // and stop this signal worker rather than repeatedly calling a stopped
    // exporter. A replacement client can reclaim the pending rows.
    if matches!(
        &result,
        Err(SubmissionExportFailure::Retryable(
            sc_observability_types::v2::ExportError::ShutdownCancelledRetry { .. }
        ))
    ) {
        release_interrupted(tx, shared, signal, &batch)?;
        return Ok(DrainProgress::Interrupted);
    }
    for row in &batch {
        let outcome = match &result {
            Ok(()) => Outcome::Delivered,
            Err(SubmissionExportFailure::Terminal(error)) => {
                Outcome::Failed(error.diagnostic().code.as_str())
            }
            Err(SubmissionExportFailure::Retryable(error)) => {
                retry_outcome(shared, row.attempts, error.diagnostic().code.as_str(), now)
            }
        };
        tx.execute(&super::row::sql("UPDATE signal_deliveries SET state=?3,last_error_code=?4,next_attempt_at_unix_nano=?5,delivered_at_unix_nano=?6,claimed_by=NULL,claim_expires_at_unix_nano=NULL WHERE submission_id=?1 AND signal=?2 AND state='{claimed}' AND claimed_by=?7 AND attempts=?8"), params![row.id.to_string(),store::signal_name(signal)?,outcome.state().as_str(),outcome.code(),outcome.next(),if matches!(outcome, Outcome::Delivered) { Some(now) } else { None },shared.holder,row.attempts]).map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    drop(credits);
    Ok(DrainProgress::Exported)
}

fn retry_outcome<'a>(shared: &Shared, attempts: u32, code: &'a str, now: UnixNanos) -> Outcome<'a> {
    let retry = shared.config.sync_http_retry.as_ref();
    let max = retry
        .and_then(|r| r.max_retries)
        .unwrap_or(crate::constants::DEFAULT_OTLP_MAX_RETRIES);
    if attempts > max {
        Outcome::Failed(code)
    } else {
        let delay = retry_backoff_ms(attempts, retry);
        Outcome::Retry {
            code,
            next: now.saturating_add(store::nanos(Duration::from_millis(delay))),
        }
    }
}

fn release_interrupted(
    tx: rusqlite::Transaction<'_>,
    shared: &Shared,
    signal: Signal,
    batch: &[Claimed],
) -> Result<(), TelemetryClientError> {
    for row in batch {
        tx.execute(&super::row::sql("UPDATE signal_deliveries SET state='{pending}',attempts=attempts-1,next_attempt_at_unix_nano=NULL,claimed_by=NULL,claim_expires_at_unix_nano=NULL WHERE submission_id=?1 AND signal=?2 AND state='{claimed}' AND claimed_by=?3 AND attempts=?4"), params![row.id.to_string(),store::signal_name(signal)?,shared.holder,row.attempts]).map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    Ok(())
}

fn retry_backoff_ms(attempts: u32, retry: Option<&SyncHttpRetryPolicyDto>) -> u64 {
    let initial = retry
        .and_then(|r| r.initial_backoff_ms)
        .unwrap_or(crate::constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS);
    let cap = retry
        .and_then(|r| r.max_backoff_ms)
        .unwrap_or(crate::constants::DEFAULT_OTLP_MAX_BACKOFF_MS);
    initial
        .saturating_mul(2u64.saturating_pow(attempts.saturating_sub(1)))
        .min(cap)
}
enum Outcome<'a> {
    Delivered,
    Failed(&'a str),
    Retry { code: &'a str, next: UnixNanos },
}
impl Outcome<'_> {
    fn state(&self) -> RowState {
        match self {
            Self::Delivered => RowState::Delivered,
            Self::Failed(_) => RowState::Failed,
            Self::Retry { .. } => RowState::Retry,
        }
    }
    fn code(&self) -> Option<&str> {
        match self {
            Self::Delivered => None,
            Self::Failed(code) | Self::Retry { code, .. } => Some(code),
        }
    }
    fn next(&self) -> Option<UnixNanos> {
        match self {
            Self::Retry { next, .. } => Some(*next),
            _ => None,
        }
    }
}

fn error_backoff(failures: u32) -> Duration {
    Duration::from_millis(
        crate::constants::DRAIN_POLL_INTERVAL_MS
            .saturating_mul(2u64.saturating_pow(failures.saturating_sub(1)))
            .min(crate::constants::DRAIN_ERROR_BACKOFF_MAX_MS),
    )
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    use crate::durable::DurableTelemetryClient;
    use sc_observability_types::otlp::submission::{
        ConfigOverrides, ConfigSources, LogInput, SubmissionInput, SystemIds, TelemetryClient,
        resolve_config,
    };

    #[test]
    fn all_four_signals_map_to_their_lifecycle_accounting_slots() {
        let mut accounted = crate::lifecycle::SignalArray::new([0; 4]);
        for signal in [
            Signal::Logs,
            Signal::Traces,
            Signal::Metrics,
            Signal::Profiles,
        ] {
            *accounted
                .get_mut(signal)
                .expect("each supported signal has an accounting slot") += 1;
        }

        assert_eq!(accounted, [1; 4], "each signal accounts in its own slot");
    }

    #[test]
    fn idle_claim_does_not_write_and_batches_obey_byte_budget() {
        let dir = tempfile::tempdir().unwrap();
        let mut overrides = ConfigOverrides::default();
        overrides.store_path = Some(dir.path().join("store.db"));
        let config = resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap();
        let mut input = SubmissionInput::new();
        input.logs.push(LogInput::new());
        let envelope = SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap();
        let size = envelope.to_canonical_json().len();
        let bounds = crate::config::validated_transport_bounds(&crate::config::OtelConfig {
            queue_byte_capacity: Some(size * 2),
            ..crate::config::OtelConfig::default()
        })
        .unwrap();
        let client = DurableTelemetryClient::prepare_validated(config, &bounds).unwrap();
        let shared = &client.owner.shared;
        let before = shared.db.lock().unwrap().total_changes();
        for signal in [
            Signal::Logs,
            Signal::Traces,
            Signal::Metrics,
            Signal::Profiles,
        ] {
            assert!(claim(shared, signal).unwrap().is_empty());
        }
        assert_eq!(
            shared.db.lock().unwrap().total_changes(),
            before,
            "idle signal polls must be read-only"
        );
        for _ in 0..4 {
            let mut input = SubmissionInput::new();
            input.logs.push(LogInput::new());
            client
                .emit(SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap())
                .unwrap();
        }
        let batch = claim(shared, Signal::Logs).unwrap();
        assert_eq!(batch.len(), 2);
        assert!(batch.iter().map(|row| row.bytes).sum::<usize>() <= size * 2);
    }
    #[test]
    fn repeated_worker_failures_have_capped_backoff() {
        let delays: Vec<_> = [1, 2, 3, 32, u32::MAX]
            .into_iter()
            .map(error_backoff)
            .collect();
        assert!(delays.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(
            *delays.last().unwrap(),
            Duration::from_millis(crate::constants::DRAIN_ERROR_BACKOFF_MAX_MS)
        );
    }
    #[test]
    fn persisted_retry_uses_named_initial_and_max_backoff_bounds() {
        assert_eq!(
            retry_backoff_ms(1, None),
            crate::constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS
        );
        assert_eq!(
            retry_backoff_ms(u32::MAX, None),
            crate::constants::DEFAULT_OTLP_MAX_BACKOFF_MS
        );
    }

    #[test]
    fn persisted_retry_uses_configured_initial_and_max_backoff() {
        let mut retry = SyncHttpRetryPolicyDto::default();
        retry.initial_backoff_ms = Some(10);
        retry.max_backoff_ms = Some(35);

        assert_eq!(retry_backoff_ms(1, Some(&retry)), 10);
        assert_eq!(retry_backoff_ms(2, Some(&retry)), 20);
        assert_eq!(retry_backoff_ms(3, Some(&retry)), 35);
        assert_eq!(retry_backoff_ms(u32::MAX, Some(&retry)), 35);
    }
}
