#![cfg(feature = "durable-store")]
use rusqlite::Connection;
#[test]
fn schema_has_version_tables_and_ready_index() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    db.execute_batch(include_str!("../src/durable/schema.sql"))
        .unwrap();
    let version: u32 = db
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    let mut stmt = db
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap();
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        tables,
        [
            "drain_lease",
            "record_keys",
            "signal_deliveries",
            "store_counters",
            "store_meta",
            "submissions"
        ]
    );
    let index: String = db
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='signal_deliveries_ready'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(index.contains("(signal, state, next_attempt_at_unix_nano)"));
    db.execute(
        "INSERT INTO submissions VALUES ('s', 'k', 1, '{}', 2, 0)",
        [],
    )
    .unwrap();
    assert!(
        db.execute(
            "INSERT INTO submissions VALUES ('s2', 'k', 1, '{}', 2, 0)",
            []
        )
        .is_err()
    );
    for signal in ["logs", "traces", "metrics", "profiles"] {
        db.execute(
            "INSERT INTO signal_deliveries(submission_id,signal,state) VALUES ('s',?1,'pending')",
            [signal],
        )
        .unwrap();
    }
    assert!(db.execute("INSERT INTO signal_deliveries(submission_id,signal,state) VALUES ('s','stats','pending')", []).is_err());
    assert!(
        db.execute("UPDATE signal_deliveries SET state='unknown'", [])
            .is_err()
    );
    for state in ["claimed", "retry", "delivered", "failed", "evicted"] {
        db.execute("UPDATE signal_deliveries SET state=?1", [state])
            .unwrap();
    }
    assert!(
        db.execute("INSERT INTO drain_lease VALUES (2,'owner',0,1)", [])
            .is_err()
    );
    db.execute("DELETE FROM submissions WHERE submission_id='s'", [])
        .unwrap();
    let rows: u32 = db
        .query_row("SELECT count(*) FROM signal_deliveries", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 0);
}
#[test]
fn durable_open_persists_records_and_loader_retains_typed_errors() {
    use sc_observability_otlp::durable::{DurableTelemetryClient, load_telemetry_file};
    use sc_observability_types::otlp::submission::*;
    let mut overrides = ConfigOverrides::default();
    let dir = tempfile::tempdir().unwrap();
    overrides.store_path = Some(dir.path().join("telemetry.db"));
    let config = resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap();
    let client = DurableTelemetryClient::open(config.clone()).unwrap();
    let mut input = SubmissionInput::new();
    input.logs.push(LogInput::new());
    let receipt = client
        .emit(SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap())
        .unwrap();
    let _ = client.shutdown(std::time::Duration::from_millis(20));
    drop(client);
    let reopened = DurableTelemetryClient::open(config).unwrap();
    let status = reopened
        .status(StatusQuery::Submissions(vec![
            receipt.submission_id.clone(),
        ]))
        .unwrap();
    assert_eq!(status.submissions.len(), 1);
    assert_eq!(status.submissions[0].submission_id, receipt.submission_id);
    assert!(matches!(
        load_telemetry_file(std::path::Path::new("absent.yaml")),
        Err(TelemetryConfigError::ConfigFile { .. })
    ));
}
