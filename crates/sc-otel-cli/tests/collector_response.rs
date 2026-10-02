//! Real HTTP acknowledgements must drive durable state, not merely HTTP status.
mod common;

use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

struct Collector {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<Vec<String>>>,
}

impl Collector {
    fn start(body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind collector");
        let address = listener.local_addr().expect("collector address");
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut paths = Vec::new();
            loop {
                let (mut stream, _) = listener.accept().expect("accept request");
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .expect("read watchdog");
                stream
                    .set_write_timeout(Some(Duration::from_secs(10)))
                    .expect("write watchdog");
                let mut reader = BufReader::new(&mut stream);
                let mut request = String::new();
                reader.read_line(&mut request).expect("request line");
                paths.push(
                    request
                        .split_whitespace()
                        .nth(1)
                        .expect("request path")
                        .to_owned(),
                );
                let mut length = None;
                loop {
                    let mut line = String::new();
                    assert!(reader.read_line(&mut line).expect("header") > 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = Some(value.trim().parse::<usize>().expect("length"));
                    }
                }
                let mut payload = vec![0; length.expect("content length")];
                reader
                    .read_exact(&mut payload)
                    .expect("complete request body");
                let _: Value = serde_json::from_slice(&payload).expect("OTLP JSON request");
                // Write headers separately: oversized-body tests may close early.
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).expect("response headers");
                let _ = stream.write_all(body.as_bytes());
            }
            paths
        });
        Self {
            address,
            stop,
            thread: Some(thread),
        }
    }
    fn finish(&mut self) -> Vec<String> {
        self.stop.store(true, Ordering::Release);
        TcpStream::connect(self.address).expect("wake collector");
        self.thread
            .take()
            .expect("collector thread")
            .join()
            .expect("collector completes")
    }
}
impl Drop for Collector {
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.finish();
        }
    }
}

fn emit(body: String, flag: &str, payload: &str, signal: &str, path: &str, rejected: bool) {
    let mut collector = Collector::start(body);
    let store = tempfile::tempdir().expect("fresh durable store");
    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .current_dir(store.path())
        .env_remove("SC_OTEL_TEST_DOUBLE")
        .env_remove("SC_OTEL_AUTH_HEADER")
        .args([
            "--endpoint",
            &format!("http://{}", collector.address),
            "--store",
        ])
        .arg(store.path().join("telemetry.sqlite"))
        .args(["emit", flag, payload])
        .output()
        .expect("real CLI runs");
    let result = common::assert_result_v1(&output.stdout, "emit");
    assert_eq!(
        output.status.code(),
        Some(if rejected { 7 } else { 0 }),
        "{result} {output:?}"
    );
    assert_eq!(
        result["state"],
        if rejected {
            "admitted_failed"
        } else {
            "admitted_delivered"
        }
    );
    assert_eq!(result["flush"]["delivered"][signal], u64::from(!rejected));
    assert_eq!(result["flush"]["failed"][signal], u64::from(rejected));
    assert_eq!(result["flush"]["still_pending"][signal], 0);
    // Reopen the store in another process: prove terminal state is persisted and
    // is not re-exported by a fresh drain owner.
    let status = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .current_dir(store.path())
        .env_remove("SC_OTEL_TEST_DOUBLE")
        .args([
            "--endpoint",
            &format!("http://{}", collector.address),
            "--store",
        ])
        .arg(store.path().join("telemetry.sqlite"))
        .args([
            "status",
            "--submission",
            result["receipt"]["submission_id"].as_str().expect("id"),
        ])
        .output()
        .expect("status runs");
    assert!(status.status.success(), "{status:?}");
    let status = common::assert_result_v1(&status.stdout, "status");
    let persisted = &status["status"]["submissions"][0]["signals"][0];
    assert_eq!(persisted[0], signal);
    assert_eq!(
        persisted[1]["state"],
        if rejected { "failed" } else { "delivered" }
    );
    assert_eq!(persisted[1]["attempts"], 1);
    if rejected {
        assert_eq!(persisted[1]["error"], "OTLP_EXPORT_TERMINAL");
    }
    assert_eq!(
        collector.finish(),
        vec![path],
        "never replay partially accepted data"
    );
}

#[test]
fn every_signal_honors_collector_partial_success_through_durable_cli() {
    for (fixture, field, flag, signal, count, path) in [
        (
            "logs",
            "logs",
            "--log",
            "logs",
            "rejectedLogRecords",
            "/v1/logs",
        ),
        (
            "traces",
            "spans",
            "--span",
            "traces",
            "rejectedSpans",
            "/v1/traces",
        ),
        (
            "metric_gauge",
            "metrics",
            "--metric",
            "metrics",
            "rejectedDataPoints",
            "/v1/metrics",
        ),
        (
            "profiles",
            "profiles",
            "--profile",
            "profiles",
            "rejectedProfiles",
            "/v1development/profiles",
        ),
    ] {
        let payload = common::fixture_component(fixture, field);
        for (body, rejected) in [
            (
                json!({"partialSuccess": {count: "1", "errorMessage": "rejected"}}).to_string(),
                true,
            ),
            (
                json!({"partialSuccess": {count: "0", "errorMessage": "warning only"}}).to_string(),
                false,
            ),
            ("{}".to_owned(), false),
        ] {
            emit(body, flag, &payload, signal, path, rejected);
        }
    }
}

#[test]
fn unreadable_or_oversized_acknowledgements_never_become_delivered_or_retried() {
    for body in [
        "not JSON".to_owned(),
        json!({"message": "x".repeat(64 * 1024)}).to_string(),
    ] {
        emit(body, "--log", "{}", "logs", "/v1/logs", true);
    }
}
