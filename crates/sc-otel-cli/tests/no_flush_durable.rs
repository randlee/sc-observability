#[path = "common/assert_result_v1.rs"]
mod assert_result_v1;
use assert_result_v1::assert_result_v1;
use std::{net::TcpListener, process::Command};

#[test]
fn no_flush_preserves_admission_when_the_collector_is_unavailable() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let unavailable_collector = TcpListener::bind("127.0.0.1:0").expect("reserve collector port");
    let endpoint = format!(
        "http://{}",
        unavailable_collector
            .local_addr()
            .expect("reserved collector address")
    );

    let output = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["--store"])
        .arg(directory.path().join("store.sqlite"))
        .args(["--endpoint", &endpoint, "emit", "--no-flush", "--log", "{}"])
        .output()
        .expect("binary runs");

    assert!(output.status.success(), "{output:?}");
    let result = assert_result_v1(&output.stdout, "emit");
    assert_eq!(result["state"], "admitted_pending");
    assert_eq!(result["exit_code"], 0);
    assert!(result["flush"].is_null());
    assert!(result["error"].is_null());
}

#[test]
fn status_is_read_only_for_a_pending_record_without_a_listener() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let unavailable_collector = TcpListener::bind("127.0.0.1:0").expect("reserve collector port");
    let collector_address = unavailable_collector
        .local_addr()
        .expect("reserved collector address");
    let endpoint = format!("http://{collector_address}");
    drop(unavailable_collector);
    let store = directory.path().join("store.sqlite");

    let emit = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
        .args(["--store"])
        .arg(&store)
        .args(["--endpoint", &endpoint, "emit", "--no-flush", "--log", "{}"])
        .output()
        .expect("binary runs");
    assert!(emit.status.success(), "{emit:?}");
    assert_eq!(
        assert_result_v1(&emit.stdout, "emit")["state"],
        "admitted_pending"
    );
    let collector = TcpListener::bind(collector_address).expect("start live collector");

    for attempt in 1..=3 {
        let status = Command::new(env!("CARGO_BIN_EXE_sc-otel"))
            .args(["--store"])
            .arg(&store)
            .args(["--endpoint", &endpoint, "status"])
            .output()
            .expect("binary runs");
        assert!(status.status.success(), "attempt {attempt}: {status:?}");
        let result = assert_result_v1(&status.stdout, "status");
        assert_eq!(result["exit_code"], 0, "attempt {attempt}");
        assert_eq!(
            result["flush"],
            serde_json::Value::Null,
            "attempt {attempt}"
        );
        assert_eq!(result["status"]["pending"]["logs"], 1, "attempt {attempt}");
        assert!(result["error"].is_null(), "attempt {attempt}");
    }
    collector
        .set_nonblocking(true)
        .expect("inspect collector without waiting");
    let mut requests = 0;
    while collector.accept().is_ok() {
        requests += 1;
    }
    assert_eq!(
        requests, 0,
        "status must not contact the live collector while inspecting pending records"
    );
}
