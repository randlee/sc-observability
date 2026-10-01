//! Scoped lifecycle snapshots and delivery status decoding.
use super::{persistence, row::RowState, store};
use rusqlite::{Connection, OptionalExtension, params};
use sc_observability_types::{
    ErrorCode,
    otlp::submission::{
        DeliveryState, DeliveryStatus, EnvelopeVersion, FlushReport, LeaseInfo, Signal,
        StatusQuery, StoreStatus, SubmissionId, TelemetryClientConfig, TelemetryClientError,
    },
};

pub(super) type Scope = Vec<(SubmissionId, Signal)>;
pub(super) fn snapshot(
    db: &Connection,
    id: Option<&SubmissionId>,
) -> Result<Scope, TelemetryClientError> {
    let mut stmt = db.prepare(&super::row::sql("SELECT submission_id,signal FROM signal_deliveries WHERE (?1 IS NOT NULL AND submission_id=?1) OR (?1 IS NULL AND state IN ('{pending}','{claimed}','{retry}'))")).map_err(persistence)?;
    let rows = stmt
        .query_map([id.map(ToString::to_string)], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(persistence)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(persistence)?;
    rows.into_iter()
        .map(|(id, signal)| {
            Ok((
                id.parse().map_err(persistence)?,
                store::parse_signal(&signal).map_err(persistence)?,
            ))
        })
        .collect()
}
pub(super) fn report(db: &Connection, scope: &Scope) -> Result<FlushReport, TelemetryClientError> {
    let mut report = FlushReport::default();
    for (id, signal) in scope {
        let state: Option<String> = db
            .query_row(
                "SELECT state FROM signal_deliveries WHERE submission_id=?1 AND signal=?2",
                params![id.to_string(), store::signal_name(*signal)?],
                |r| r.get(0),
            )
            .optional()
            .map_err(persistence)?;
        // Only delivered retention removes rows; purged successful rows remain success for this snapshot.
        let counts = match state
            .as_deref()
            .map(RowState::parse)
            .transpose()
            .map_err(persistence)?
        {
            Some(RowState::Failed) => &mut report.failed,
            Some(RowState::Evicted) => &mut report.evicted,
            Some(RowState::Pending | RowState::Claimed | RowState::Retry) => {
                &mut report.still_pending
            }
            None | Some(RowState::Delivered) => &mut report.delivered,
        };
        counts.increment(*signal);
    }
    Ok(report)
}
fn delivery(row: &rusqlite::Row<'_>) -> rusqlite::Result<(Signal, DeliveryState)> {
    let signal = store::parse_signal(&row.get::<_, String>(0)?)?;
    let state: String = row.get(1)?;
    let attempts = row.get(2)?;
    let value = match RowState::parse(&state)? {
        RowState::Pending => DeliveryState::Pending,
        RowState::Claimed => DeliveryState::Claimed {
            holder: row.get(3)?,
            attempts,
        },
        RowState::Retry => DeliveryState::RetryScheduled {
            attempts,
            next_attempt_at: store::timestamp(row.get(4)?),
            last_error: ErrorCode::new_owned(row.get::<_, String>(5)?),
        },
        RowState::Delivered => DeliveryState::Delivered {
            at: store::timestamp(row.get(6)?),
            attempts,
        },
        RowState::Failed => DeliveryState::Failed {
            attempts,
            error: ErrorCode::new_owned(row.get::<_, String>(5)?),
        },
        RowState::Evicted => DeliveryState::EvictedByDiskBound {
            at: store::timestamp(row.get(6)?),
        },
    };
    Ok((signal, value))
}
pub(super) fn status(
    db: &Connection,
    config: &TelemetryClientConfig,
    query: StatusQuery,
) -> Result<StoreStatus, TelemetryClientError> {
    let mut status = StoreStatus::default();
    status.schema_version = crate::constants::STORE_SCHEMA_VERSION;
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
            |r| store::unsigned_column(r, 0),
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
        match RowState::parse(&row.get::<_, String>(1).map_err(persistence)?)
            .map_err(persistence)?
        {
            RowState::Pending | RowState::Claimed => status.pending.increment(signal),
            RowState::Retry => status.retry_scheduled.increment(signal),
            RowState::Delivered => status.delivered_retained.increment(signal),
            RowState::Failed => status.failed.increment(signal),
            RowState::Evicted => {}
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
        _ => {
            return Err(super::AdmissionError::StoreUnavailable {
                context: super::context(
                    crate::error_codes::DURABLE_QUERY,
                    "unsupported status query; use Summary, Submissions or RecordKeys",
                ),
            }
            .into());
        }
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
