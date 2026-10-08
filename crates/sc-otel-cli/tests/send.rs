//! Runs the built `sc-otel` binary, as a fresh process without a Tokio
//! runtime, against a loopback collector that captures each OTLP request.

use std::{
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Watchdog for a fixture that never receives its request; not a pass condition.
const FIXTURE_WATCHDOG: Duration = Duration::from_secs(20);
const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const SPAN_ID: &str = "00f067aa0ba902b7";
const SECRET: &str = "s3cret-token-value";

struct Captured {
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Captured {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn body_contains(&self, needle: &[u8]) -> bool {
        self.body
            .windows(needle.len())
            .any(|window| window == needle)
    }
}

/// Accepts exactly one request, answers with `status` and returns it.
struct Collector {
    listener: TcpListener,
    endpoint: String,
}

impl Collector {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind collector");
        listener.set_nonblocking(true).expect("nonblocking accept");
        let endpoint = format!("http://{}", listener.local_addr().expect("address"));
        Self { listener, endpoint }
    }

    /// Serves one request on a thread so the CLI can run on the test thread.
    fn serve(self, status: &'static str) -> thread::JoinHandle<Captured> {
        thread::spawn(move || {
            let deadline = Instant::now() + FIXTURE_WATCHDOG;
            let stream = loop {
                match self.listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "no request arrived");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            };
            handle(stream, status)
        })
    }

    /// After the CLI has exited, proves it never connected.
    fn assert_untouched(&self) {
        match self.listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            other => panic!("the collector received a connection: {other:?}"),
        }
    }
}

fn handle(mut stream: TcpStream, status: &str) -> Captured {
    stream.set_nonblocking(false).expect("blocking stream");
    stream
        .set_read_timeout(Some(FIXTURE_WATCHDOG))
        .expect("read watchdog");
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("request line");
    let path = line.split_whitespace().nth(1).expect("path").to_owned();
    let mut headers = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("header line");
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').expect("header");
        headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .map_or(0, |(_, value)| value.parse().expect("content length"));
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("body");
    write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: application/x-protobuf\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    )
    .expect("response");
    Captured {
        path,
        headers,
        body,
    }
}

/// The binary with every ambient `OTEL_*` setting removed and an empty
/// working directory, so a test sees only the configuration it sets.
fn sc_otel(directory: &tempfile::TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
    command.current_dir(directory.path());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("OTEL_") {
            command.env_remove(key);
        }
    }
    command
}

fn run(command: &mut Command) -> Output {
    command.output().expect("run sc-otel")
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("hex"))
        .collect()
}

fn assert_no_files(directory: &tempfile::TempDir) {
    let entries = std::fs::read_dir(directory.path())
        .expect("read working directory")
        .count();
    assert_eq!(entries, 0, "sc-otel created files in its working directory");
}

#[test]
fn log_exports_native_record_with_cli_and_environment_configuration() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let request = collector.serve("200 OK");
    let output = run(sc_otel(&directory)
        .env(
            "OTEL_EXPORTER_OTLP_HEADERS",
            "x-tenant=from-env,x-env-only=yes",
        )
        .args(["--endpoint", &endpoint, "--service", "cli-service"])
        .args(["--header", "x-tenant=from-cli", "--timeout", "5"])
        .args(["log", "--body", "hello-from-cli", "--severity", "warn"])
        .args(["--trace-id", TRACE_ID, "--span-id", SPAN_ID])
        .args(["--attributes", r#"{"job":"build-42","attempt":2}"#]));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(request.path, "/v1/logs");
    assert_eq!(
        request.header("content-type"),
        Some("application/x-protobuf")
    );
    assert_eq!(request.header("x-tenant"), Some("from-cli"));
    assert_eq!(request.header("x-env-only"), Some("yes"));
    for needle in [
        &b"hello-from-cli"[..],
        b"WARN",
        b"cli-service",
        b"build-42",
        b"attempt",
        b"sc-otel",
        &hex(TRACE_ID),
        &hex(SPAN_ID),
    ] {
        assert!(
            request.body_contains(needle),
            "{:?} missing from the log request",
            String::from_utf8_lossy(needle)
        );
    }
    assert_no_files(&directory);
}

#[test]
fn span_exports_completed_native_span() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let request = collector.serve("200 OK");
    let mut child = sc_otel(&directory)
        .args(["--endpoint", &endpoint, "span", "--name", "deploy-step"])
        .args(["--trace-id", TRACE_ID, "--parent-span-id", SPAN_ID])
        .args(["--kind", "client", "--error", "rollout-timed-out"])
        .args(["--start-time-unix-nano", "1700000000000000000"])
        .args(["--end-time-unix-nano", "1700000005000000000"])
        .args(["--attributes", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"region":"eu-west"}"#)
        .expect("write attributes");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(request.path, "/v1/traces");
    for needle in [
        &b"deploy-step"[..],
        b"rollout-timed-out",
        b"region",
        b"eu-west",
        &hex(TRACE_ID),
        &hex(SPAN_ID),
        &1_700_000_000_000_000_000_u64.to_le_bytes(),
        &1_700_000_005_000_000_000_u64.to_le_bytes(),
    ] {
        assert!(
            request.body_contains(needle),
            "{:?} missing from the span request",
            String::from_utf8_lossy(needle)
        );
    }
    assert_no_files(&directory);
}

#[test]
fn metric_exports_one_measurement_to_the_environment_endpoint() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let request = collector.serve("200 OK");
    let output = run(sc_otel(&directory)
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args(["metric", "--name", "jobs.completed", "--kind", "counter"])
        .args(["--value", "3", "--unit", "{job}"])
        .args(["--attributes", r#"{"queue":"default"}"#]));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(request.path, "/v1/metrics");
    for needle in [&b"jobs.completed"[..], b"{job}", b"queue", b"default"] {
        assert!(
            request.body_contains(needle),
            "{:?} missing from the metric request",
            String::from_utf8_lossy(needle)
        );
    }
    assert_no_files(&directory);
}

#[test]
fn rejected_export_exits_7_without_printing_credentials() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector
        .endpoint
        .replace("http://", "http://user:s3cret-pass@");
    let request = collector.serve("400 Bad Request");
    let output = run(sc_otel(&directory)
        .args(["--endpoint", &endpoint, "--timeout", "5"])
        .args(["--header", &format!("authorization=Bearer {SECRET}")])
        .args(["log", "--body", "rejected"]));
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("export failed"), "{stderr}");
    assert!(!stderr.contains(SECRET), "{stderr}");
    assert!(!stderr.contains("s3cret-pass"), "{stderr}");
    assert!(output.stdout.is_empty());
}

#[test]
fn invalid_input_exits_3_and_sends_nothing() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let cases: [(&[&str], &str); 4] = [
        (
            &[
                "log",
                "--body",
                "x",
                "--attributes",
                r#"{"nested":{"a":1}}"#,
            ],
            "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD",
        ),
        (
            &[
                "span",
                "--name",
                "x",
                "--start-time-unix-nano",
                "2",
                "--end-time-unix-nano",
                "1",
            ],
            "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD",
        ),
        (
            &[
                "metric",
                "--name",
                "1-not-an-instrument-name",
                "--kind",
                "gauge",
                "--value",
                "1",
            ],
            "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD",
        ),
        (
            &["--root-certificate", "missing.pem", "log", "--body", "x"],
            "SC_OBSERVABILITY_OTLP_SYNC_INVALID_CONFIG",
        ),
    ];
    for (args, code) in cases {
        let output = run(sc_otel(&directory)
            .args(["--endpoint", &endpoint])
            .args(args));
        assert_eq!(output.status.code(), Some(3), "{args:?}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(code), "{args:?}: {stderr}");
    }
    collector.assert_untouched();
    assert_no_files(&directory);
}

#[test]
fn oversized_standard_input_is_rejected_before_export() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let mut child = sc_otel(&directory)
        .args(["--endpoint", &collector.endpoint])
        .args(["log", "--body", "x", "--attributes", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    let writer = thread::spawn(move || {
        let chunk = vec![b' '; 64 * 1024];
        // The CLI stops reading at the limit and exits, closing the pipe.
        for _ in 0..64 {
            if stdin.write_all(&chunk).is_err() {
                break;
            }
        }
    });
    let output = child.wait_with_output().expect("wait");
    writer.join().expect("writer");
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("SC_OBSERVABILITY_OTLP_SYNC_INPUT_LIMIT_EXCEEDED"),
        "{stderr}"
    );
    collector.assert_untouched();
}

#[test]
fn usage_errors_exit_2_and_help_exits_0() {
    let directory = tempfile::tempdir().expect("tempdir");
    for args in [
        &["--store", "queue.sqlite", "log", "--body", "x"][..],
        &["emit", "--log", "{}"],
        &[
            "log",
            "--body",
            "x",
            "--trace-id",
            "xyz",
            "--span-id",
            SPAN_ID,
        ],
    ] {
        let output = run(sc_otel(&directory).args(args));
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    }
    let output = run(sc_otel(&directory).arg("--help"));
    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8_lossy(&output.stdout);
    for command in ["log", "span", "metric"] {
        assert!(help.contains(command), "{help}");
    }
    assert_no_files(&directory);
}
