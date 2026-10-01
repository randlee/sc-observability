//! Transactional admission, retention and the frozen `SQLite` schema.
use super::{context, persistence};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sc_observability_types::{
    Timestamp,
    otlp::submission::{
        AdmissionError, AdmissionReceipt, DiskBoundPolicy, Signal, SubmissionEnvelope,
        SubmissionId, TelemetryClientConfig, TelemetryClientError, error_codes,
    },
};
use std::{path::Path, time::Duration};

pub(super) fn now() -> i64 {
    i64::try_from(Timestamp::now_utc().into_inner().unix_timestamp_nanos())
        .expect("current time fits SQLite nanoseconds")
}
pub(super) fn nanos(duration: Duration) -> i64 {
    i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX)
}
pub(super) fn timestamp(value: i64) -> Timestamp {
    let epoch = Timestamp::UNIX_EPOCH.into_inner();
    let duration = Duration::from_nanos(value.unsigned_abs());
    Timestamp::from(if value >= 0 {
        epoch + duration
    } else {
        epoch - duration
    })
}
pub(super) fn signal_name(signal: Signal) -> &'static str {
    match signal {
        Signal::Logs => "logs",
        Signal::Traces => "traces",
        Signal::Metrics => "metrics",
        Signal::Profiles => "profiles",
        _ => unreachable!("validated current signal"),
    }
}
pub(super) fn parse_signal(value: &str) -> rusqlite::Result<Signal> {
    match value {
        "logs" => Ok(Signal::Logs),
        "traces" => Ok(Signal::Traces),
        "metrics" => Ok(Signal::Metrics),
        "profiles" => Ok(Signal::Profiles),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn check_version(db: &Connection) -> Result<u32, TelemetryClientError> {
    let found: u32 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(persistence)?;
    if found > 1 {
        return Err(AdmissionError::SchemaTooNew { found, supported: 1, context: context(error_codes::SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW, "store schema is newer than this client; use a compatible client without modifying the store") }.into());
    }
    Ok(found)
}
pub(super) fn open(path: &Path) -> Result<Connection, TelemetryClientError> {
    // Inspect existing files read-only before any journal/schema changes.
    if path.exists() {
        let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(persistence)?;
        check_version(&db)?;
    } else if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(persistence)?;
    }
    let mut db = Connection::open(path).map_err(persistence)?;
    db.busy_timeout(Duration::from_millis(
        crate::constants::STORE_BUSY_TIMEOUT_MS,
    ))
    .map_err(persistence)?;
    check_version(&db)?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .map_err(persistence)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(persistence)?;
    if check_version(&tx)? == 0 {
        tx.execute_batch(include_str!("schema.sql"))
            .map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    Ok(db)
}
pub(super) fn bytes(db: &Connection) -> rusqlite::Result<u64> {
    db.query_row(
        "SELECT coalesce(sum(envelope_bytes),0) FROM submissions",
        [],
        |r| {
            r.get::<_, i64>(0).and_then(|v| {
                u64::try_from(v).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, v))
            })
        },
    )
}
pub(super) fn count(db: &Connection, name: &str) -> rusqlite::Result<u64> {
    Ok(db
        .query_row(
            "SELECT value FROM store_counters WHERE name=?1",
            [name],
            |r| {
                r.get::<_, i64>(0).and_then(|v| {
                    u64::try_from(v).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, v))
                })
            },
        )
        .optional()?
        .unwrap_or(0))
}
fn increment(db: &Connection, name: &str) -> rusqlite::Result<()> {
    db.execute("INSERT INTO store_counters(name,value) VALUES(?1,1) ON CONFLICT(name) DO UPDATE SET value=value+1", [name])?;
    Ok(())
}
pub(super) fn maintain(
    tx: &Transaction<'_>,
    config: &TelemetryClientConfig,
    now: i64,
) -> rusqlite::Result<()> {
    // Keys and receipts outlive payloads. Never purge a submission with unfinished rows.
    tx.execute("DELETE FROM store_meta WHERE key IN (SELECT 'receipt:' || submission_id FROM record_keys WHERE admitted_at_unix_nano < ?1)", [now.saturating_sub(nanos(config.record_key_retention))])?;
    tx.execute(
        "DELETE FROM record_keys WHERE admitted_at_unix_nano < ?1",
        [now.saturating_sub(nanos(config.record_key_retention))],
    )?;
    tx.execute("UPDATE submissions SET record_key=NULL WHERE record_key IS NOT NULL AND record_key NOT IN (SELECT record_key FROM record_keys)", [])?;
    tx.execute("DELETE FROM submissions WHERE NOT EXISTS (SELECT 1 FROM signal_deliveries d WHERE d.submission_id=submissions.submission_id AND (d.state != 'delivered' OR d.delivered_at_unix_nano >= ?1))", [now.saturating_sub(nanos(config.delivered_retention))])?;
    Ok(())
}
pub(super) fn admit(
    db: &mut Connection,
    config: &TelemetryClientConfig,
    envelope: &SubmissionEnvelope,
) -> Result<AdmissionReceipt, TelemetryClientError> {
    envelope.validate()?;
    let time = now();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(persistence)?;
    maintain(&tx, config, time).map_err(persistence)?;
    if let Some(key) = &envelope.record_key {
        let receipt: Option<String> = tx.query_row("SELECT m.value FROM record_keys r JOIN store_meta m ON m.key='receipt:' || r.submission_id WHERE r.record_key=?1", [key.to_string()], |r| r.get(0)).optional().map_err(persistence)?;
        if let Some(receipt) = receipt {
            let mut receipt: AdmissionReceipt =
                serde_json::from_str(&receipt).map_err(persistence)?;
            receipt.duplicate = true;
            tx.commit().map_err(persistence)?;
            return Ok(receipt);
        }
    }
    let encoded = envelope.to_canonical_json();
    let size = u64::try_from(encoded.len()).map_err(persistence)?;
    let used = bytes(&tx).map_err(persistence)?;
    if size > config.max_store_bytes
        || used.saturating_add(size) > config.max_store_bytes
            && config.disk_bound_policy != DiskBoundPolicy::EvictOldest
    {
        increment(&tx, "rejected_by_disk_bound").map_err(persistence)?;
        tx.commit().map_err(persistence)?;
        return Err(AdmissionError::DiskBoundExceeded {
            context: context(
                error_codes::SC_OBSERVABILITY_ADMIT_DISK_BOUND,
                "store byte bound exceeded; drain the store or increase max_store_bytes",
            ),
        }
        .into());
    }
    while bytes(&tx).map_err(persistence)?.saturating_add(size) > config.max_store_bytes {
        let candidate: Option<String> = tx.query_row("SELECT s.submission_id FROM submissions s WHERE s.envelope_bytes>0 AND EXISTS(SELECT 1 FROM signal_deliveries d WHERE d.submission_id=s.submission_id AND d.state IN ('pending','claimed','retry')) ORDER BY s.admitted_at_unix_nano,s.submission_id LIMIT 1", [], |r| r.get(0)).optional().map_err(persistence)?;
        let Some(id) = candidate else {
            increment(&tx, "rejected_by_disk_bound").map_err(persistence)?;
            tx.commit().map_err(persistence)?;
            return Err(AdmissionError::DiskBoundExceeded { context: context(error_codes::SC_OBSERVABILITY_ADMIT_DISK_BOUND, "retained terminal data occupies the store; adjust retention or max_store_bytes") }.into());
        };
        tx.execute("UPDATE signal_deliveries SET state='evicted',claimed_by=NULL,claim_expires_at_unix_nano=NULL,delivered_at_unix_nano=?2 WHERE submission_id=?1 AND state IN ('pending','claimed','retry')", params![id,time]).map_err(persistence)?;
        // Preserve queryable eviction rows despite the schema's cascading foreign key.
        tx.execute(
            "UPDATE submissions SET envelope=x'',envelope_bytes=0 WHERE submission_id=?1",
            [&id],
        )
        .map_err(persistence)?;
        increment(&tx, "evicted_by_disk_bound").map_err(persistence)?;
    }
    let id: SubmissionId = uuid::Uuid::now_v7()
        .to_string()
        .parse()
        .map_err(persistence)?;
    let receipt = AdmissionReceipt::new(
        id.clone(),
        envelope.record_key.clone(),
        timestamp(time),
        envelope.signals().iter().collect(),
        false,
    );
    tx.execute(
        "INSERT INTO submissions VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            id.to_string(),
            envelope.record_key.as_ref().map(ToString::to_string),
            envelope.version.get(),
            encoded.as_bytes(),
            i64::try_from(size).map_err(persistence)?,
            time
        ],
    )
    .map_err(persistence)?;
    for signal in envelope.signals().iter() {
        tx.execute(
            "INSERT INTO signal_deliveries(submission_id,signal,state) VALUES(?1,?2,'pending')",
            params![id.to_string(), signal_name(signal)],
        )
        .map_err(persistence)?;
    }
    if let Some(key) = &envelope.record_key {
        tx.execute(
            "INSERT INTO record_keys VALUES(?1,?2,?3)",
            params![key.to_string(), id.to_string(), time],
        )
        .map_err(persistence)?;
        tx.execute(
            "INSERT INTO store_meta VALUES(?1,?2)",
            params![
                format!("receipt:{id}"),
                serde_json::to_string(&receipt).map_err(persistence)?
            ],
        )
        .map_err(persistence)?;
    }
    tx.commit().map_err(persistence)?;
    Ok(receipt)
}
