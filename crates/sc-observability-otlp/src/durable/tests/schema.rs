use super::super::{query, store};
use super::{config, log};
use rusqlite::Connection;
use std::process::Command;

const V1_SCHEMA_GIT_BASELINE: &str = "37a7e7f57c02f0d918e7f455440cd23f931f2643";
const V1_SCHEMA_PATH: &str = "crates/sc-observability-otlp/src/durable/schema.sql";

fn selected_v1_schema_from_git() -> String {
    let revision = format!("{V1_SCHEMA_GIT_BASELINE}:{V1_SCHEMA_PATH}");
    let output = Command::new("git")
        .args(["show", &revision])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("durable schema contract test must be able to run git show");
    assert!(
        output.status.success(),
        "durable SQLite schema contract could not load selected version 1 from {revision}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("durable schema SQL must be UTF-8")
}

fn v1_schema_contract_error(expected: &str, actual: &str) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }
    let changed_line = expected
        .lines()
        .zip(actual.lines())
        .position(|(expected, actual)| expected != actual)
        .map_or_else(
            || expected.lines().count().min(actual.lines().count()) + 1,
            |index| index + 1,
        );
    Err(format!(
        "durable SQLite schema contract mismatch: selected version 1; changed fields/lines begin at line {changed_line}. Create a new schema version and regenerate its snapshot rather than editing v1 in place."
    ))
}

#[test]
fn v1_schema_matches_selected_git_baseline() {
    v1_schema_contract_error(
        &selected_v1_schema_from_git(),
        include_str!("../schema.sql"),
    )
    .expect("current durable schema must retain the selected immutable version 1 contract");
}

#[test]
fn v1_schema_contract_detects_an_in_place_change() {
    let v1 = include_str!("../schema.sql");
    let modified = v1.replacen("CREATE TABLE store_meta", "CREATE TABLE store_metadata", 1);
    let error = v1_schema_contract_error(v1, &modified)
        .expect_err("same-version durable schema edits must be rejected");
    assert!(error.contains("durable SQLite schema contract"));
    assert!(error.contains("selected version 1"));
    assert!(error.contains("changed fields/lines"));
    assert!(error.contains("new schema version"));
}
#[test]
fn schema_has_version_tables_and_ready_index() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    db.execute_batch(include_str!("../schema.sql")).unwrap();
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
fn retained_v1_database_reopens_with_pending_telemetry_usable() {
    let directory = tempfile::tempdir().unwrap();
    let config = config(directory.path());
    let pending = log("retained-v1");
    let retained_v1 = selected_v1_schema_from_git();
    let receipt = {
        let mut v1 = Connection::open(&config.store_path).unwrap();
        v1.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        v1.execute_batch(&retained_v1).unwrap();
        store::admit(&mut v1, &config, &pending).unwrap()
    };

    let mut reopened = store::open(&config.store_path).unwrap();
    let version: u32 = reopened
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, crate::constants::STORE_SCHEMA_VERSION);
    assert_eq!(
        query::snapshot(&reopened, Some(&receipt.submission_id))
            .unwrap()
            .len(),
        1
    );

    let later = store::admit(&mut reopened, &config, &log("retained-v1-later")).unwrap();
    assert_eq!(
        query::snapshot(&reopened, Some(&later.submission_id))
            .unwrap()
            .len(),
        1
    );
}
