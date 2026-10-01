use super::*;
#[test]
fn terminal_marks_failed() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(ScriptedExporter::new(dir.path()));
    exporter.set_outcome(Signal::Logs, DeliveryOutcome::Fail);
    let client = DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter).unwrap();
    let receipt = client.emit(log("failed")).unwrap();
    assert!(matches!(
        client.flush_submission(&receipt.submission_id, DEADLINE),
        Err(TelemetryClientError::Delivery(
            DeliveryError::TerminalFailure { .. }
        ))
    ));
    assert_eq!(client.status(StatusQuery::Summary).unwrap().failed.logs, 1);
}
struct RetryExporter {
    inner: ScriptedExporter,
    remaining: Mutex<u32>,
}
impl SubmissionExporter for RetryExporter {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure> {
        let mut remaining = self.remaining.lock().unwrap();
        if *remaining > 0 {
            *remaining -= 1;
            return Err(SubmissionExportFailure::Retryable(export_error()));
        }
        self.inner.export(signal, envelopes)
    }
}
#[test]
fn retryable_then_delivered() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(RetryExporter {
        inner: ScriptedExporter::new(dir.path()),
        remaining: Mutex::new(1),
    });
    let client = DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter).unwrap();
    let receipt = client.emit(log("retry")).unwrap();
    assert_eq!(
        client
            .flush_submission(&receipt.submission_id, DEADLINE)
            .unwrap()
            .delivered
            .logs,
        1
    );
    let status = client
        .status(StatusQuery::Submissions(vec![receipt.submission_id]))
        .unwrap();
    assert!(matches!(
        status.submissions[0].signals[0].1,
        DeliveryState::Delivered { attempts: 2, .. }
    ));
}
#[test]
fn lease_expiry_takeover() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let receipt = store::admit(&mut db, &config, &log("takeover")).unwrap();
    db.execute("INSERT INTO drain_lease VALUES(1,'expired',0,1)", [])
        .unwrap();
    db.execute("UPDATE signal_deliveries SET state='claimed',claimed_by='expired',claim_expires_at_unix_nano=1",[]).unwrap();
    drop(db);
    let client = DurableTelemetryClient::open_with_exporter(
        config,
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    assert_eq!(
        client
            .flush_submission(&receipt.submission_id, DEADLINE)
            .unwrap()
            .delivered
            .logs,
        1
    );
    let acquired: i64 = client
        .owner
        .shared
        .db
        .lock()
        .unwrap()
        .query_row("SELECT acquired_at_unix_nano FROM drain_lease", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        acquired > 1,
        "takeover records the new holder's acquisition time"
    );
}
#[test]
fn non_holder_flush_waits_then_acquires() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let mut db = store::open(&config.store_path).unwrap();
    let receipt = store::admit(&mut db, &config, &log("wait")).unwrap();
    db.execute(
        "INSERT INTO drain_lease VALUES(1,'other',?1,?2)",
        rusqlite::params![
            store::now(),
            store::now() + store::nanos(Duration::from_millis(100))
        ],
    )
    .unwrap();
    drop(db);
    let client = DurableTelemetryClient::open_with_exporter(
        config,
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    assert!(
        client
            .flush_submission(&receipt.submission_id, Duration::from_millis(10))
            .is_err()
    );
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
fn shutdown_deadline_does_not_join_stalled_exporter() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(ScriptedExporter::new(dir.path()));
    exporter.set_outcome(Signal::Logs, DeliveryOutcome::Stall);
    let client =
        DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter.clone()).unwrap();
    client.emit(log("stall")).unwrap();
    let start = Instant::now();
    assert!(client.shutdown(Duration::from_millis(10)).is_err());
    assert!(start.elapsed() < Duration::from_millis(500));
    assert!(client.emit(log("closed")).is_err());
    assert_eq!(client.shutdown(DEADLINE).unwrap(), FlushReport::default());
    exporter.release();
}

struct ChildProcess(std::process::Child);
impl Drop for ChildProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn child(path: &Path, mode: &str) -> ChildProcess {
    ChildProcess(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "durable::tests::drain::child_drainer",
                "--ignored",
            ])
            .env("SC_D33_TEST_STORE", path)
            .env("SC_D33_TEST_MODE", mode)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}
struct CrashExporter {
    inner: ScriptedExporter,
    ready: PathBuf,
}
impl SubmissionExporter for CrashExporter {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure> {
        self.inner.export(signal, envelopes)?;
        // Simulate collector acceptance before the delivered transaction commits.
        std::fs::write(&self.ready, b"accepted").unwrap();
        loop {
            std::thread::park();
        }
    }
}
#[test]
#[ignore = "subprocess helper; invoked by durable multiprocess tests"]
fn child_drainer() {
    let path = PathBuf::from(std::env::var_os("SC_D33_TEST_STORE").expect("parent supplies store"));
    let mode = std::env::var("SC_D33_TEST_MODE").unwrap();
    let exporter: Arc<dyn SubmissionExporter> = if mode == "crash" {
        Arc::new(CrashExporter {
            inner: ScriptedExporter::new(&path),
            ready: path.join("accepted"),
        })
    } else {
        Arc::new(ScriptedExporter::new(&path))
    };
    let client = DurableTelemetryClient::open_with_exporter(config(&path), exporter).unwrap();
    if mode == "receipt" {
        let receipt = client.emit(log("receipt")).unwrap();
        let mut file = std::fs::File::create(path.join("receipt.json")).unwrap();
        file.write_all(serde_json::to_string(&receipt).unwrap().as_bytes())
            .unwrap();
        file.sync_all().unwrap();
        loop {
            std::thread::park();
        }
    }
    client.flush(Duration::from_secs(5)).unwrap();
    client.shutdown(Duration::from_secs(5)).unwrap();
}
#[test]
fn receipt_after_commit() {
    let dir = tempfile::tempdir().unwrap();
    let mut process = child(dir.path(), "receipt");
    wait_until(|| {
        dir.path().join("receipt.json").exists()
            && serde_json::from_slice::<AdmissionReceipt>(
                &std::fs::read(dir.path().join("receipt.json")).unwrap(),
            )
            .is_ok()
    });
    process.0.kill().unwrap();
    process.0.wait().unwrap();
    let receipt: AdmissionReceipt =
        serde_json::from_slice(&std::fs::read(dir.path().join("receipt.json")).unwrap()).unwrap();
    let db = store::open(&config(dir.path()).store_path).unwrap();
    assert_eq!(
        query::snapshot(&db, Some(&receipt.submission_id))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        db.query_row("PRAGMA synchronous", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
}
fn seed(path: &Path, amount: usize) -> Vec<AdmissionReceipt> {
    let config = config(path);
    let mut db = store::open(&config.store_path).unwrap();
    let mut envelope = fixture("logs");
    envelope.spans = fixture("traces").spans;
    envelope.metrics = fixture("metric_gauge").metrics;
    envelope.profiles = fixture("profiles").profiles;
    (0..amount)
        .map(|n| {
            envelope.record_key = Some(format!("record-{n}").parse().unwrap());
            store::admit(&mut db, &config, &envelope).unwrap()
        })
        .collect()
}

fn delivery_counts(path: &Path) -> HashMap<String, usize> {
    let text = std::fs::read_to_string(path.join("deliveries.jsonl")).unwrap();
    let mut counts = HashMap::new();
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        *counts
            .entry(format!(
                "{}:{}",
                value["signal"].as_str().unwrap(),
                value["key"].as_str().unwrap()
            ))
            .or_default() += 1;
    }
    counts
}
#[test]
fn two_process_drainers_no_loss() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = seed(dir.path(), crate::constants::DRAIN_BATCH_SIZE * 2);
    let mut first = child(dir.path(), "drain");
    let mut second = child(dir.path(), "drain");
    assert!(first.0.wait().unwrap().success());
    assert!(second.0.wait().unwrap().success());
    let counts = delivery_counts(dir.path());
    assert_eq!(counts.len(), receipts.len() * 4);
    assert!(
        counts.values().all(|count| *count == 1),
        "no takeover occurred: {counts:?}"
    );
}
#[test]
fn crash_mid_drain_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = seed(dir.path(), crate::constants::DRAIN_BATCH_SIZE + 1);
    let mut process = child(dir.path(), "crash");
    wait_until(|| dir.path().join("accepted").exists());
    process.0.kill().unwrap();
    process.0.wait().unwrap();
    let mut replacement = child(dir.path(), "drain");
    assert!(replacement.0.wait().unwrap().success());
    let counts = delivery_counts(dir.path());
    assert_eq!(counts.len(), receipts.len() * 4);
    assert!(counts.values().all(|count| *count >= 1));
    assert!(
        counts.values().sum::<usize>() - receipts.len() * 4
            <= crate::constants::DRAIN_BATCH_SIZE * 4
    );
}

#[test]
fn flush_deadline_does_not_wait_for_the_writer_mutex() {
    let dir = tempfile::tempdir().unwrap();
    let client = DurableTelemetryClient::prepare(config(dir.path())).unwrap();
    client.emit(log("pending")).unwrap();
    let locked = client.owner.shared.db.lock().unwrap();
    let start = Instant::now();
    assert!(client.flush(Duration::from_millis(10)).is_err());
    assert!(start.elapsed() < Duration::from_millis(500));
    assert!(client.shutdown(Duration::ZERO).is_err());
    drop(locked);
}

#[test]
fn retry_budget_exhaustion_is_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    let mut retry = SyncHttpRetryPolicyDto::default();
    retry.max_retries = Some(1);
    cfg.sync_http_retry = Some(retry);
    let exporter = Arc::new(RetryExporter {
        inner: ScriptedExporter::new(dir.path()),
        remaining: Mutex::new(u32::MAX),
    });
    let client = DurableTelemetryClient::open_with_exporter(cfg, exporter).unwrap();
    let receipt = client.emit(log("exhausted")).unwrap();
    assert!(matches!(
        client.flush_submission(&receipt.submission_id, DEADLINE),
        Err(TelemetryClientError::Delivery(
            DeliveryError::TerminalFailure { .. }
        ))
    ));
    let status = client
        .status(StatusQuery::Submissions(vec![receipt.submission_id]))
        .unwrap();
    assert!(matches!(
        status.submissions[0].signals[0].1,
        DeliveryState::Failed { attempts: 2, .. }
    ));
}
