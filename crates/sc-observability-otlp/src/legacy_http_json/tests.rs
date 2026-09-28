use super::implementation::{
    OtlpHttpExporter, build_logs_payload, normalize_logs_endpoint, parse_retry_after,
};
use crate::contracts::LogExporter;
use sc_observability_types::{
    ActionName, Level, LogEvent, ProcessIdentity, SchemaVersion, ServiceName, TargetCategory,
    Timestamp,
};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

fn sample_log() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: ServiceName::new("legacy-test").expect("service"),
        target: TargetCategory::new("legacy.http").expect("target"),
        action: ActionName::new("export").expect("action"),
        message: Some("hello".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer).expect("read request");
    request.extend_from_slice(&buffer[..read]);
    String::from_utf8_lossy(&request).into_owned()
}

#[test]
fn endpoint_normalization_preserves_existing_suffix() {
    assert_eq!(
        normalize_logs_endpoint("https://collector.example/"),
        "https://collector.example/v1/logs"
    );
    assert_eq!(
        normalize_logs_endpoint("https://collector.example/v1/logs"),
        "https://collector.example/v1/logs"
    );
}

#[test]
fn retry_after_accepts_delta_seconds_and_bounded_dates() {
    assert_eq!(
        parse_retry_after("3", SystemTime::UNIX_EPOCH),
        Some(Duration::from_secs(3))
    );
    assert!(parse_retry_after(&"x".repeat(129), SystemTime::now()).is_none());
}

#[test]
fn log_payload_preserves_timestamp_and_severity() {
    let record = super::implementation::log_record(&sample_log());
    let payload = build_logs_payload(&[record]);
    let log = &payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
    assert_eq!(log["timeUnixNano"], Value::String("0".to_owned()));
    assert_eq!(log["severityNumber"], 9);
    assert_eq!(log["body"]["stringValue"], "hello");
}

#[test]
fn loopback_export_posts_json_to_logs_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_server = Arc::clone(&seen);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request(&mut stream);
        assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
        assert!(request.contains("\"hello\""));
        seen_server.fetch_add(1, Ordering::Relaxed);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write response");
    });
    let exporter =
        OtlpHttpExporter::for_endpoint(format!("http://{address}")).expect("construct exporter");
    exporter
        .export_logs(&[sample_log()])
        .expect("export succeeds");
    server.join().expect("join server");
    assert_eq!(seen.load(Ordering::Relaxed), 1);
}

#[test]
fn terminal_client_status_is_not_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_server = Arc::clone(&calls);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let _ = read_request(&mut stream);
        calls_server.fetch_add(1, Ordering::Relaxed);
        stream
            .write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write response");
    });
    let exporter =
        OtlpHttpExporter::for_endpoint(format!("http://{address}")).expect("construct exporter");
    let error = exporter
        .export_logs(&[sample_log()])
        .expect_err("400 is terminal");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::NonRetryableHttpStatus { .. }
    ));
    server.join().expect("join server");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
