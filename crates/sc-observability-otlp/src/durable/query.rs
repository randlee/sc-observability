//! Scoped lifecycle snapshots and delivery status decoding.
use super::{persistence, store};
use rusqlite::{Connection, OptionalExtension, params};
use sc_observability_types::{
    ErrorCode,
    otlp::submission::{
        DeliveryState, DeliveryStatus, EnvelopeVersion, FlushReport, LeaseInfo, Signal,
        StatusQuery, StoreStatus, SubmissionId, TelemetryClientConfig, TelemetryClientError,
    },
};

pub(super) type Scope = Vec<(String, String)>;
pub(super) fn snapshot(
    db: &Connection,
    id: Option<&SubmissionId>,
) -> Result<Scope, TelemetryClientError> {
    let mut stmt = db.prepare("SELECT submission_id,signal FROM signal_deliveries WHERE (?1 IS NOT NULL AND submission_id=?1) OR (?1 IS NULL AND state IN ('pending','claimed','retry'))").map_err(persistence)?;
    stmt.query_map([id.map(ToString::to_string)], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })
    .map_err(persistence)?
    .collect::<rusqlite::Result<_>>()
    .map_err(persistence)
}
pub(super) fn report(db: &Connection, scope: &Scope) -> Result<FlushReport, TelemetryClientError> {
    let mut report = FlushReport::default();
    for (id, signal) in scope {
        let state: Option<String> = db
            .query_row(
                "SELECT state FROM signal_deliveries WHERE submission_id=?1 AND signal=?2",
                params![id, signal],
                |r| r.get(0),
            )
            .optional()
            .map_err(persistence)?;
        // Only delivered retention removes rows; purged successful rows remain success for this snapshot.
        let counts = match state.as_deref() {
            Some("failed") => &mut report.failed,
            Some("evicted") => &mut report.evicted,
            Some("pending" | "claimed" | "retry") => &mut report.still_pending,
            _ => &mut report.delivered,
        };
        counts.increment(store::parse_signal(signal).map_err(persistence)?);
    }
    Ok(report)
}
fn delivery(row: &rusqlite::Row<'_>) -> rusqlite::Result<(Signal, DeliveryState)> {
    let signal = store::parse_signal(&row.get::<_, String>(0)?)?;
    let state: String = row.get(1)?;
    let attempts = row.get(2)?;
    let value = match state.as_str() {
        "pending" => DeliveryState::Pending,
        "claimed" => DeliveryState::Claimed {
            holder: row.get(3)?,
            attempts,
        },
        "retry" => DeliveryState::RetryScheduled {
            attempts,
            next_attempt_at: store::timestamp(row.get(4)?),
            last_error: ErrorCode::new_owned(row.get::<_, String>(5)?),
        },
        "delivered" => DeliveryState::Delivered {
            at: store::timestamp(row.get(6)?),
            attempts,
        },
        "failed" => DeliveryState::Failed {
            attempts,
            error: ErrorCode::new_owned(row.get::<_, String>(5)?),
        },
        "evicted" => DeliveryState::EvictedByDiskBound {
            at: store::timestamp(row.get(6)?),
        },
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok((signal, value))
}
pub(super) fn status(
    db: &Connection,
    config: &TelemetryClientConfig,
    query: StatusQuery,
) -> Result<StoreStatus, TelemetryClientError> {
    let mut status = StoreStatus::default();
    status.schema_version = 1;
    status.store_bytes = store::bytes(db).map_err(persistence)?;
    status.max_store_bytes = config.max_store_bytes;
    status.evicted_by_disk_bound =
        store::count(db, "evicted_by_disk_bound").map_err(persistence)?;
    status.rejected_by_disk_bound =
        store::count(db, "rejected_by_disk_bound").map_err(persistence)?;
    status.unreadable_newer_envelopes = db
        .query_row(
            "SELECT count(*) FROM submissions WHERE envelope_version>?1",
            [EnvelopeVersion::CURRENT.get()],
            |r| {
                r.get::<_, i64>(0).and_then(|v| {
                    u64::try_from(v).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, v))
                })
            },
        )
        .map_err(persistence)?;
    status.lease = db
        .query_row(
            "SELECT holder,expires_at_unix_nano FROM drain_lease WHERE id=1",
            [],
            |r| Ok(LeaseInfo::new(r.get(0)?, store::timestamp(r.get(1)?))),
        )
        .optional()
        .map_err(persistence)?;
    let mut stmt = db
        .prepare("SELECT signal,state FROM signal_deliveries")
        .map_err(persistence)?;
    let mut rows = stmt.query([]).map_err(persistence)?;
    while let Some(row) = rows.next().map_err(persistence)? {
        let signal = store::parse_signal(&row.get::<_, String>(0).map_err(persistence)?)
            .map_err(persistence)?;
        match row.get::<_, String>(1).map_err(persistence)?.as_str() {
            "pending" | "claimed" => status.pending.increment(signal),
            "retry" => status.retry_scheduled.increment(signal),
            "delivered" => status.delivered_retained.increment(signal),
            "failed" => status.failed.increment(signal),
            _ => {}
        }
    }
    let ids = match query {
        StatusQuery::Summary => vec![],
        StatusQuery::Submissions(ids) => ids,
        StatusQuery::RecordKeys(keys) => {
            let mut ids = vec![];
            for key in keys {
                let id: Option<String> = db
                    .query_row(
                        "SELECT submission_id FROM record_keys WHERE record_key=?1",
                        [key.to_string()],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(persistence)?;
                if let Some(id) = id {
                    ids.push(id.parse().map_err(persistence)?);
                }
            }
            ids
        }
        _ => return Err(persistence("unsupported status query")),
    };
    let mut stmt = db.prepare("SELECT signal,state,attempts,claimed_by,next_attempt_at_unix_nano,last_error_code,delivered_at_unix_nano FROM signal_deliveries WHERE submission_id=?1 ORDER BY signal").map_err(persistence)?;
    for id in ids {
        let deliveries = stmt
            .query_map([id.to_string()], delivery)
            .map_err(persistence)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(persistence)?;
        if !deliveries.is_empty() {
            status.submissions.push(DeliveryStatus::new(id, deliveries));
        }
    }
    Ok(status)
}
