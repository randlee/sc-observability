//! Compiles the shared fixture `tests/compat/events.rs` against `tracing` 0.1
//! (compile-only) and against `sc_observability_log` (executed and asserted
//! against the JSONL output). One `init` per test binary.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::path::Path;
use std::time::Duration;

use sc_observability_log::{ActionName, BridgeOptions, LevelFilter, LoggerConfig, ServiceName};
use serde_json::{Value, json};

/// The fixture compiled with the genuine tracing macros: proves the syntax is tracing syntax.
mod tracing_syntax {
    use tracing::{Level, debug, error, event, info, trace, warn};

    include!("compat/events.rs");
}

/// The same fixture compiled with the sc-observability-log macros.
mod sc_syntax {
    use sc_observability_log::{Level, debug, error, event, info, trace, warn};

    include!("compat/events.rs");
}

const DEFAULT_ACTION: &str = "compat.default";
const DEFAULT_TARGET: &str = "compat_events.sc_syntax";

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

fn by_field<'a>(events: &'a [Value], key: &str) -> &'a Value {
    events
        .iter()
        .find(|event| event["fields"].get(key).is_some())
        .unwrap_or_else(|| panic!("missing event with field {key:?}"))
}

/// `(message, level, target, action, fields)` for every fixture call with a message.
#[expect(clippy::too_many_lines, reason = "one row per fixture call")]
fn expected() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Value,
)> {
    let d = DEFAULT_TARGET;
    let a = DEFAULT_ACTION;
    vec![
        ("compat: plain 7", "Info", d, a, json!({})),
        (
            "compat: target literal",
            "Info",
            "compat.literal_target",
            a,
            json!({}),
        ),
        (
            "compat: target const",
            "Info",
            "compat.const_target",
            a,
            json!({}),
        ),
        ("compat: target module_path", "Info", d, a, json!({})),
        (
            "compat: target concat",
            "Info",
            "compat.concat_target",
            a,
            json!({}),
        ),
        (
            "compat: name literal",
            "Info",
            d,
            "compat.literal_name",
            json!({}),
        ),
        (
            "compat: name const",
            "Info",
            d,
            "compat.const_name",
            json!({}),
        ),
        (
            "compat: name concat",
            "Info",
            d,
            "compat.concat_name",
            json!({}),
        ),
        (
            "compat: name and target",
            "Info",
            "compat.both_target",
            "compat.both_name",
            json!({}),
        ),
        ("compat: key", "Info", d, a, json!({"k": 7})),
        ("compat: dotted key", "Info", d, a, json!({"a.b": 7})),
        (
            "compat: literal key",
            "Info",
            d,
            a,
            json!({"literal_key": 7}),
        ),
        ("compat: raw key", "Info", d, a, json!({"type": "beads"})),
        (
            "compat: dynamic key",
            "Info",
            d,
            a,
            json!({"compat.dynamic": 7}),
        ),
        (
            "compat: dynamic key debug",
            "Info",
            d,
            a,
            json!({"compat.dynamic": "\"beads\""}),
        ),
        (
            "compat: dynamic key display",
            "Info",
            d,
            a,
            json!({"compat.dynamic": "a/b"}),
        ),
        (
            "compat: debug shorthand",
            "Info",
            d,
            a,
            json!({"label": "\"beads\""}),
        ),
        (
            "compat: debug value",
            "Info",
            d,
            a,
            json!({"k": "\"beads\""}),
        ),
        (
            "compat: display shorthand",
            "Info",
            d,
            a,
            json!({"path": "a/b"}),
        ),
        ("compat: display value", "Info", d, a, json!({"k": "a/b"})),
        ("compat: shorthand", "Info", d, a, json!({"count": 7})),
        (
            "compat: dotted shorthand",
            "Info",
            d,
            a,
            json!({"point.x": 3}),
        ),
        (
            "compat: brace 7",
            "Info",
            d,
            a,
            json!({"k": 1, "label": "\"beads\""}),
        ),
        (
            "compat: brace after name",
            "Info",
            d,
            "compat.brace_name",
            json!({"k": 1}),
        ),
        (
            "compat: brace after target",
            "Info",
            "compat.brace_target",
            a,
            json!({"k": 1}),
        ),
        (
            "compat: brace after both",
            "Info",
            "compat.brace_both_target",
            "compat.brace_both_name",
            json!({"k": 1}),
        ),
        ("compat: event brace", "Info", d, a, json!({"k": 1})),
        (
            "compat: fields then message beads",
            "Info",
            d,
            a,
            json!({"k": 7, "ok": true}),
        ),
        ("compat: event level", "Info", d, a, json!({})),
        (
            "compat: event target",
            "Warn",
            "compat.event_target",
            a,
            json!({}),
        ),
        (
            "compat: event name",
            "Info",
            d,
            "compat.event_name",
            json!({}),
        ),
        (
            "compat: event both",
            "Error",
            "compat.event_both_target",
            "compat.event_both_name",
            json!({}),
        ),
        ("compat: event const level", "Warn", d, a, json!({})),
        ("compat: trace", "Trace", d, a, json!({"k": 1})),
        ("compat: debug", "Debug", d, a, json!({"k": 1})),
        ("compat: warn", "Warn", d, a, json!({"k": 1})),
        ("compat: error", "Error", d, a, json!({"k": 1})),
    ]
}

fn assert_fixture_events(events: &[Value]) {
    for (message, level, target, action, fields) in expected() {
        let event = by_message(events, message);
        assert_eq!(event["level"], level, "level of {message:?}");
        assert_eq!(event["target"], target, "target of {message:?}");
        assert_eq!(event["action"], action, "action of {message:?}");
        assert_eq!(event["fields"], fields, "fields of {message:?}");
    }
    let shorthand_only = by_field(events, "compat_shorthand_only");
    assert!(shorthand_only.get("message").is_none_or(Value::is_null));
    assert_eq!(
        shorthand_only["fields"],
        json!({"compat_shorthand_only": true})
    );
    let brace_only = by_field(events, "compat_brace_only");
    assert!(brace_only.get("message").is_none_or(Value::is_null));
    assert_eq!(brace_only["fields"], json!({"compat_brace_only": 1}));
    // Every fixture call emitted exactly one event.
    assert_eq!(events.len(), expected().len() + 2);
}

#[test]
fn compat_fixture_matches_tracing_syntax_and_emits_jsonl() {
    // Compile-only: referencing the tracing build keeps it type-checked and live.
    let _: fn() = tracing_syntax::emit_compat_events;

    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("compat-events").unwrap(),
        root.path().into(),
    );
    config.level = LevelFilter::Trace;
    let options = BridgeOptions {
        default_action: ActionName::new(DEFAULT_ACTION).unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let path = guard.active_log_path().unwrap().to_path_buf();

    sc_syntax::emit_compat_events();
    guard.flush(Duration::from_secs(5)).unwrap();
    assert_fixture_events(&read_events(&path));
    assert_eq!(guard.dropped_events().total(), 0);
    guard.shutdown(Duration::from_secs(5)).unwrap();
}
