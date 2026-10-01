use rusqlite::Connection;
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
