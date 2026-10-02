use super::*;
#[test]
fn disk_bound_reject_new() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    config.max_store_bytes = log("one").to_canonical_json().len() as u64;
    let mut db = store::open(&config.store_path).unwrap();
    store::admit(&mut db, &config, &log("one")).unwrap();
    assert!(matches!(
        store::admit(&mut db, &config, &log("two")),
        Err(TelemetryClientError::Admission(
            AdmissionError::DiskBoundExceeded { .. }
        ))
    ));
    let status = query::status(&db, &config, StatusQuery::Summary).unwrap();
    assert_eq!(status.rejected_by_disk_bound, 1);
    assert_eq!(status.pending.logs, 1);
}
#[test]
fn disk_bound_evict_oldest_counted() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    config.max_store_bytes = log("one").to_canonical_json().len() as u64;
    config.disk_bound_policy = DiskBoundPolicy::EvictOldest;
    let mut db = store::open(&config.store_path).unwrap();
    let old = store::admit(&mut db, &config, &log("one")).unwrap();
    store::admit(&mut db, &config, &log("two")).unwrap();
    let status = query::status(
        &db,
        &config,
        StatusQuery::Submissions(vec![old.submission_id.clone()]),
    )
    .unwrap();
    assert_eq!(status.evicted_by_disk_bound, 1);
    assert_eq!(status.store_bytes, config.max_store_bytes);
    assert!(matches!(
        status.submissions[0].signals[0].1,
        DeliveryState::EvictedByDiskBound { .. }
    ));
    assert_eq!(
        db.query_row(
            "SELECT length(envelope) FROM submissions WHERE submission_id=?1",
            [old.submission_id.to_string()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert!(
        store::admit(&mut db, &config, &log("one"))
            .unwrap()
            .duplicate
    );
}
#[test]
fn retention_purges_delivered_only() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let old = store::admit(&mut db, &config, &log("old")).unwrap();
    store::admit(&mut db, &config, &log("pending")).unwrap();
    db.execute("UPDATE signal_deliveries SET state='delivered',delivered_at_unix_nano=1 WHERE submission_id=?1",[old.submission_id.to_string()]).unwrap();
    let tx = db.transaction().unwrap();
    store::maintain(&tx, &config, store::now()).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        query::status(&db, &config, StatusQuery::Summary)
            .unwrap()
            .pending
            .logs,
        1
    );
    let duplicate = store::admit(&mut db, &config, &log("old")).unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.submission_id, old.submission_id);
}
#[test]
fn backend_queue_full_pauses_no_eviction() {
    let dir = tempfile::tempdir().unwrap();
    let client = DurableTelemetryClient::open_with_exporter(
        config(dir.path()),
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    let held = client
        .owner
        .shared
        .credits
        .reserve(crate::constants::DEFAULT_OTLP_QUEUE_BYTE_CAPACITY)
        .unwrap();
    let receipt = client.emit(log("blocked")).unwrap();
    assert!(
        client
            .flush_submission(&receipt.submission_id, Duration::from_millis(20))
            .is_err()
    );
    let start = Instant::now();
    loop {
        let generation = client.owner.shared.generation();
        if client
            .owner
            .shared
            .waiting_for_credits
            .lock()
            .unwrap()
            .contains(&Signal::Logs)
        {
            break;
        }
        assert!(
            start.elapsed() < DEADLINE,
            "log worker did not reach credit wait"
        );
        client
            .owner
            .shared
            .wait_since(generation, DEADLINE.saturating_sub(start.elapsed()));
    }
    let state: String = client
        .owner
        .shared
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT state FROM signal_deliveries WHERE submission_id=?1 AND signal='logs'",
            [receipt.submission_id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        state, "claimed",
        "credit exhaustion preserves the actual claim"
    );
    let status = client.status(StatusQuery::Summary).unwrap();
    assert_eq!(status.pending.logs, 1);
    assert_eq!(status.evicted_by_disk_bound, 0);
    drop(held);
    assert_eq!(
        client
            .flush_submission(&receipt.submission_id, DEADLINE)
            .unwrap()
            .delivered
            .logs,
        1
    );
}

#[test]
fn batch_larger_than_available_credits_exports_in_prefixes() {
    let dir = tempfile::tempdir().unwrap();
    let _clock = FrozenClock::new();
    let exporter = Arc::new(ScriptedExporter::new(dir.path()));
    let client = conformance::open_gated(config(dir.path()), &exporter);
    let first = log("one");
    let second = log("two");
    let size = first
        .to_canonical_json()
        .len()
        .max(second.to_canonical_json().len());
    let held = client
        .owner
        .shared
        .credits
        .reserve(crate::constants::DEFAULT_OTLP_QUEUE_BYTE_CAPACITY - size)
        .unwrap();
    let first = client.emit(first).unwrap();
    let second = client.emit(second).unwrap();
    // Both rows are committed before a drain begins, guaranteeing one claimed
    // batch that must split. Each explicit step traverses the production drain.
    assert!(worker::drain_once_for_test(
        &client.owner.shared,
        exporter.as_ref(),
        Signal::Logs
    ));
    let status = client.status(StatusQuery::Summary).unwrap();
    assert_eq!(status.delivered_retained.logs, 1);
    assert_eq!(status.pending.logs, 1);
    assert!(worker::drain_once_for_test(
        &client.owner.shared,
        exporter.as_ref(),
        Signal::Logs
    ));
    assert_eq!(*exporter.batches.lock().unwrap(), vec![1, 1]);
    for receipt in [first, second] {
        assert_eq!(
            client
                .flush_submission(&receipt.submission_id, Duration::ZERO)
                .unwrap()
                .delivered
                .logs,
            1
        );
    }
    drop(held);
}
