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
            store::now().saturating_add(store::nanos(Duration::from_secs(3600)))
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
    client
        .owner
        .shared
        .db
        .lock()
        .unwrap()
        .execute("UPDATE drain_lease SET expires_at_unix_nano=1", [])
        .unwrap();
    client.owner.shared.notify();
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
    let receipt = client.emit(log("stall")).unwrap();
    exporter.await_stall();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        let result = client.shutdown(Duration::from_millis(10));
        send.send((client, result)).unwrap();
    });
    let received = receive.recv_timeout(DEADLINE);
    // Always release the exporter before asserting, including a failing test.
    exporter.release();
    let (client, result) = received.expect("shutdown waited for the blocked exporter");
    assert!(result.is_err());
    thread.join().unwrap();
    assert!(client.emit(log("closed")).is_err());
    assert_eq!(client.shutdown(DEADLINE).unwrap(), FlushReport::default());
    worker::join(&client.owner.shared, DEADLINE);
    let status = client
        .status(StatusQuery::Submissions(vec![receipt.submission_id]))
        .unwrap();
    assert!(
        matches!(
            status.submissions[0].signals[0].1,
            DeliveryState::Delivered { .. }
        ),
        "a successful in-flight export commits even after shutdown stops workers"
    );
}

struct ChildProcess {
    child: Option<std::process::Child>,
    ready: std::sync::mpsc::Receiver<()>,
    mode: String,
    address: std::net::SocketAddr,
}
impl ChildProcess {
    fn ready(&self) {
        self.ready
            .recv_timeout(DEADLINE)
            .unwrap_or_else(|error| panic!("child {} readiness deadline: {error}", self.mode));
    }
    fn kill(&mut self) {
        self.child.as_mut().unwrap().kill().unwrap();
    }
    fn wait(&mut self) -> std::process::ExitStatus {
        let mut child = self.child.take().unwrap();
        let pid = child.id();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = send.send(child.wait());
        });
        match receive.recv_timeout(DEADLINE) {
            Ok(status) => status.unwrap(),
            Err(error) => {
                // The wait thread owns Child. Kill only this still-unreaped PID.
                #[cfg(unix)]
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .spawn();
                #[cfg(windows)]
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .spawn();
                let _ = receive.recv_timeout(DEADLINE);
                panic!(
                    "child {} completion deadline; pending process {pid}: {error}",
                    self.mode
                );
            }
        }
    }
}
impl Drop for ChildProcess {
    fn drop(&mut self) {
        // Unblock the readiness listener even if the child failed before signaling.
        let _ = std::net::TcpStream::connect_timeout(&self.address, DEADLINE);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let (send, receive) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let _ = send.send(child.wait());
            });
            let _ = receive.recv_timeout(DEADLINE);
        }
    }
}
fn child(path: &Path, mode: &str) -> ChildProcess {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "durable::tests::drain::child_drainer",
            "--ignored",
        ])
        .env("SC_D33_TEST_STORE", path)
        .env("SC_D33_TEST_MODE", mode)
        .env("SC_D33_TEST_READY", address.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .unwrap();
    let (send, ready) = std::sync::mpsc::sync_channel(1);
    // The listener owns readiness; no file-existence or scheduling poll loop.
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::Read;
            let _ = stream.set_read_timeout(Some(DEADLINE));
            let mut byte = [0];
            if stream.read_exact(&mut byte).is_ok() && byte == [1] {
                let _ = send.send(());
            }
        }
    });
    ChildProcess {
        child: Some(child),
        ready,
        mode: mode.to_owned(),
        address,
    }
}
fn signal_ready() {
    let mut stream =
        std::net::TcpStream::connect(std::env::var("SC_D33_TEST_READY").unwrap()).unwrap();
    stream.write_all(&[1]).unwrap();
}
struct CrashExporter {
    inner: ScriptedExporter,
    ready: std::sync::Once,
}
impl SubmissionExporter for CrashExporter {
    fn export(
        &self,
        signal: Signal,
        envelopes: &[SubmissionEnvelope],
    ) -> Result<(), SubmissionExportFailure> {
        self.inner.export(signal, envelopes)?;
        // Simulate collector acceptance before the delivered transaction commits.
        self.ready.call_once(signal_ready);
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
            ready: std::sync::Once::new(),
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
        signal_ready();
        loop {
            std::thread::park();
        }
    }
    if mode == "drain" {
        signal_ready();
    }
    client.flush(DEADLINE).unwrap();
    client.shutdown(DEADLINE).unwrap();
}
#[test]
fn receipt_after_commit() {
    let dir = tempfile::tempdir().unwrap();
    let mut process = child(dir.path(), "receipt");
    process.ready();
    process.kill();
    process.wait();
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
    first.ready();
    second.ready();
    assert!(first.wait().success());
    assert!(second.wait().success());
    let counts = delivery_counts(dir.path());
    assert_eq!(counts.len(), receipts.len() * 4);
    assert_bounded_duplicates(dir.path(), &counts, receipts.len());
}
#[test]
fn crash_mid_drain_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let receipts = seed(dir.path(), crate::constants::DRAIN_BATCH_SIZE + 1);
    let mut process = child(dir.path(), "crash");
    process.ready();
    process.kill();
    process.wait();
    let mut replacement = child(dir.path(), "drain");
    replacement.ready();
    assert!(replacement.wait().success());
    let counts = delivery_counts(dir.path());
    assert_eq!(counts.len(), receipts.len() * 4);
    assert!(counts.values().all(|count| *count >= 1));
    assert_bounded_duplicates(dir.path(), &counts, receipts.len());
}

#[test]
fn flush_deadline_does_not_wait_for_the_writer_mutex() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(ScriptedExporter::new(dir.path()));
    exporter.set_outcome(Signal::Logs, DeliveryOutcome::Stall);
    let client =
        DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter.clone()).unwrap();
    client.emit(log("pending")).unwrap();
    exporter.await_stall();
    let locked = client.owner.shared.db.lock().unwrap();
    std::thread::scope(|scope| {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let client = &client;
        scope.spawn(move || {
            send.send(client.flush(Duration::ZERO)).unwrap();
        });
        let result = receive.recv_timeout(DEADLINE);
        assert!(client.shutdown(Duration::ZERO).is_err());
        drop(locked);
        assert!(result.expect("flush waited for the writer mutex").is_err());
    });
    exporter.release();
    worker::join(&client.owner.shared, DEADLINE);
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

// A scheduler delay can expire the deliberately short lease. Bound resends by
// observed acquisitions rather than asserting exactly-once delivery.
fn assert_bounded_duplicates(path: &Path, counts: &HashMap<String, usize>, records: usize) {
    let db = store::open(&config(path).store_path).unwrap();
    let acquisitions: i64 = db
        .query_row(
            "SELECT value FROM store_counters WHERE name='test_lease_acquisitions'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(counts.values().all(|count| *count >= 1));
    let duplicates = counts.values().sum::<usize>() - records * 4;
    let takeovers = usize::try_from(acquisitions.saturating_sub(1)).unwrap();
    assert!(
        duplicates <= takeovers * crate::constants::DRAIN_BATCH_SIZE * 4,
        "{duplicates} duplicate deliveries across {takeovers} possible takeovers"
    );
}
