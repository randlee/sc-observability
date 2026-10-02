mod common;

use common::assert_result_v1;
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
