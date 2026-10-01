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
