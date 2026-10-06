//! Real HTTP acknowledgements must drive durable state, not merely HTTP status.
#[path = "common/assert_result_v1.rs"]
mod assert_result_v1;
#[path = "common/fixture_component.rs"]
mod fixture_component;
#[path = "common/golden_root.rs"]
mod golden_root;
use assert_result_v1::assert_result_v1;
use fixture_component::fixture_component;

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
    time::{Duration, Instant},
};

struct Collector {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<Vec<String>>>,
}

impl Collector {
    fn start(body: String) -> Self {
        Self::start_with_request_deadline(body, Duration::from_secs(20))
    }

    fn start_with_request_deadline(body: String, request_deadline: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind collector");
        listener
            .set_nonblocking(true)
            .expect("make collector listener nonblocking");
        let address = listener.local_addr().expect("collector address");
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut paths = Vec::new();
            let deadline = Instant::now() + request_deadline;
            loop {
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "collector timed out waiting for a request"
                );
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => panic!("accept request: {error}"),
                };
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                stream
                    .set_nonblocking(false)
                    .expect("make accepted collector stream blocking");
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .expect("read watchdog");
                stream
                    .set_write_timeout(Some(Duration::from_secs(10)))
                    .expect("write watchdog");
                let mut reader = BufReader::new(&mut stream);
                let mut request = String::new();
                reader.read_line(&mut request).expect("request line");
                let path = request
                    .split_whitespace()
                    .nth(1)
                    .expect("request path")
                    .to_owned();
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
                paths.push(path);
                // Write headers separately: oversized-body tests may close early.
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).expect("response headers");
                let _ = stream.write_all(body.as_bytes());
                break;
            }
            while !stopped.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(10));
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
        self.thread
            .take()
            .expect("collector thread")
            .join()
            .expect("collector thread panicked")
    }
}
impl Drop for Collector {
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.finish();
        }
    }
}

fn scrub_telemetry_environment(command: &mut Command) -> &mut Command {
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("SC_OTEL_") || name.starts_with("OTEL_") {
            command.env_remove(key);
        }
    }
    command
}

#[derive(Clone, Copy)]
struct SignalCase<'a> {
    flag: &'a str,
    payload: &'a str,
    signal: &'a str,
    path: &'a str,
}

fn emit(body: String, case: SignalCase<'_>, rejected: bool) {
    let SignalCase {
        flag,
        payload,
        signal,
        path,
    } = case;
    let mut collector = Collector::start(body);
    let store = tempfile::tempdir().expect("fresh durable store");
    let mut emit = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
    let output = scrub_telemetry_environment(&mut emit)
        .current_dir(store.path())
        .args([
            "--endpoint",
            &format!("http://{}", collector.address),
            "--store",
        ])
        .arg(store.path().join("telemetry.sqlite"))
        .args(["emit", flag, payload])
        .output()
        .expect("real CLI runs");
    let result = assert_result_v1(&output.stdout, "emit");
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
    let mut status_command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
    let status = scrub_telemetry_environment(&mut status_command)
        .current_dir(store.path())
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
    let status = assert_result_v1(&status.stdout, "status");
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
        let payload = fixture_component(fixture, field);
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
            emit(
                body,
                SignalCase {
                    flag,
                    payload: &payload,
                    signal,
                    path,
                },
                rejected,
            );
        }
    }
}

#[test]
fn unreadable_or_oversized_acknowledgements_never_become_delivered_or_retried() {
    for body in [
        "not JSON".to_owned(),
        json!({"message": "x".repeat(64 * 1024)}).to_string(),
    ] {
        emit(
            body,
            SignalCase {
                flag: "--log",
                payload: "{}",
                signal: "logs",
                path: "/v1/logs",
            },
            true,
        );
    }
}

#[test]
fn collector_deadline_stops_after_serving_the_expected_request() {
    let mut collector =
        Collector::start_with_request_deadline("{}".to_owned(), Duration::from_secs(1));
    let mut stream = TcpStream::connect(collector.address).expect("connect collector");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set client response read timeout");
    stream
        .write_all(b"POST /v1/logs HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\n\r\n{}")
        .expect("write request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    assert!(response.starts_with("HTTP/1.1 200 OK"));

    thread::sleep(Duration::from_millis(1100));
    assert_eq!(collector.finish(), vec!["/v1/logs"]);
}
