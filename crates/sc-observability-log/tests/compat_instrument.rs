//! Compiles the shared fixture `tests/compat/instrument.rs` against
//! `tracing::instrument` (compile-only) and against
//! `sc_observability_log::instrument` (executed and asserted against the JSONL
//! output). One `init` per test binary.
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

/// The fixture compiled with the genuine tracing attribute: proves the syntax is tracing syntax.
mod tracing_syntax {
    use tracing::{Level, instrument};

    include!("compat/instrument.rs");
}

/// The same fixture compiled with the sc-observability-log attribute.
mod sc_syntax {
    use sc_observability_log::{Level, instrument};

    include!("compat/instrument.rs");
}

const DEFAULT_ACTION: &str = "compat.default";
const DEFAULT_TARGET: &str = "compat_instrument.sc_syntax";

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn events_for<'a>(events: &'a [Value], action: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event["action"] == action)
        .collect()
}

/// Removes and checks `duration_ms` (a u64), then returns the remaining fields.
fn fields_without_duration(event: &Value) -> Value {
    let mut fields = event["fields"].clone();
    let duration = fields
        .as_object_mut()
        .unwrap()
        .remove("duration_ms")
        .unwrap_or_else(|| panic!("duration_ms missing in {event}"));
    assert!(duration.is_u64(), "duration_ms is a u64 in {event}");
    fields
}

/// Asserts one completion event: level, target, outcome, fields, no message, a trace.
fn assert_completion(event: &Value, (level, target, outcome): (&str, &str, &str), fields: &Value) {
    let action = &event["action"];
    assert_eq!(event["level"], level, "level of {action}");
    assert_eq!(event["target"], target, "target of {action}");
    assert_eq!(event["outcome"], outcome, "outcome of {action}");
    assert!(event["message"].is_null(), "message of {action}");
    assert_eq!(
        &fields_without_duration(event),
        fields,
        "fields of {action}"
    );
    let trace = &event["trace"];
    assert_eq!(
        trace["trace_id"].as_str().unwrap().len(),
        32,
        "trace of {action}"
    );
    assert_eq!(
        trace["span_id"].as_str().unwrap().len(),
        16,
        "span of {action}"
    );
}

/// `(action, level, target, outcome, fields)` for every single-call fixture fn.
#[expect(clippy::too_many_lines, reason = "one row per fixture fn")]
fn expected_single() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Value,
)> {
    let d = DEFAULT_TARGET;
    vec![
        ("compat.name_literal", "Info", d, "ok", json!({})),
        ("compat.instrument.const_name", "Info", d, "ok", json!({})),
        ("compat.name_positional", "Info", d, "ok", json!({})),
        ("compat_default_name", "Info", d, "ok", json!({})),
        (
            "compat.target_literal",
            "Info",
            "compat.instrument.literal_target",
            "ok",
            json!({}),
        ),
        (
            "compat.target_const",
            "Info",
            "compat.instrument.const_target",
            "ok",
            json!({}),
        ),
        ("compat.level_str", "Warn", d, "ok", json!({})),
        ("compat.level_path", "Debug", d, "ok", json!({})),
        ("compat.level_const", "Warn", d, "ok", json!({})),
        ("compat.level_int_trace", "Trace", d, "ok", json!({})),
        ("compat.level_int_error", "Error", d, "ok", json!({})),
        (
            "compat.default_args",
            "Info",
            d,
            "ok",
            json!({"count": 3, "label": "beads", "widget": "CompatWidget { id: 1 }"}),
        ),
        ("compat.skip_all", "Info", d, "ok", json!({})),
        (
            "compat.fields",
            "Info",
            d,
            "ok",
            json!({
                "k": 7,
                "label": "\"beads\"",
                "path": "a/b",
                "a.b": 1,
                "compat.dynamic": 1,
                "compat.dynamic_debug": "\"beads\"",
                "type": 1,
            }),
        ),
        (
            "compat.method_self",
            "Info",
            d,
            "ok",
            json!({"self": "CompatWidget { id: 1 }", "n": 2}),
        ),
        ("compat.skip", "Info", d, "ok", json!({"n": 1})),
        ("compat.generic", "Info", d, "ok", json!({"t": "\"serde\""})),
        (
            "compat.method_owned",
            "Info",
            d,
            "ok",
            json!({"self": "CompatWidget { id: 8 }"}),
        ),
        (
            "compat.ret",
            "Info",
            d,
            "ok",
            json!({"n": 21, "return": "42"}),
        ),
        (
            "compat.ret_debug",
            "Info",
            d,
            "ok",
            json!({"return": "\"beads\""}),
        ),
        (
            "compat.ret_display",
            "Info",
            d,
            "ok",
            json!({"return": "beads"}),
        ),
        (
            "compat.ret_level",
            "Debug",
            d,
            "ok",
            json!({"n": 1, "return": "1"}),
        ),
        (
            "compat.ret_debug_level",
            "Warn",
            d,
            "ok",
            json!({"return": "\"beads\""}),
        ),
        (
            "compat.ret_display_level",
            "Debug",
            d,
            "ok",
            json!({"return": "beads"}),
        ),
        (
            "compat.async_outer",
            "Info",
            d,
            "ok",
            json!({"n": 40, "stage": "outer"}),
        ),
        (
            "compat.async_inner",
            "Info",
            d,
            "ok",
            json!({"n": 40, "return": "40"}),
        ),
    ]
}

/// `(action, ok event, err event)` for the fns called with `Ok` and then `Err`:
/// each tuple is `(level, fields)`.
type ErrRow = (&'static str, (&'static str, Value), (&'static str, Value));

fn expected_err() -> Vec<ErrRow> {
    vec![
        (
            "compat.err",
            ("Info", json!({"fail": false})),
            ("Error", json!({"fail": true, "error": "compat failure 1"})),
        ),
        (
            "compat.err_debug",
            ("Info", json!({"fail": false})),
            (
                "Error",
                json!({"fail": true, "error": "CompatError { code: 2 }"}),
            ),
        ),
        (
            "compat.err_display",
            ("Info", json!({"fail": false})),
            ("Error", json!({"fail": true, "error": "compat failure 3"})),
        ),
        (
            "compat.err_level",
            ("Info", json!({"fail": false})),
            ("Warn", json!({"fail": true, "error": "compat failure 4"})),
        ),
        (
            "compat.err_debug_level",
            ("Info", json!({"fail": false})),
            (
                "Info",
                json!({"fail": true, "error": "CompatError { code: 5 }"}),
            ),
        ),
        (
            "compat.err_display_level",
            ("Info", json!({"fail": false})),
            ("Warn", json!({"fail": true, "error": "compat failure 6"})),
        ),
        (
            "compat.ret_err",
            ("Info", json!({"fail": false, "return": "7"})),
            ("Error", json!({"fail": true, "error": "compat failure 7"})),
        ),
    ]
}

fn assert_fixture_events(events: &[Value]) {
    for (action, level, target, outcome, fields) in expected_single() {
        let matching = events_for(events, action);
        assert_eq!(
            matching.len(),
            1,
            "exactly one completion event for {action}"
        );
        assert_completion(matching[0], (level, target, outcome), &fields);
    }
    for (action, (ok_level, ok_fields), (err_level, err_fields)) in expected_err() {
        let matching = events_for(events, action);
        assert_eq!(matching.len(), 2, "one event per call for {action}");
        assert_completion(matching[0], (ok_level, DEFAULT_TARGET, "ok"), &ok_fields);
        assert_completion(
            matching[1],
            (err_level, DEFAULT_TARGET, "error"),
            &err_fields,
        );
    }
    let outer = events_for(events, "compat.async_outer")[0];
    let inner = events_for(events, "compat.async_inner")[0];
    assert_eq!(inner["trace"]["trace_id"], outer["trace"]["trace_id"]);
    assert_eq!(inner["trace"]["parent_span_id"], outer["trace"]["span_id"]);
    // Every fixture call emitted exactly one completion event and nothing else.
    assert_eq!(
        events.len(),
        expected_single().len() + 2 * expected_err().len()
    );
}

#[test]
fn compat_fixture_matches_tracing_syntax_and_emits_jsonl() {
    // Compile-only: referencing the tracing build keeps it type-checked and live.
    let _: fn() -> usize = tracing_syntax::run_compat_instrument;

    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("compat-instrument").unwrap(),
        root.path().into(),
    );
    config.level = LevelFilter::Trace;
    let options = BridgeOptions {
        default_action: ActionName::new(DEFAULT_ACTION).unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let path = guard.active_log_path().unwrap().to_path_buf();

    let total = sc_syntax::run_compat_instrument();
    assert_eq!(total, sc_syntax_expected_total());
    guard.flush(Duration::from_secs(5)).unwrap();
    assert_fixture_events(&read_events(&path));
    assert_eq!(guard.dropped_events().total(), 0);
    guard.shutdown(Duration::from_secs(5)).unwrap();
}

/// The fixture's return-value sum: the bodies' values pass through unchanged.
fn sc_syntax_expected_total() -> usize {
    // default_args 9, skip_all 8, fields 15, method_self 3, generic "8:\"serde\"" 9,
    // method_owned 8, ret 42, ret_debug 5, ret_display 5, ret_level 1,
    // ret_debug_level 5, ret_display_level 5, err fns 2 * (1..=7) = 56, async 41.
    9 + 8 + 15 + 3 + 9 + 8 + 42 + 5 + 5 + 1 + 5 + 5 + 56 + 41
}
