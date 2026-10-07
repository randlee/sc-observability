//! Regression coverage for the carried D33 QA findings.
use super::*;
fn rows(db: &rusqlite::Connection, table: &str, id: &SubmissionId) -> i64 {
    db.query_row(
        &format!("SELECT count(*) FROM {table} WHERE submission_id=?1"),
        [id.to_string()],
        |r| r.get(0),
    )
    .unwrap()
}
#[test]
fn retention_removes_only_delivered_payloads_and_expires_keys_and_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let delivered = store::admit(&mut db, &config, &log("delivered")).unwrap();
    let pending = store::admit(&mut db, &config, &log("pending")).unwrap();
    let failed = store::admit(&mut db, &config, &log("failed")).unwrap();
    db.execute("UPDATE signal_deliveries SET state='delivered',delivered_at_unix_nano=1 WHERE submission_id=?1", [delivered.submission_id.to_string()]).unwrap();
    db.execute(
        "UPDATE signal_deliveries SET state='failed',last_error_code='test' WHERE submission_id=?1",
        [failed.submission_id.to_string()],
    )
    .unwrap();
    let tx = db.transaction().unwrap();
    store::maintain(&tx, &config, store::now()).unwrap();
    tx.commit().unwrap();
    for table in ["submissions", "signal_deliveries"] {
        assert_eq!(rows(&db, table, &delivered.submission_id), 0);
        assert_eq!(rows(&db, table, &pending.submission_id), 1);
        assert_eq!(rows(&db, table, &failed.submission_id), 1);
    }
    assert!(
        store::admit(&mut db, &config, &log("delivered"))
            .unwrap()
            .duplicate
    );
    db.execute("UPDATE record_keys SET admitted_at_unix_nano=1", [])
        .unwrap();
    let tx = db.transaction().unwrap();
    store::maintain(&tx, &config, store::now()).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM record_keys", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM store_meta WHERE key LIKE 'receipt:%'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert!(
        !store::admit(&mut db, &config, &log("delivered"))
            .unwrap()
            .duplicate
    );
}
#[test]
fn eviction_reclaims_failed_payload_but_preserves_future_and_live_claims() {
    for state in ["failed", "newer", "claimed"] {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(dir.path());
        config.max_store_bytes = log("one").to_canonical_json().len() as u64;
        config.disk_bound_policy = DiskBoundPolicy::EvictOldest;
        let mut db = store::open(&config.store_path).unwrap();
        let first = store::admit(&mut db, &config, &log("one")).unwrap();
        match state {
            "failed" => {
                db.execute(
                    "UPDATE signal_deliveries SET state='failed',last_error_code='test'",
                    [],
                )
                .unwrap();
            }
            "newer" => {
                db.execute("UPDATE submissions SET envelope_version=99", [])
                    .unwrap();
            }
            _ => {
                db.execute("UPDATE signal_deliveries SET state='claimed',claimed_by='other',claim_expires_at_unix_nano=?1",[i64::MAX]).unwrap();
            }
        }
        let before: Vec<u8> = db
            .query_row("SELECT envelope FROM submissions", [], |r| r.get(0))
            .unwrap();
        let result = store::admit(&mut db, &config, &log("two"));
        let bytes: Vec<u8> = db
            .query_row(
                "SELECT envelope FROM submissions WHERE submission_id=?1",
                [first.submission_id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        if state == "failed" {
            result.unwrap();
            assert!(bytes.is_empty());
            assert_eq!(
                query::status(&db, &config, StatusQuery::Summary)
                    .unwrap()
                    .failed
                    .logs,
                1
            );
        } else {
            assert!(result.is_err());
            assert_eq!(bytes, before);
        }
    }
}
#[test]
fn corrupt_and_oversized_stored_rows_do_not_starve_later_rows() {
    for corrupt in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let mut db = store::open(&config.store_path).unwrap();
        let bad = store::admit(&mut db, &config, &log("bad")).unwrap();
        {
            if corrupt {
                db.execute("UPDATE submissions SET envelope=x'ffff'", [])
                    .unwrap();
            } else {
                db.execute(
                    "UPDATE submissions SET envelope_bytes=?1",
                    [
                        i64::try_from(crate::constants::DEFAULT_OTLP_QUEUE_BYTE_CAPACITY + 1)
                            .unwrap(),
                    ],
                )
                .unwrap();
            }
        }
        drop(db);
        let client = conformance::open_manual(|| {
            DurableTelemetryClient::open_with_exporter(
                config,
                Arc::new(ScriptedExporter::new(dir.path())),
            )
        })
        .unwrap();
        let good = client.emit(log("good")).unwrap();
        assert_eq!(
            client
                .flush_submission(&good.submission_id, DEADLINE)
                .unwrap()
                .delivered
                .logs,
            1
        );
        let status = client
            .status(StatusQuery::Submissions(vec![bad.submission_id]))
            .unwrap();
        assert!(matches!(
            status.submissions[0].signals[0].1,
            DeliveryState::Failed { .. }
        ));
    }
}
#[test]
fn oversized_admission_is_typed_and_does_not_block_small_submission() {
    let dir = tempfile::tempdir().unwrap();
    let client = conformance::open_manual(|| {
        DurableTelemetryClient::open_with_exporter(
            config(dir.path()),
            Arc::new(ScriptedExporter::new(dir.path())),
        )
    })
    .unwrap();
    let mut huge = fixture("logs");
    huge.logs[0].record.body = Some(sc_observability_types::otlp::signals::AnyValue::String(
        "x".repeat(64 * 1024),
    ));
    huge.logs = vec![
        huge.logs[0].clone();
        crate::constants::DEFAULT_OTLP_QUEUE_BYTE_CAPACITY / (64 * 1024) + 1
    ];
    huge.validate().unwrap();
    let error = client.emit(huge).unwrap_err();
    assert_eq!(error.code(), &crate::error_codes::DURABLE_OVERSIZE);
    let good = client.emit(log("good")).unwrap();
    assert_eq!(
        client
            .flush_submission(&good.submission_id, DEADLINE)
            .unwrap()
            .delivered
            .logs,
        1
    );
}
#[test]
fn unknown_delivery_state_never_reports_success() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let receipt = store::admit(&mut db, &config, &log("corrupt-state")).unwrap();
    let scope = query::snapshot(&db, Some(&receipt.submission_id)).unwrap();
    db.execute_batch(
        "PRAGMA ignore_check_constraints=ON; UPDATE signal_deliveries SET state='garbage'",
    )
    .unwrap();
    assert!(query::report(&db, &scope).is_err());
    assert!(query::status(&db, &config, StatusQuery::Summary).is_err());
}
#[test]
fn oversized_yaml_is_a_config_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.yaml");
    std::fs::write(
        &path,
        "x".repeat(usize::try_from(crate::constants::TELEMETRY_CONFIG_MAX_BYTES + 1).unwrap()),
    )
    .unwrap();
    assert!(matches!(
        load_telemetry_file(&path),
        Err(TelemetryConfigError::ConfigFile { .. })
    ));
}
#[test]
fn flush_after_shutdown_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let client = DurableTelemetryClient::open_with_exporter(
        config(dir.path()),
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    client.shutdown(DEADLINE).unwrap();
    assert!(matches!(
        client.flush(DEADLINE),
        Err(TelemetryClientError::Admission(
            AdmissionError::Closed { .. }
        ))
    ));
}
#[test]
fn database_mutex_timeout_is_typed_and_drop_releases() {
    let dir = tempfile::tempdir().unwrap();
    let db = super::super::database::Database::new(
        super::super::store::open(&dir.path().join("lock.db")).unwrap(),
    )
    .unwrap();
    let held = db.lock().unwrap();
    assert_eq!(
        db.lock_for(Duration::ZERO).err().unwrap().code(),
        &crate::error_codes::DURABLE_LOCK_TIMEOUT
    );
    drop(held);
    assert!(db.try_lock().is_ok());
}

#[test]
fn concurrent_sqlite_writer_reports_coded_lock_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let store_path = config.store_path.clone();
    let client = conformance::open_manual(|| {
        DurableTelemetryClient::open_with_exporter(
            config,
            Arc::new(ScriptedExporter::new(dir.path())),
        )
    })
    .unwrap();
    let writer = rusqlite::Connection::open(store_path).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();

    let error = client.emit(log("blocked")).unwrap_err();
    assert_eq!(error.code(), &crate::error_codes::DURABLE_LOCK_TIMEOUT);

    writer.execute_batch("COMMIT").unwrap();
    assert!(client.emit(log("after-release")).is_ok());
    assert!(client.shutdown(DEADLINE).is_ok());
}

#[test]
fn shutdown_after_successful_delivery_does_not_race_a_drain_writer() {
    let dir = tempfile::tempdir().unwrap();
    let client = DurableTelemetryClient::open_with_exporter(
        config(dir.path()),
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    let receipt = client.emit(log("delivered-before-shutdown")).unwrap();

    let report = client
        .flush_submission(&receipt.submission_id, DEADLINE)
        .expect("successful delivery");
    assert_eq!(report.delivered.logs, 1);
    assert!(
        client.shutdown(DEADLINE).is_ok(),
        "a completed delivery must not become a durable lock timeout during shutdown"
    );
}

#[test]
fn invalid_envelope_is_rejected_before_any_rows_are_inserted() {
    let dir = tempfile::tempdir().unwrap();
    let client = DurableTelemetryClient::open_with_exporter(
        config(dir.path()),
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    let mut input = SubmissionInput::new();
    let mut span = sc_observability_types::otlp::submission::SpanInput::new(
        "invalid-after-mutation".into(),
        sc_observability_types::Timestamp::now_utc(),
    );
    span.duration_nanos = Some(1);
    input.spans.push(span);
    let mut envelope = SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap();
    envelope.spans[0].record.end_time = sc_observability_types::Timestamp::UNIX_EPOCH;
    let db = client.owner.shared.db.lock().unwrap();
    let before_submissions: i64 = db
        .query_row("SELECT count(*) FROM submissions", [], |row| row.get(0))
        .unwrap();
    let before_deliveries: i64 = db
        .query_row("SELECT count(*) FROM signal_deliveries", [], |row| {
            row.get(0)
        })
        .unwrap();
    drop(db);

    assert!(client.emit(envelope).is_err());

    let db = client.owner.shared.db.lock().unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM submissions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        before_submissions
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM signal_deliveries", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        before_deliveries
    );
}

#[test]
fn worker_database_errors_are_retained_and_poisoned_wake_is_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    store::open(&config.store_path)
        .unwrap()
        .execute_batch("DROP TABLE signal_deliveries")
        .unwrap();
    let client = DurableTelemetryClient::open_with_exporter(
        config,
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    let start = Instant::now();
    loop {
        let generation = client.owner.shared.generation();
        if client.owner.shared.last_error.lock().unwrap().is_some() {
            break;
        }
        assert!(
            start.elapsed() < DEADLINE,
            "background database failure was not retained"
        );
        client
            .owner
            .shared
            .wait_since(generation, DEADLINE.saturating_sub(start.elapsed()));
    }
    client.owner.shared.stop.store(true, Ordering::Release);
    client.owner.shared.notify();
    worker::join(&client.owner.shared, DEADLINE);
    let shared = Arc::clone(&client.owner.shared);
    let _ = std::thread::spawn(move || {
        let _held = shared.wake.lock().unwrap();
        panic!("simulate an unwinding notifier");
    })
    .join();
    client.owner.shared.notify();
    drop(client);
}

#[test]
fn runtime_invalid_ca_fails_open_without_consuming_pending_attempts() {
    use crate::config::{ExporterBackend, OtelConfig, OtlpProtocol};
    use crate::sync_http::submission::{SyncHttpConfig, exporter_for};
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    store::admit(&mut db, &config, &log("ca-open")).unwrap();
    drop(db);
    // The frozen durable config has no CA field. Inject only that prepared
    // input at the existing factory seam; exporter construction is production.
    let result = DurableTelemetryClient::open_with_exporter_factory(config.clone(), |_, _| {
        let mut otel = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
        otel.enabled = true;
        otel.ca_file = Some(dir.path().join("absent-ca.pem"));
        let (worker, bounds) = SyncHttpConfig::from_otel(&otel)?;
        exporter_for(worker, bounds)
    });
    assert!(matches!(result, Err(TelemetryClientError::Config(_))));
    let db = store::reader(&config.store_path).unwrap();
    let (state, attempts): (String, u32) = db
        .query_row("SELECT state,attempts FROM signal_deliveries", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(state, "pending");
    assert_eq!(attempts, 0);
}
