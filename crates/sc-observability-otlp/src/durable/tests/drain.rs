use super::*;
#[test]
fn terminal_marks_failed() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(ScriptedExporter::new(dir.path()));
    exporter.set_outcome(Signal::Logs, DeliveryOutcome::Fail);
    let client = conformance::open_manual(|| {
        DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter)
    })
    .unwrap();
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
    let client = conformance::open_manual(|| {
        DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter.clone())
    })
    .unwrap();
    let receipt = client.emit(log("retry")).unwrap();
    let _clock = FrozenClock::new();
    assert!(drain_once_bounded(
        &client.owner.shared,
        exporter.as_ref(),
        Signal::Logs
    ));
    let retry_due = frozen_now()
        .expect("manual drain freezes its first attempt")
        .saturating_add(store::nanos(Duration::from_millis(
            crate::constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS,
        )));
    FROZEN_NOW.set(Some(retry_due));
    assert!(drain_once_bounded(
        &client.owner.shared,
        exporter.as_ref(),
        Signal::Logs
    ));
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
    let mut stalled_config = config(dir.path());
    // The exporter is deliberately held across a bounded shutdown. Keep its
    // ownership fence alive until the explicit release and worker join finish.
    stalled_config.lease_duration = DEADLINE.saturating_mul(2);
    let client =
        DurableTelemetryClient::open_with_exporter(stalled_config, exporter.clone()).unwrap();
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
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    mode: String,
}
impl ChildProcess {
    fn signal(&mut self, expected: &str, phase: &str) {
        loop {
            let mut line = String::new();
            match self.stdout.read_line(&mut line) {
                Ok(0) => {
                    let status = self.child.take().unwrap().wait().unwrap();
                    panic!(
                        "child {} exited before {phase}: exit code {:?}",
                        self.mode,
                        status.code()
                    );
                }
                Ok(_) if line.trim_end() == expected => return,
                Ok(_) if matches!(line.trim_end(), "READY" | "COMPLETE") => panic!(
                    "child {} sent {line:?} instead of {expected} during {phase}",
                    self.mode
                ),
                Ok(_) => {}
                Err(error) => panic!("failed to read child {} {phase}: {error}", self.mode),
            }
        }
    }
    fn ready(&mut self) {
        self.signal("READY", "readiness");
    }
    fn complete(&mut self) {
        self.stdin.write_all(b"COMPLETE\n").unwrap();
        self.stdin.flush().unwrap();
    }
    fn assert_running(&mut self, phase: &str) {
        if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
            panic!(
                "child {} exited before {phase}: exit code {:?}",
                self.mode,
                status.code()
            );
        }
    }
    fn kill(&mut self) {
        self.child.as_mut().unwrap().kill().unwrap();
    }
    fn wait(&mut self) -> std::process::ExitStatus {
        if self.mode == "controlled" {
            self.signal("COMPLETE", "completion");
        }
        self.child.take().unwrap().wait().unwrap()
    }
}
impl Drop for ChildProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn child(path: &Path, mode: &str) -> ChildProcess {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "durable::tests::drain::child_drainer",
            "--ignored",
            "--nocapture",
            "--quiet",
        ])
        .env("SC_D33_TEST_STORE", path)
        .env("SC_D33_TEST_MODE", mode)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    ChildProcess {
        child: Some(child),
        stdin,
        stdout,
        mode: mode.to_owned(),
    }
}
fn signal_ready() {
    signal_parent("READY");
}
fn signal_complete() {
    signal_parent("COMPLETE");
}
fn signal_parent(signal: &str) {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{signal}").unwrap();
    stdout.flush().unwrap();
}
fn wait_until_store_is_empty(path: &Path, children: &mut [&mut ChildProcess]) {
    loop {
        for child in &mut *children {
            child.assert_running("the store drained");
        }
        let db = store::reader(&config(path).store_path).unwrap();
        if query::snapshot(&db, None).unwrap().is_empty() {
            return;
        }
        std::thread::yield_now();
    }
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
    if mode == "exit" {
        return;
    }
    if mode == "exit-after-ready" {
        signal_ready();
        return;
    }
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
    if mode == "controlled" {
        signal_ready();
        let mut complete = String::new();
        std::io::stdin().read_line(&mut complete).unwrap();
        assert_eq!(
            complete, "COMPLETE\n",
            "parent ends concurrent drainers once clean"
        );
        client.shutdown(DEADLINE).unwrap();
        signal_complete();
        return;
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
#[test]
fn child_exit_before_readiness_reports_status() {
    let dir = tempfile::tempdir().unwrap();
    let mut process = child(dir.path(), "exit");
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| process.ready()))
        .expect_err("an exited child cannot become ready");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.contains("child exit exited before readiness"));
    assert!(message.contains("exit code Some(0)"), "{message}");
}
#[test]
fn child_exit_before_store_drains_reports_status() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 1);
    let mut process = child(dir.path(), "exit-after-ready");
    process.ready();
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wait_until_store_is_empty(dir.path(), &mut [&mut process]);
    }))
    .expect_err("an exited child cannot drain the store");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.contains("child exit-after-ready exited before the store drained"));
    assert!(message.contains("exit code Some(0)"), "{message}");
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
    let mut first = child(dir.path(), "controlled");
    let mut second = child(dir.path(), "controlled");
    first.ready();
    second.ready();
    wait_until_store_is_empty(dir.path(), &mut [&mut first, &mut second]);
    first.complete();
    second.complete();
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
    struct HandshakeExporter {
        entered: std::sync::mpsc::SyncSender<()>,
        released: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl SubmissionExporter for HandshakeExporter {
        fn export(
            &self,
            _: Signal,
            _: &[SubmissionEnvelope],
        ) -> Result<(), SubmissionExportFailure> {
            self.entered.send(()).unwrap();
            // Sender drop also releases this worker during assertion unwinding.
            let _ = self.released.lock().unwrap().recv();
            Ok(())
        }
    }
    let _clock = FrozenClock::new();
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(ScriptedExporter::new(dir.path()));
    let client = conformance::open_gated(config(dir.path()), &exporter);
    client.emit(log("pending")).unwrap();
    // This test exercises flush's writer-lock deadline, not scripted settlement.
    client
        .owner
        .shared
        .drain_on_flush_only
        .store(false, Ordering::Release);
    let now = frozen_now();
    std::thread::scope(|scope| {
        let (entered, receive_entered) = std::sync::mpsc::sync_channel(1);
        let (release, released) = std::sync::mpsc::channel();
        let (completed, receive_completed) = std::sync::mpsc::sync_channel(1);
        let shared = &client.owner.shared;
        let drain = scope.spawn(move || {
            FROZEN_NOW.set(now);
            let _clock = FrozenClock;
            worker::drain_for_handshake_test(
                shared,
                &HandshakeExporter {
                    entered,
                    released: Mutex::new(released),
                },
                Signal::Logs,
            );
            let _ = completed.send(());
        });
        // Entry is established by the exporter handshake. The deadline is a
        // failure-only watchdog; panic or early return also closes the sender.
        if receive_entered.recv_timeout(DEADLINE).is_err() {
            shared.stop.store(true, Ordering::Release);
            shared.notify();
            drop(release);
            let _ = drain.join();
            panic!("drain did not reach export");
        }
        let locked = client.owner.shared.db.lock().unwrap();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let client = &client;
        scope.spawn(move || {
            let _ = send.send(client.flush(Duration::ZERO));
        });
        let result = receive.recv_timeout(DEADLINE);
        let shutdown = client.shutdown(Duration::ZERO);
        drop(locked);
        drop(release);
        match receive_completed.recv_timeout(DEADLINE) {
            Ok(()) => drain.join().unwrap(),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                shared.stop.store(true, Ordering::Release);
                shared.notify();
                let _ = drain.join();
                panic!("drain did not complete after release");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                drain.join().unwrap();
                panic!("drain exited without completion");
            }
        }
        assert!(shutdown.is_err());
        assert!(result.expect("flush waited for the writer mutex").is_err());
    });
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
