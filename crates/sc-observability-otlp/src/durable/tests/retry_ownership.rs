//! Count real HTTP attempts across durable flushes, including profiles.
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Collector {
    address: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    paths: Arc<Mutex<Vec<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Collector {
    fn new(recover_after: Option<usize>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let paths = Arc::new(Mutex::new(Vec::new()));
        let (thread_stop, thread_calls, thread_paths) =
            (stop.clone(), calls.clone(), paths.clone());
        let worker = std::thread::spawn(move || {
            loop {
                let (mut stream, _) = listener.accept().unwrap();
                if thread_stop.load(Ordering::Acquire) {
                    break;
                }
                stream.set_read_timeout(Some(DEADLINE)).unwrap();
                let (path, _) = read_http(&mut stream);
                thread_paths.lock().unwrap().push(path);
                let call = thread_calls.fetch_add(1, Ordering::AcqRel) + 1;
                let status = if recover_after.is_some_and(|failed| call > failed) {
                    "200 OK"
                } else {
                    "503 Service Unavailable"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            }
        });
        Self {
            address,
            stop,
            calls,
            paths,
            worker: Some(worker),
        }
    }
    fn configure(&self, path: &Path) -> TelemetryClientConfig {
        let mut config = config(path);
        config.endpoint = format!("http://{}", self.address);
        let mut retry = SyncHttpRetryPolicyDto::default();
        retry.max_retries = Some(3);
        retry.initial_backoff_ms = Some(1);
        retry.max_backoff_ms = Some(1);
        retry.retry_jitter_percent = Some(0);
        config.sync_http_retry = Some(retry);
        config
    }
}
impl Drop for Collector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn read_http(stream: &mut TcpStream) -> (String, Vec<u8>) {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let end = loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..end]).unwrap();
    let path = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    let size: usize = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap();
    while bytes.len() < end + size {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
    }
    (path, bytes[end..end + size].to_vec())
}

#[test]
fn exhausted_transport_is_terminal_across_drain_reruns_including_profiles() {
    for (name, path) in [
        ("logs", "/v1/logs"),
        ("profiles", "/v1development/profiles"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let collector = Collector::new(None);
        let config = collector.configure(dir.path());
        let client = conformance::open_real_gated(config.clone());
        let receipt = client.emit(fixture(name)).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                client.flush_submission(&receipt.submission_id, DEADLINE),
                Err(TelemetryClientError::Delivery(
                    DeliveryError::TerminalFailure { .. }
                ))
            ));
        }
        let status = client
            .status(StatusQuery::Submissions(vec![
                receipt.submission_id.clone(),
            ]))
            .unwrap();
        assert!(matches!(
            status.submissions[0].signals[0].1,
            DeliveryState::Failed { attempts: 1, .. }
        ));
        let _ = client.shutdown(DEADLINE);
        drop(client);
        // A new owner must not spend the exhausted network budget again.
        let reopened = conformance::open_real_gated(config);
        assert!(
            reopened
                .flush_submission(&receipt.submission_id, DEADLINE)
                .is_err()
        );
        assert_eq!(
            collector.calls.load(Ordering::Acquire),
            4,
            "{name} transport attempt budget"
        );
        assert_eq!(
            collector.paths.lock().unwrap().as_slice(),
            vec![path.to_owned(); 4].as_slice()
        );
        let _ = reopened.shutdown(DEADLINE);
    }
}

#[test]
fn transient_collector_failure_delivers_within_one_drain_attempt() {
    for name in ["logs", "profiles"] {
        let dir = tempfile::tempdir().unwrap();
        let collector = Collector::new(Some(1));
        let client = conformance::open_real_gated(collector.configure(dir.path()));
        let receipt = client.emit(fixture(name)).unwrap();
        client
            .flush_submission(&receipt.submission_id, DEADLINE)
            .unwrap();
        let status = client
            .status(StatusQuery::Submissions(vec![receipt.submission_id]))
            .unwrap();
        assert!(matches!(
            status.submissions[0].signals[0].1,
            DeliveryState::Delivered { attempts: 1, .. }
        ));
        assert_eq!(collector.calls.load(Ordering::Acquire), 2);
        client.shutdown(DEADLINE).unwrap();
    }
}

struct CancelledExporter(AtomicUsize);
impl SubmissionExporter for CancelledExporter {
    fn export(&self, _: Signal, _: &[SubmissionEnvelope]) -> Result<(), SubmissionExportFailure> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Err(SubmissionExportFailure::Retryable(
            ExportError::ShutdownCancelledRetry {
                context: context(
                    error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
                    "transport shutdown",
                ),
            },
        ))
    }
}

#[test]
fn interrupted_transport_remains_pending_for_replacement_client() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(CancelledExporter(AtomicUsize::new(0)));
    let config = config(dir.path());
    let client = conformance::open_gated_with(config.clone(), exporter.clone());
    let receipt = client.emit(fixture("logs")).unwrap();
    // Frozen lease instant and no background workers: the lease cannot lapse and
    // no other thread can claim the row, so the single drain below is the only
    // claim and its refund is observable exactly.
    let _clock = FrozenClock::new();
    assert!(!drain_once_bounded(
        &client.owner.shared,
        exporter.as_ref(),
        Signal::Logs
    ));
    let status = client
        .status(StatusQuery::Submissions(vec![
            receipt.submission_id.clone(),
        ]))
        .unwrap();
    assert!(matches!(
        status.submissions[0].signals[0].1,
        DeliveryState::Pending
    ));
    let attempts: u32 = client
        .owner
        .shared
        .db
        .lock()
        .unwrap()
        .query_row("SELECT attempts FROM signal_deliveries", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(attempts, 0);
    assert_eq!(exporter.0.load(Ordering::Acquire), 1);
    // Shutdown would re-drive the cancelled exporter; hand the lease over instead.
    worker::release(&client.owner.shared);
    drop(client);
    assert_eq!(exporter.0.load(Ordering::Acquire), 1);
    let replacement = conformance::open_gated(config, &Arc::new(ScriptedExporter::new(dir.path())));
    replacement
        .flush_submission(&receipt.submission_id, DEADLINE)
        .unwrap();
    replacement.shutdown(DEADLINE).unwrap();
}
