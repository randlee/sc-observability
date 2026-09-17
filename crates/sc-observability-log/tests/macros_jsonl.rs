//! Event macros → JSONL: one assertion per grammar-table row, runtime label and
//! key failures, field value dispatch and disabled-level laziness.
//!
//! One `init` per test binary: every sub-case runs inside the single test fn, so
//! the `DropCause::InvalidEvent` deltas asserted here cannot race.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sc_observability_log::{
    ActionName, BridgeOptions, DropCause, Level, LevelFilter, LogGuard, LoggerConfig, ServiceName,
    debug, error, event, info, trace, warn,
};
use serde::ser::{Error as _, SerializeStruct as _};
use serde_json::{Value, json};

const DEFAULT_ACTION: &str = "macros.default";
const MODULE_TARGET: &str = "macros_jsonl";

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn by_message<'a>(events: &'a [Value], message: &str) -> &'a Value {
    let mut matches = events.iter().filter(|event| event["message"] == message);
    let event = matches
        .next()
        .unwrap_or_else(|| panic!("missing event {message:?}"));
    assert!(matches.next().is_none(), "duplicate event {message:?}");
    event
}

fn flushed_events(guard: &LogGuard, path: &Path) -> Vec<Value> {
    guard.flush(Duration::from_secs(5)).unwrap();
    read_events(path)
}

fn assert_event(
    events: &[Value],
    message: &str,
    (level, target, action): (&str, &str, &str),
    fields: &Value,
) {
    let event = by_message(events, message);
    assert_eq!(event["level"], level, "level of {message:?}");
    assert_eq!(event["target"], target, "target of {message:?}");
    assert_eq!(event["action"], action, "action of {message:?}");
    assert_eq!(&event["fields"], fields, "fields of {message:?}");
}

// ---- value types ----

/// Implements both `Serialize` and `Debug`: records JSON.
#[derive(Debug)]
struct Both {
    a: i64,
}

impl serde::Serialize for Both {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Both", 1)?;
        state.serialize_field("a", &self.a)?;
        state.end()
    }
}

/// Implements only `Debug`: records the Debug string.
#[derive(Debug)]
#[allow(dead_code, reason = "read through Debug only")]
struct DebugOnly {
    b: i64,
}

/// A `Serialize` impl that fails.
#[derive(Debug)]
struct FailingSerialize;

impl serde::Serialize for FailingSerialize {
    fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(S::Error::custom("failing serialize"))
    }
}

/// Every impl panics: evaluating, formatting or serializing it fails the test.
struct Panicky;

impl fmt::Debug for Panicky {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("Panicky Debug was evaluated for a disabled event")
    }
}

impl fmt::Display for Panicky {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("Panicky Display was evaluated for a disabled event")
    }
}

impl serde::Serialize for Panicky {
    fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        panic!("Panicky Serialize was evaluated for a disabled event")
    }
}

fn panicky_value() -> Panicky {
    panic!("a disabled event evaluated its field expression")
}

struct Point {
    x: i64,
}

// ---- sub-cases ----

const TGT: &str = "macros.const_target";
const NAME: &str = "macros.const_name";
const KEY: &str = "macros.dynamic";
const LVL: Level = Level::WARN;

#[expect(
    clippy::too_many_lines,
    reason = "one call and one assertion per grammar-table row"
)]
fn grammar_rows(path: &Path, guard: &LogGuard) {
    let count: u32 = 5;
    let ok = true;
    let label = "beads";
    let both = Both { a: 1 };
    let point = Point { x: 3 };
    let err = std::io::Error::other("disk full");
    let m = MODULE_TARGET;
    let d = DEFAULT_ACTION;

    info!("row plain {} {}", count, label);
    info!(target: "macros.literal_target", "row target literal");
    info!(target: TGT, "row target const");
    info!(target: module_path!(), "row target module_path");
    info!(target: concat!("macros", ".concat_target"), "row target concat");
    info!(name: "bad name", target: "bad target::x", "row unsanitized labels");
    info!(name: "macros.literal_name", "row name literal");
    info!(name: NAME, "row name const");
    info!(name: concat!("macros", ".concat_name"), "row name concat");
    info!(name: "macros.both_name", target: "macros.both_target", "row name and target");
    info!(n = count, b = ok, s = label, o = both, "row key value");
    info!(a.b = count, "row dotted key");
    info!("literal key" = ok, "row literal key");
    info!(r#type = label, "row raw key");
    info!({ KEY } = count, "row dynamic key");
    info!({ KEY } = ?label, "row dynamic key debug");
    info!({ KEY } = %err, "row dynamic key display");
    info!(?label, "row debug shorthand");
    info!(k = ?both, "row debug value");
    info!(%err, "row display shorthand");
    info!(k = %err, "row display value");
    info!(count, "row shorthand");
    info!(point.x, "row dotted shorthand");
    info!({ k = 1, ?label }, "row brace {}", count);
    info!(name: "macros.brace_name", { k = true }, "row brace after name");
    info!(target: "macros.brace_target", { k = "v" }, "row brace after target");
    info!(name: "macros.brace_both_name", target: "macros.brace_both_target", { o = both }, "row brace after both");
    event!(Level::INFO, { k = 1 }, "row event brace");
    info!(k = count, ok, "row fields then message {}", label);
    event!(Level::INFO, "row event level");
    event!(target: "macros.event_target", Level::WARN, "row event target");
    event!(name: "macros.event_name", Level::INFO, "row event name");
    event!(name: "macros.event_both_name", target: "macros.event_both_target", Level::ERROR, "row event both");
    event!(LVL, "row event const level");
    debug!(k = 1, "row debug macro");
    warn!(k = 1, "row warn macro");
    error!(k = 1, "row error macro");
    info!({ brace_only = 1 });

    let events = flushed_events(guard, path);
    let info = |target, action| ("Info", target, action);
    assert_event(&events, "row plain 5 beads", info(m, d), &json!({}));
    assert_event(
        &events,
        "row target literal",
        info("macros.literal_target", d),
        &json!({}),
    );
    assert_event(
        &events,
        "row target const",
        info("macros.const_target", d),
        &json!({}),
    );
    assert_event(&events, "row target module_path", info(m, d), &json!({}));
    assert_event(
        &events,
        "row target concat",
        info("macros.concat_target", d),
        &json!({}),
    );
    assert_event(
        &events,
        "row unsanitized labels",
        info("bad_target.x", "bad_name"),
        &json!({}),
    );
    assert_event(
        &events,
        "row name literal",
        info(m, "macros.literal_name"),
        &json!({}),
    );
    assert_event(
        &events,
        "row name const",
        info(m, "macros.const_name"),
        &json!({}),
    );
    assert_event(
        &events,
        "row name concat",
        info(m, "macros.concat_name"),
        &json!({}),
    );
    assert_event(
        &events,
        "row name and target",
        info("macros.both_target", "macros.both_name"),
        &json!({}),
    );
    assert_event(
        &events,
        "row key value",
        info(m, d),
        &json!({"n": 5, "b": true, "s": "beads", "o": {"a": 1}}),
    );
    assert_event(&events, "row dotted key", info(m, d), &json!({"a.b": 5}));
    assert_event(
        &events,
        "row literal key",
        info(m, d),
        &json!({"literal_key": true}),
    );
    assert_event(
        &events,
        "row raw key",
        info(m, d),
        &json!({"type": "beads"}),
    );
    assert_event(
        &events,
        "row dynamic key",
        info(m, d),
        &json!({"macros.dynamic": 5}),
    );
    assert_event(
        &events,
        "row dynamic key debug",
        info(m, d),
        &json!({"macros.dynamic": "\"beads\""}),
    );
    assert_event(
        &events,
        "row dynamic key display",
        info(m, d),
        &json!({"macros.dynamic": "disk full"}),
    );
    assert_event(
        &events,
        "row debug shorthand",
        info(m, d),
        &json!({"label": "\"beads\""}),
    );
    assert_event(
        &events,
        "row debug value",
        info(m, d),
        &json!({"k": "Both { a: 1 }"}),
    );
    assert_event(
        &events,
        "row display shorthand",
        info(m, d),
        &json!({"err": "disk full"}),
    );
    assert_event(
        &events,
        "row display value",
        info(m, d),
        &json!({"k": "disk full"}),
    );
    assert_event(&events, "row shorthand", info(m, d), &json!({"count": 5}));
    assert_event(
        &events,
        "row dotted shorthand",
        info(m, d),
        &json!({"point.x": 3}),
    );
    assert_event(
        &events,
        "row brace 5",
        info(m, d),
        &json!({"k": 1, "label": "\"beads\""}),
    );
    assert_event(
        &events,
        "row brace after name",
        info(m, "macros.brace_name"),
        &json!({"k": true}),
    );
    assert_event(
        &events,
        "row brace after target",
        info("macros.brace_target", d),
        &json!({"k": "v"}),
    );
    assert_event(
        &events,
        "row brace after both",
        info("macros.brace_both_target", "macros.brace_both_name"),
        &json!({"o": {"a": 1}}),
    );
    assert_event(&events, "row event brace", info(m, d), &json!({"k": 1}));
    assert_event(
        &events,
        "row fields then message beads",
        info(m, d),
        &json!({"k": 5, "ok": true}),
    );
    assert_event(&events, "row event level", info(m, d), &json!({}));
    assert_event(
        &events,
        "row event target",
        ("Warn", "macros.event_target", d),
        &json!({}),
    );
    assert_event(
        &events,
        "row event name",
        info(m, "macros.event_name"),
        &json!({}),
    );
    assert_event(
        &events,
        "row event both",
        (
            "Error",
            "macros.event_both_target",
            "macros.event_both_name",
        ),
        &json!({}),
    );
    assert_event(&events, "row event const level", ("Warn", m, d), &json!({}));
    assert_event(
        &events,
        "row debug macro",
        ("Debug", m, d),
        &json!({"k": 1}),
    );
    assert_event(&events, "row warn macro", ("Warn", m, d), &json!({"k": 1}));
    assert_event(
        &events,
        "row error macro",
        ("Error", m, d),
        &json!({"k": 1}),
    );
    let brace_only = events
        .iter()
        .find(|event| event["fields"].get("brace_only").is_some())
        .expect("brace-only event");
    assert!(brace_only.get("message").is_none_or(Value::is_null));
    assert_eq!(brace_only["fields"], json!({"brace_only": 1}));
}

fn runtime_labels_and_keys(path: &Path, guard: &LogGuard) {
    const EMPTY: &str = "";
    const RESERVED: &str = "sc_observability_log.x";
    const RESERVED_PATH: &str = "sc_observability_log::y";
    const SPACED: &str = "a b";

    let invalid = || guard.dropped_events().get(DropCause::InvalidEvent);

    let before = invalid();
    info!(
        { EMPTY } = 1,
        { RESERVED } = 2,
        { RESERVED_PATH } = 3,
        { SPACED } = 4,
        ok = 5,
        "labels dynamic keys"
    );
    assert_eq!(invalid(), before + 3, "one count per omitted key");

    let before = invalid();
    info!(name: "", "labels empty name");
    assert_eq!(invalid(), before + 1, "one count per invalid name");

    let before = invalid();
    info!(name: "bad name", target: "bad target::x", "labels sanitized");
    assert_eq!(invalid(), before, "sanitized labels are not counted");

    let events = flushed_events(guard, path);
    assert_event(
        &events,
        "labels dynamic keys",
        ("Info", MODULE_TARGET, DEFAULT_ACTION),
        &json!({"a_b": 4, "ok": 5}),
    );
    assert_event(
        &events,
        "labels empty name",
        ("Info", MODULE_TARGET, DEFAULT_ACTION),
        &json!({}),
    );
    assert_event(
        &events,
        "labels sanitized",
        ("Info", "bad_target.x", "bad_name"),
        &json!({}),
    );
}

fn field_dispatch(path: &Path, guard: &LogGuard) {
    let both = Both { a: 3 };
    let debug_only = DebugOnly { b: 4 };
    let hm: HashMap<(i32, i32), i32> = HashMap::from([((1, 2), 3)]);
    let failing = FailingSerialize;
    let nan = f64::NAN;

    info!(both, debug_only, "dispatch shorthand");
    info!(hm, failing, nan, n = 5_u32, "dispatch failures");

    let events = flushed_events(guard, path);
    assert_event(
        &events,
        "dispatch shorthand",
        ("Info", MODULE_TARGET, DEFAULT_ACTION),
        &json!({"both": {"a": 3}, "debug_only": "DebugOnly { b: 4 }"}),
    );
    assert_event(
        &events,
        "dispatch failures",
        ("Info", MODULE_TARGET, DEFAULT_ACTION),
        &json!({
            "hm": null,
            "failing": null,
            "nan": null,
            "n": 5,
            "sc_observability_log.serialize_errors": {
                "hm": "key must be a string",
                "failing": "failing serialize",
            },
        }),
    );
}

fn disabled_level_is_lazy(path: &Path, guard: &LogGuard) {
    let panicky = Panicky;
    trace!(panicky, "disabled {:?} {}", Panicky, Panicky);
    trace!(k = panicky_value(), d = ?Panicky, s = %Panicky, { KEY } = Panicky, "disabled fields");
    event!(
        Level::TRACE,
        { k = panicky_value() },
        "disabled {}",
        Panicky
    );
    let events = flushed_events(guard, path);
    assert!(events.iter().all(|event| event["level"] != "Trace"));
}

#[test]
fn event_macros_write_structured_jsonl() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("macros-jsonl").unwrap(),
        root.path().into(),
    );
    config.level = LevelFilter::Debug;
    let options = BridgeOptions {
        default_action: ActionName::new(DEFAULT_ACTION).unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let path: PathBuf = guard.active_log_path().unwrap().to_path_buf();

    grammar_rows(&path, &guard);
    runtime_labels_and_keys(&path, &guard);
    field_dispatch(&path, &guard);
    disabled_level_is_lazy(&path, &guard);

    guard.shutdown(Duration::from_secs(5)).unwrap();
}
