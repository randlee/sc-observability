//! Lease-fenced per-signal workers; transport calls never hold a database lock.
use super::{Shared, persistence, store};
use crate::contracts::submission::{SubmissionExportFailure, SubmissionExporter};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use sc_observability_types::otlp::submission::{
    EnvelopeVersion, Signal, SubmissionEnvelope, TelemetryClientError,
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
    std::thread::Builder::new()
        .name("telemetry-lease".into())
        .spawn(move || {
            while !heartbeat.stop.load(Ordering::Acquire) {
                let _ = lease(&heartbeat);
                heartbeat.wait(
                    heartbeat.config.lease_duration / crate::constants::LEASE_RENEWAL_DIVISOR,
                );
            }
            release(&heartbeat);
        })
        .map_err(persistence)?;
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
        let shared = Arc::clone(shared);
        std::thread::Builder::new()
            .name(format!("telemetry-{}", store::signal_name(signal)))
            .spawn(move || {
                while !shared.stop.load(Ordering::Acquire) {
                    let generation = shared.generation();
                    match drain(&shared, exporter.as_ref(), signal) {
                        Ok(true) => shared.notify(),
                        Ok(false) | Err(_) => shared.wait_since(generation, shared.poll_interval()),
                    }
                }
            })
            .map_err(persistence)?;
    }
    Ok(())
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
        let _ = db.busy_timeout(Duration::from_millis(
            crate::constants::STORE_BUSY_TIMEOUT_MS,
        ));
    }
}
fn lease(shared: &Shared) -> Result<bool, TelemetryClientError> {
    let mut db = shared.db.lock().map_err(persistence)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(persistence)?;
    let now = store::now();
    let old: Option<(String, i64)> = tx
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
        // An absent lease can still have claims from a cleanly stopped worker.
        tx.execute("UPDATE signal_deliveries SET state='pending',claimed_by=NULL,claim_expires_at_unix_nano=NULL WHERE state='claimed'", []).map_err(persistence)?;
    }
    tx.execute("INSERT INTO drain_lease VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET holder=excluded.holder,expires_at_unix_nano=excluded.expires_at_unix_nano", params![shared.holder,now,expiry]).map_err(persistence)?;
    tx.execute("UPDATE signal_deliveries SET claim_expires_at_unix_nano=?2 WHERE state='claimed' AND claimed_by=?1", params![shared.holder,expiry]).map_err(persistence)?;
    store::maintain(&tx, &shared.config, now).map_err(persistence)?;
    tx.commit().map_err(persistence)?;
    Ok(true)
}
struct Claimed {
    id: String,
    envelope: SubmissionEnvelope,
    bytes: usize,
    attempts: u32,
}
fn claim(shared: &Shared, signal: Signal) -> Result<Vec<Claimed>, TelemetryClientError> {
    if !lease(shared)? {
        return Ok(vec![]);
    }
    let mut db = shared.db.lock().map_err(persistence)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(persistence)?;
    let now = store::now();
    let expiry: Option<i64> = tx.query_row("SELECT expires_at_unix_nano FROM drain_lease WHERE holder=?1 AND expires_at_unix_nano>=?2", params![shared.holder,now], |r| r.get(0)).optional().map_err(persistence)?;
    let Some(expiry) = expiry else {
        return Ok(vec![]);
    };
    let mut batch = vec![];
    {
        let mut stmt = tx.prepare("SELECT s.submission_id,s.envelope,s.envelope_bytes,d.attempts FROM submissions s JOIN signal_deliveries d USING(submission_id) WHERE d.signal=?1 AND d.state IN ('pending','retry') AND coalesce(d.next_attempt_at_unix_nano,0)<=?2 AND s.envelope_version<=?3 ORDER BY s.admitted_at_unix_nano,s.submission_id LIMIT ?4").map_err(persistence)?;
        let mut rows = stmt
            .query(params![
                store::signal_name(signal),
                now,
                EnvelopeVersion::CURRENT.get(),
                i64::try_from(crate::constants::DRAIN_BATCH_SIZE).expect("batch size fits SQLite")
            ])
            .map_err(persistence)?;
        while let Some(row) = rows.next().map_err(persistence)? {
            let bytes: Vec<u8> = row.get(1).map_err(persistence)?;
            batch.push(Claimed {
                id: row.get(0).map_err(persistence)?,
                envelope: serde_json::from_slice(&bytes).map_err(persistence)?,
                bytes: usize::try_from(row.get::<_, i64>(2).map_err(persistence)?)
                    .map_err(persistence)?,
                attempts: row.get::<_, u32>(3).map_err(persistence)?.saturating_add(1),
            });
        }
    }
    for row in &batch {
        tx.execute("UPDATE signal_deliveries SET state='claimed',claimed_by=?3,claim_expires_at_unix_nano=?4,attempts=?5 WHERE submission_id=?1 AND signal=?2", params![row.id,store::signal_name(signal),shared.holder,expiry,row.attempts]).map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    Ok(batch)
}
fn drain(
    shared: &Shared,
    exporter: &dyn SubmissionExporter,
    signal: Signal,
) -> Result<bool, TelemetryClientError> {
    #[cfg(test)]
    if shared.drain_on_flush_only.load(Ordering::Acquire)
        && shared.active_flushes.load(Ordering::Acquire) == 0
    {
        return Ok(false);
    }
    let mut batch = claim(shared, signal)?;
    if batch.is_empty() {
        return Ok(false);
    }
    let mut credits = vec![];
    'reserve: for row in &batch {
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(false);
            }
            if let Ok(credit) = shared.credits.reserve(row.bytes) {
                credits.push(credit);
                break;
            }
            if !credits.is_empty() {
                break 'reserve;
            }
            shared.credits.wait_for_release(shared.poll_interval());
        }
    }
    // Never wait for credits held by this same batch. Return its unreserved tail
    // to the store and export the prefix that fits the shared budget.
    let deferred = batch.split_off(credits.len());
    {
        let db = shared.db.lock().map_err(persistence)?;
        for row in deferred {
            db.execute("UPDATE signal_deliveries SET state='pending',claimed_by=NULL,claim_expires_at_unix_nano=NULL,attempts=attempts-1 WHERE submission_id=?1 AND signal=?2 AND state='claimed' AND claimed_by=?3 AND attempts=?4", params![row.id,store::signal_name(signal),shared.holder,row.attempts]).map_err(persistence)?;
        }
        let owns: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM drain_lease WHERE holder=?1 AND expires_at_unix_nano>=?2)", params![shared.holder,store::now()], |r|r.get(0)).map_err(persistence)?;
        if !owns || shared.stop.load(Ordering::Acquire) {
            return Ok(false);
        }
    }
    let envelopes: Vec<_> = batch.iter().map(|row| row.envelope.clone()).collect();
    let result = exporter.export(signal, &envelopes);
    let mut db = shared.db.lock().map_err(persistence)?;
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
    if !owns || shared.stop.load(Ordering::Acquire) {
        return Ok(false);
    }
    for row in &batch {
        let (state, code, next) = match &result {
            Ok(()) => ("delivered", None, None),
            Err(SubmissionExportFailure::Terminal(error)) => {
                ("failed", Some(error.diagnostic().code.as_str()), None)
            }
            Err(SubmissionExportFailure::Retryable(error)) => {
                let retry = shared.config.sync_http_retry.as_ref();
                let max = retry
                    .and_then(|r| r.max_retries)
                    .unwrap_or(crate::constants::DEFAULT_OTLP_MAX_RETRIES);
                if row.attempts > max {
                    ("failed", Some(error.diagnostic().code.as_str()), None)
                } else {
                    let initial = retry
                        .and_then(|r| r.initial_backoff_ms)
                        .unwrap_or(crate::constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS);
                    let cap = retry
                        .and_then(|r| r.max_backoff_ms)
                        .unwrap_or(crate::constants::DEFAULT_OTLP_MAX_BACKOFF_MS);
                    let delay = initial
                        .saturating_mul(2u64.saturating_pow(row.attempts.saturating_sub(1)))
                        .min(cap);
                    (
                        "retry",
                        Some(error.diagnostic().code.as_str()),
                        Some(now.saturating_add(store::nanos(Duration::from_millis(delay)))),
                    )
                }
            }
        };
        tx.execute("UPDATE signal_deliveries SET state=?3,last_error_code=?4,next_attempt_at_unix_nano=?5,delivered_at_unix_nano=?6,claimed_by=NULL,claim_expires_at_unix_nano=NULL WHERE submission_id=?1 AND signal=?2 AND state='claimed' AND claimed_by=?7 AND attempts=?8", params![row.id,store::signal_name(signal),state,code,next,if state=="delivered" { Some(now) } else { None },shared.holder,row.attempts]).map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    drop(credits);
    Ok(true)
}
