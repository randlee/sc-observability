//! `#[instrument]` → JSONL: one assertion per argument-table row and completion
//! event field, every `CallOutcome`, trace-context propagation (nested sync
//! calls, async across `.await` on a multi-thread runtime and across threads),
//! runtime label failures and disabled-level laziness.
//!
//! One `init` per test binary: every sub-case runs inside the single test fn
//! (the tokio runtime is built inside it), so the `DropCause::InvalidEvent`
//! delta asserted here cannot race.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::fmt;
use std::future::Future;
use std::num::ParseIntError;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use sc_observability_log::{
    ActionName, BridgeOptions, DropCause, Level, LevelFilter, LogGuard, LoggerConfig, ServiceName,
    info, instrument,
};
use serde::ser::SerializeStruct as _;
use serde_json::{Value, json};

const DEFAULT_ACTION: &str = "instrument.default";
const MODULE_TARGET: &str = "instrument_jsonl";

const NAME: &str = "jsonl.const_name";
const TGT: &str = "jsonl.const_target";
const LVL: Level = Level::WARN;
const KEY: &str = "jsonl.dynamic";
const DEBUG_KEY: &str = "jsonl dynamic debug";
const EMPTY: &str = "";
const RESERVED: &str = "sc_observability_log::x";

// ---- helpers ----

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn flushed_events(guard: &LogGuard, path: &Path) -> Vec<Value> {
    guard.flush(Duration::from_secs(5)).unwrap();
    read_events(path)
}

fn with_action<'a>(events: &'a [Value], action: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event["action"] == action)
        .collect()
}

/// The single event with `action`; fails on zero or several.
fn one<'a>(events: &'a [Value], action: &str) -> &'a Value {
    let matching = with_action(events, action);
    assert_eq!(
        matching.len(),
        1,
        "exactly one event with action {action:?}"
    );
    matching[0]
}

fn by_message<'a>(events: &'a [Value], message: &str) -> &'a Value {
    let matching: Vec<_> = events
        .iter()
        .filter(|event| event["message"] == message)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "exactly one event with message {message:?}"
    );
    matching[0]
}

/// Asserts a completion event: every field of the completion event table.
///
/// `duration_ms` is checked to be a u64 and removed before comparing `fields`.
fn assert_completion(
    event: &Value,
    (level, target, outcome): (&str, &str, Option<&str>),
    fields: &Value,
) {
    let action = &event["action"];
    assert_eq!(event["level"], level, "level of {action}");
    assert_eq!(event["target"], target, "target of {action}");
    match outcome {
        Some(outcome) => assert_eq!(event["outcome"], outcome, "outcome of {action}"),
        None => assert!(event["outcome"].is_null(), "outcome of {action}"),
    }
    assert!(event["message"].is_null(), "message of {action}");
    let mut actual = event["fields"].clone();
    let duration = actual.as_object_mut().unwrap().remove("duration_ms");
    assert!(
        duration.as_ref().is_some_and(Value::is_u64),
        "duration_ms of {action}"
    );
    assert_eq!(&actual, fields, "fields of {action}");
    assert_trace_shape(event);
}

fn assert_trace_shape(event: &Value) {
    let trace = &event["trace"];
    let hex = |value: &Value, len: usize| {
        let text = value
            .as_str()
            .unwrap_or_else(|| panic!("hex id in {event}"));
        text.len() == len
            && text
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    };
    assert!(hex(&trace["trace_id"], 32), "trace_id of {event}");
    assert!(hex(&trace["span_id"], 16), "span_id of {event}");
}

fn span_id(event: &Value) -> &Value {
    &event["trace"]["span_id"]
}

fn trace_id(event: &Value) -> &Value {
    &event["trace"]["trace_id"]
}

fn parent_span_id(event: &Value) -> &Value {
    &event["trace"]["parent_span_id"]
}

fn ok(level: &str) -> (&str, &str, Option<&str>) {
    (level, MODULE_TARGET, Some("ok"))
}

// ---- value types ----

/// Implements both `Serialize` and `Debug`.
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

/// An error with distinct `Debug` and `Display` text.
#[derive(Debug)]
struct Failure(u32);

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failure {}", self.0)
    }
}

/// `Debug` and `Serialize` both panic: proves a disabled call never records its args.
struct Explosive;

impl fmt::Debug for Explosive {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("Explosive::fmt must not run for a disabled level");
    }
}

impl serde::Serialize for Explosive {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        panic!("Explosive::serialize must not run for a disabled level");
    }
}

fn result(fail: bool, code: u32) -> Result<u32, Failure> {
    if fail { Err(Failure(code)) } else { Ok(code) }
}

// ---- argument table: name, target, level ----

#[instrument("jsonl.positional", skip_all)]
fn name_positional() {}

#[instrument(name = "jsonl.literal_name", skip_all)]
fn name_literal() {}

#[instrument(name = NAME, skip_all)]
fn name_const() {}

#[instrument(skip_all)]
fn default_name_and_target() {}

#[instrument(
    name = "jsonl.literal_target",
    target = "jsonl.literal_target",
    skip_all
)]
fn target_literal() {}

#[instrument(name = "jsonl.const_target", target = TGT, skip_all)]
fn target_const() {}

#[instrument(name = "jsonl.bad_target", target = "bad target::x", skip_all)]
fn target_sanitized() {}

#[instrument(name = "jsonl.level_str", level = "ERROR", skip_all)]
fn level_str() {}

#[instrument(name = "jsonl.level_path", level = Level::WARN, skip_all)]
fn level_path() {}

#[instrument(name = "jsonl.level_const", level = LVL, skip_all)]
fn level_const() {}

#[instrument(name = "jsonl.level_2", level = 2, skip_all)]
fn level_int_2() {}

#[instrument(name = "jsonl.level_3", level = 3, skip_all)]
fn level_int_3() {}

#[instrument(name = "jsonl.level_4", level = 4, skip_all)]
fn level_int_4() {}

#[instrument(name = "jsonl.level_5", level = 5, skip_all)]
fn level_int_5() {}

// ---- argument table: args, skip, skip_all, fields ----

#[instrument(name = "jsonl.default_args")]
fn default_args(count: u32, label: &str, both: &Both, failure: &Failure, (x, y): (u8, u8)) -> u32 {
    count
        + u32::from(x)
        + u32::from(y)
        + u32::try_from(label.len() + failure.to_string().len()).unwrap()
        + u32::try_from(both.a).unwrap()
}

#[instrument(name = "jsonl.skip", skip(label, secret))]
fn skip_args(count: u32, label: &str, secret: &str) -> usize {
    usize::try_from(count).unwrap() + label.len() + secret.len()
}

#[instrument(name = "jsonl.skip_all", skip_all)]
fn skip_all_args(count: u32, label: &str) -> usize {
    usize::try_from(count).unwrap() + label.len()
}

#[instrument(
    name = "jsonl.fields",
    skip(label, path),
    fields(k = count + 1, ?label, %path, a.b = 1, { KEY } = 2, { DEBUG_KEY } = ?label, r#type = "t", count = count * 10)
)]
fn fields_forms(count: u32, label: &str, path: &str) -> usize {
    usize::try_from(count).unwrap() + label.len() + path.len()
}

#[instrument(name = "", skip_all, fields({ EMPTY } = 1, { RESERVED } = 2, kept = 3))]
fn label_failures() {}

// ---- QA-1 RBP-F001: completion keys shadow same-named user fields ----

/// A parameter named `error`, combined with `err`, collides with the
/// completion key written for the `Err` outcome.
#[instrument(name = "jsonl.shadow_error_param", err)]
fn shadow_error_param(error: &str) -> Result<u32, Failure> {
    let _ = error;
    Err(Failure(9))
}

/// A `fields(duration_ms = ..)` entry collides with the real `duration_ms`.
#[instrument(
    name = "jsonl.shadow_duration_field",
    skip_all,
    fields(duration_ms = 7)
)]
fn shadow_duration_field() {}

/// A parameter named `duration_ms` collides with the real `duration_ms`.
#[instrument(name = "jsonl.shadow_duration_param")]
fn shadow_duration_param(duration_ms: u32) -> u32 {
    duration_ms
}

/// No collision: `sc_observability_log.shadowed_fields` must not appear.
#[instrument(name = "jsonl.no_collision")]
fn no_collision(count: u32) -> u32 {
    count
}

#[derive(Debug)]
struct Service {
    id: u32,
}

impl Service {
    #[instrument(name = "jsonl.self_ref")]
    fn by_ref(&self, n: u32) -> u32 {
        self.id + n
    }

    #[instrument(name = "jsonl.self_mut")]
    fn by_mut(&mut self, n: u32) {
        self.id += n;
    }

    #[instrument(name = "jsonl.self_owned")]
    fn owned(self) -> u32 {
        self.id
    }

    #[instrument(name = "jsonl.skip_self", skip(self))]
    fn skip_self(&self, n: u32) -> u32 {
        self.id * n
    }

    #[instrument(name = "jsonl.generic_debug", skip(self))]
    fn generic_debug<T: fmt::Debug>(&self, t: T) -> String {
        format!("{}:{t:?}", self.id)
    }

    #[instrument(name = "jsonl.generic_both", skip(self))]
    fn generic_both<T>(&self, t: T) -> String
    where
        T: fmt::Debug + serde::Serialize,
    {
        format!("{}:{t:?}", self.id)
    }
}

// ---- argument table: ret and err ----

#[instrument(name = "jsonl.ret", ret)]
fn ret_bare(label: &str) -> String {
    label.to_uppercase()
}

#[instrument(name = "jsonl.ret_debug", ret(Debug), skip_all)]
fn ret_debug() -> Failure {
    Failure(1)
}

#[instrument(name = "jsonl.ret_display", ret(Display), skip_all)]
fn ret_display() -> Failure {
    Failure(2)
}

#[instrument(name = "jsonl.ret_level", ret(level = "warn"), skip_all)]
fn ret_level() -> u32 {
    3
}

#[instrument(name = "jsonl.ret_debug_level", ret(Debug, level = Level::ERROR), skip_all)]
fn ret_debug_level() -> Failure {
    Failure(4)
}

#[instrument(name = "jsonl.ret_display_level", ret(Display, level = 4), skip_all)]
fn ret_display_level() -> Failure {
    Failure(5)
}

#[instrument(name = "jsonl.ret_unit", ret, skip_all)]
fn ret_unit() {}

#[instrument(name = "jsonl.err", err)]
fn err_bare(fail: bool) -> Result<u32, Failure> {
    result(fail, 1)
}

#[instrument(name = "jsonl.err_debug", err(Debug))]
fn err_debug(fail: bool) -> Result<u32, Failure> {
    result(fail, 2)
}

#[instrument(name = "jsonl.err_display", err(Display))]
fn err_display(fail: bool) -> Result<u32, Failure> {
    result(fail, 3)
}

#[instrument(name = "jsonl.err_level", err(level = "warn"))]
fn err_level(fail: bool) -> Result<u32, Failure> {
    result(fail, 4)
}

#[instrument(name = "jsonl.err_debug_level", err(Debug, level = LVL))]
fn err_debug_level(fail: bool) -> Result<u32, Failure> {
    result(fail, 5)
}

#[instrument(name = "jsonl.err_display_level", err(Display, level = 3))]
fn err_display_level(fail: bool) -> Result<u32, Failure> {
    result(fail, 6)
}

#[instrument(name = "jsonl.ret_err", ret(Display), err(Debug), level = "debug")]
fn ret_err(fail: bool) -> Result<Failure, Failure> {
    if fail {
        Err(Failure(7))
    } else {
        Ok(Failure(8))
    }
}

// ---- early exits and panics (sync) ----

#[instrument(name = "early.return_ok", err)]
fn early_return_ok(n: u32) -> Result<u32, String> {
    if n > 0 {
        return Ok(n);
    }
    Err("not reached".to_owned())
}

#[instrument(name = "early.return_err", err)]
fn early_return_err(n: u32) -> Result<u32, String> {
    if n > 0 {
        return Err(format!("early {n}"));
    }
    Ok(0)
}

#[instrument(name = "early.question", err)]
fn early_question(input: &str) -> Result<u32, ParseIntError> {
    let n: u32 = input.parse()?;
    Ok(n + 1)
}

#[instrument(name = "early.no_err")]
fn early_no_err(n: u32) -> u32 {
    if n > 0 {
        return n * 10;
    }
    0
}

#[instrument(name = "sync.panics")]
fn sync_panics(n: u32) -> u32 {
    assert!(n == 0, "sync instrumented panic (expected by this test)");
    n
}

#[instrument(name = "lazy.disabled", level = "trace", ret)]
fn lazy_disabled(value: &Explosive) -> &Explosive {
    value
}

// ---- trace context (sync) ----

#[instrument(name = "nested.outer")]
fn nested_outer(n: u32) -> u32 {
    info!(name: "nested.outer.before", "outer before");
    let inner = nested_inner(n + 1);
    log::info!(target: "nested", "outer bridge after");
    inner + 1
}

#[instrument(name = "nested.inner")]
fn nested_inner(n: u32) -> u32 {
    info!(name: "nested.inner.event", "inner event");
    log::warn!(target: "nested", "inner bridge");
    n
}

// ---- async ----

#[instrument(name = "async.hop")]
async fn async_hop(task: u32) -> u32 {
    info!(name: "async.hop.before", task, "hop before");
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    info!(name: "async.hop.after", task, "hop after");
    async_hop_inner(task).await
}

#[instrument(name = "async.hop.inner", ret)]
async fn async_hop_inner(task: u32) -> u32 {
    tokio::task::yield_now().await;
    log::info!(target: "async", "hop bridge {task}");
    task
}

/// Returns `Pending` once (waking itself), then `Ready`.
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn thread_label() -> String {
    format!("{:?}", std::thread::current().id())
}

#[instrument(name = "async.cross_thread", skip_all)]
async fn cross_thread() -> String {
    info!(name: "cross.before", thread = %thread_label(), "cross before");
    YieldOnce(false).await;
    info!(name: "cross.after", thread = %thread_label(), "cross after");
    thread_label()
}

#[instrument(name = "async.cancelled")]
async fn cancelled(sleep_ms: u64) {
    info!(name: "async.cancelled.started", "cancel started");
    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
}

#[instrument(name = "async.panics")]
async fn async_panics(n: u32) -> u32 {
    tokio::task::yield_now().await;
    assert!(n == 0, "async instrumented panic (expected by this test)");
    n
}

#[instrument(name = "async.never_polled")]
async fn never_polled(n: u32) -> u32 {
    tokio::task::yield_now().await;
    n
}

#[instrument(name = "async.err", err(level = "warn"), skip_all)]
async fn async_err(fail: bool) -> Result<u32, Failure> {
    tokio::task::yield_now().await;
    result(fail, 9)
}

fn assert_send<F: Future + Send>(future: F) -> F {
    future
}

fn block_on_here<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
    }
}

// ---- sub-cases ----

fn call_argument_table() {
    name_positional();
    name_literal();
    name_const();
    default_name_and_target();
    target_literal();
    target_const();
    target_sanitized();
    level_str();
    level_path();
    level_const();
    level_int_2();
    level_int_3();
    level_int_4();
    level_int_5();
    assert_eq!(
        default_args(1, "ab", &Both { a: 4 }, &Failure(3), (5, 6)),
        1 + 5 + 6 + 2 + 9 + 4
    );
    assert_eq!(skip_args(1, "ab", "cde"), 6);
    assert_eq!(skip_all_args(1, "ab"), 3);
    assert_eq!(fields_forms(2, "beads", "a/b"), 10);
    let mut service = Service { id: 7 };
    assert_eq!(service.by_ref(1), 8);
    service.by_mut(1);
    assert_eq!(service.skip_self(2), 16);
    assert_eq!(service.generic_debug(Both { a: 4 }), "8:Both { a: 4 }");
    assert_eq!(service.generic_both(Both { a: 3 }), "8:Both { a: 3 }");
    assert_eq!(service.owned(), 8);

    assert_eq!(ret_bare("beads"), "BEADS");
    assert_eq!(ret_debug().0, 1);
    assert_eq!(ret_display().0, 2);
    assert_eq!(ret_level(), 3);
    assert_eq!(ret_debug_level().0, 4);
    assert_eq!(ret_display_level().0, 5);
    ret_unit();
    for fail in [false, true] {
        let _ = err_bare(fail);
        let _ = err_debug(fail);
        let _ = err_display(fail);
        let _ = err_level(fail);
        let _ = err_debug_level(fail);
        let _ = err_display_level(fail);
        let _ = ret_err(fail);
    }

    // QA-1 RBP-F001: completion keys (`duration_ms`, `return`/`error`) shadow
    // same-named user fields instead of silently overwriting them.
    let _ = shadow_error_param("boom");
    shadow_duration_field();
    assert_eq!(shadow_duration_param(42), 42);
    assert_eq!(no_collision(5), 5);
}

#[expect(
    clippy::too_many_lines,
    reason = "one assertion per argument-table row"
)]
fn assert_argument_table(events: &[Value]) {
    let m = MODULE_TARGET;
    assert_completion(one(events, "jsonl.positional"), ok("Info"), &json!({}));
    assert_completion(one(events, "jsonl.literal_name"), ok("Info"), &json!({}));
    assert_completion(one(events, NAME), ok("Info"), &json!({}));
    assert_completion(
        one(events, "default_name_and_target"),
        ok("Info"),
        &json!({}),
    );
    assert_completion(
        one(events, "jsonl.literal_target"),
        ("Info", "jsonl.literal_target", Some("ok")),
        &json!({}),
    );
    assert_completion(
        one(events, "jsonl.const_target"),
        ("Info", TGT, Some("ok")),
        &json!({}),
    );
    assert_completion(
        one(events, "jsonl.bad_target"),
        ("Info", "bad_target.x", Some("ok")),
        &json!({}),
    );
    assert_completion(one(events, "jsonl.level_str"), ok("Error"), &json!({}));
    assert_completion(one(events, "jsonl.level_path"), ok("Warn"), &json!({}));
    assert_completion(one(events, "jsonl.level_const"), ok("Warn"), &json!({}));
    assert_completion(one(events, "jsonl.level_2"), ok("Debug"), &json!({}));
    assert_completion(one(events, "jsonl.level_3"), ok("Info"), &json!({}));
    assert_completion(one(events, "jsonl.level_4"), ok("Warn"), &json!({}));
    assert_completion(one(events, "jsonl.level_5"), ok("Error"), &json!({}));

    // Default recording: bare-field dispatch per argument, pattern bindings included.
    assert_completion(
        one(events, "jsonl.default_args"),
        ok("Info"),
        &json!({
            "count": 1,
            "label": "ab",
            "both": {"a": 4},
            "failure": "Failure(3)",
            "x": 5,
            "y": 6,
        }),
    );
    assert_completion(one(events, "jsonl.skip"), ok("Info"), &json!({"count": 1}));
    assert_completion(one(events, "jsonl.skip_all"), ok("Info"), &json!({}));
    // `count = count * 10` replaces the recorded argument, as in tracing.
    assert_completion(
        one(events, "jsonl.fields"),
        ok("Info"),
        &json!({
            "k": 3,
            "label": "\"beads\"",
            "path": "a/b",
            "a.b": 1,
            "jsonl.dynamic": 2,
            "jsonl_dynamic_debug": "\"beads\"",
            "type": "t",
            "count": 20,
        }),
    );
    assert_completion(
        one(events, "jsonl.self_ref"),
        ok("Info"),
        &json!({"self": "Service { id: 7 }", "n": 1}),
    );
    assert_completion(
        one(events, "jsonl.self_mut"),
        ok("Info"),
        &json!({"self": "Service { id: 7 }", "n": 1}),
    );
    assert_completion(
        one(events, "jsonl.self_owned"),
        ok("Info"),
        &json!({"self": "Service { id: 8 }"}),
    );
    assert_completion(one(events, "jsonl.skip_self"), ok("Info"), &json!({"n": 2}));
    // Static-type dispatch: `T: Debug` records the Debug string, `T: Debug + Serialize` JSON.
    assert_completion(
        one(events, "jsonl.generic_debug"),
        ok("Info"),
        &json!({"t": "Both { a: 4 }"}),
    );
    assert_completion(
        one(events, "jsonl.generic_both"),
        ok("Info"),
        &json!({"t": {"a": 3}}),
    );

    // ret: bare = Debug; without `err` the whole return value.
    assert_completion(
        one(events, "jsonl.ret"),
        ok("Info"),
        &json!({"label": "beads", "return": "\"BEADS\""}),
    );
    assert_completion(
        one(events, "jsonl.ret_debug"),
        ok("Info"),
        &json!({"return": "Failure(1)"}),
    );
    assert_completion(
        one(events, "jsonl.ret_display"),
        ok("Info"),
        &json!({"return": "failure 2"}),
    );
    assert_completion(
        one(events, "jsonl.ret_level"),
        ok("Warn"),
        &json!({"return": "3"}),
    );
    assert_completion(
        one(events, "jsonl.ret_debug_level"),
        ok("Error"),
        &json!({"return": "Failure(4)"}),
    );
    assert_completion(
        one(events, "jsonl.ret_display_level"),
        ok("Warn"),
        &json!({"return": "failure 5"}),
    );
    assert_completion(
        one(events, "jsonl.ret_unit"),
        ok("Info"),
        &json!({"return": "()"}),
    );

    // err: bare = Display; `Err` → outcome error at ERROR or the `err(level)`.
    for (action, err_level, error) in [
        ("jsonl.err", "Error", "failure 1"),
        ("jsonl.err_debug", "Error", "Failure(2)"),
        ("jsonl.err_display", "Error", "failure 3"),
        ("jsonl.err_level", "Warn", "failure 4"),
        ("jsonl.err_debug_level", "Warn", "Failure(5)"),
        ("jsonl.err_display_level", "Info", "failure 6"),
    ] {
        let calls = with_action(events, action);
        assert_eq!(calls.len(), 2, "one completion per call of {action}");
        assert_completion(calls[0], ok("Info"), &json!({"fail": false}));
        assert_completion(
            calls[1],
            (err_level, m, Some("error")),
            &json!({"fail": true, "error": error}),
        );
    }
    // ret + err: `ret` records the `Ok` value; `ok` uses `level`, `error` uses ERROR.
    let calls = with_action(events, "jsonl.ret_err");
    assert_eq!(calls.len(), 2);
    assert_completion(
        calls[0],
        ok("Debug"),
        &json!({"fail": false, "return": "failure 8"}),
    );
    assert_completion(
        calls[1],
        ("Error", m, Some("error")),
        &json!({"fail": true, "error": "Failure(7)"}),
    );

    // QA-1 RBP-F001: the completion key wins; the displaced user value is
    // preserved under `sc_observability_log.shadowed_fields`.
    assert_completion(
        one(events, "jsonl.shadow_error_param"),
        ("Error", m, Some("error")),
        &json!({
            "error": "failure 9",
            "sc_observability_log.shadowed_fields": {"error": "boom"},
        }),
    );
    assert_completion(
        one(events, "jsonl.shadow_duration_field"),
        ok("Info"),
        &json!({"sc_observability_log.shadowed_fields": {"duration_ms": 7}}),
    );
    assert_completion(
        one(events, "jsonl.shadow_duration_param"),
        ok("Info"),
        &json!({"sc_observability_log.shadowed_fields": {"duration_ms": 42}}),
    );
    // No collision: `shadowed_fields` must not appear at all.
    let no_collision_event = one(events, "jsonl.no_collision");
    assert_completion(no_collision_event, ok("Info"), &json!({"count": 5}));
    assert!(
        no_collision_event["fields"]
            .get("sc_observability_log.shadowed_fields")
            .is_none(),
        "no shadowed_fields key without a collision"
    );
}

fn call_early_exits_and_panics() {
    assert_eq!(early_return_ok(4), Ok(4));
    assert_eq!(early_return_err(4), Err("early 4".to_owned()));
    assert!(early_question("x").is_err());
    assert_eq!(early_question("41"), Ok(42));
    assert_eq!(early_no_err(4), 40);

    let caught = std::panic::catch_unwind(|| sync_panics(1));
    let payload = caught.expect_err("the user panic propagates to catch_unwind");
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"sync instrumented panic (expected by this test)")
    );
    // Level `trace` is below the configured `Debug`: no argument or return value is formatted.
    let explosive = Explosive;
    assert!(std::ptr::eq(
        lazy_disabled(&explosive),
        std::ptr::from_ref(&explosive)
    ));
}

fn assert_early_exits_and_panics(events: &[Value]) {
    let m = MODULE_TARGET;
    assert_completion(one(events, "early.return_ok"), ok("Info"), &json!({"n": 4}));
    assert_completion(
        one(events, "early.return_err"),
        ("Error", m, Some("error")),
        &json!({"n": 4, "error": "early 4"}),
    );
    let question = with_action(events, "early.question");
    assert_eq!(question.len(), 2, "one completion per `?` call");
    assert_completion(
        question[0],
        ("Error", m, Some("error")),
        &json!({"input": "x", "error": "invalid digit found in string"}),
    );
    assert_completion(question[1], ok("Info"), &json!({"input": "41"}));
    assert_completion(one(events, "early.no_err"), ok("Info"), &json!({"n": 4}));
    assert_completion(
        one(events, "sync.panics"),
        ("Info", m, Some("panicked")),
        &json!({"n": 1}),
    );
    assert!(with_action(events, "lazy.disabled").is_empty());
}

fn assert_label_failures(events: &[Value]) {
    // `name = ""` fails `action_label`: the event still emits with the default action.
    let event = events
        .iter()
        .find(|event| event["action"] == DEFAULT_ACTION && event["fields"].get("kept").is_some())
        .expect("label failure completion event");
    assert_completion(event, ok("Info"), &json!({"kept": 3}));
}

fn call_and_assert_nested(guard: &LogGuard, path: &Path) {
    assert_eq!(nested_outer(1), 3);
    let events = flushed_events(guard, path);
    let outer = one(&events, "nested.outer");
    let inner = one(&events, "nested.inner");
    assert_completion(outer, ok("Info"), &json!({"n": 1}));
    assert_completion(inner, ok("Info"), &json!({"n": 2}));
    assert!(parent_span_id(outer).is_null(), "a root call has no parent");
    assert_eq!(trace_id(inner), trace_id(outer));
    assert_eq!(parent_span_id(inner), span_id(outer));
    assert_ne!(span_id(inner), span_id(outer));

    let outer_before = by_message(&events, "outer before");
    let outer_bridge = by_message(&events, "outer bridge after");
    let inner_event = by_message(&events, "inner event");
    let inner_bridge = by_message(&events, "inner bridge");
    for event in [outer_before, outer_bridge] {
        assert_eq!(event["trace"], outer["trace"], "outer context: {event}");
    }
    for event in [inner_event, inner_bridge] {
        assert_eq!(event["trace"], inner["trace"], "inner context: {event}");
    }
    // Outside any instrumented call the context is gone again.
    info!(name: "nested.after", "after nested");
    log::info!(target: "nested", "bridge after nested");
    let events = flushed_events(guard, path);
    assert!(by_message(&events, "after nested")["trace"].is_null());
    assert!(by_message(&events, "bridge after nested")["trace"].is_null());
}

const HOP_TASKS: u32 = 32;

fn call_async(runtime: &tokio::runtime::Runtime) {
    // Many tasks hopping across `.await` points on a 4-worker runtime.
    runtime.block_on(async {
        let handles: Vec<_> = (0..HOP_TASKS)
            .map(|task| tokio::spawn(assert_send(async_hop(task))))
            .collect();
        for (task, handle) in (0..HOP_TASKS).zip(handles) {
            assert_eq!(handle.await.unwrap(), task);
        }
    });

    // Deterministic thread hop: first poll here, resume on another OS thread.
    let mut future = Box::pin(assert_send(cross_thread()));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert!(
        sc_observability_log::__private::current_trace().is_none(),
        "the context is not left entered between polls"
    );
    let first_thread = thread_label();
    let resumed_thread = std::thread::spawn(move || block_on_here(future))
        .join()
        .unwrap();
    assert_ne!(first_thread, resumed_thread);

    // Cancelled after the first poll.
    runtime.block_on(async {
        let timed_out = tokio::time::timeout(Duration::from_millis(20), cancelled(60_000)).await;
        assert!(timed_out.is_err());
    });

    // Panic after `.await` in a spawned task.
    let handle = runtime.spawn(async_panics(1));
    let join_error = runtime
        .block_on(handle)
        .expect_err("the async panic propagates to the JoinHandle");
    assert!(join_error.is_panic());

    // Dropped before the first poll: no CallSpan, no event.
    drop(never_polled(1));

    runtime.block_on(async {
        assert_eq!(async_err(false).await.unwrap(), 9);
        assert!(async_err(true).await.is_err());
    });
}

fn assert_async(events: &[Value]) {
    let m = MODULE_TARGET;
    let hops = with_action(events, "async.hop");
    assert_eq!(hops.len(), usize::try_from(HOP_TASKS).unwrap());
    for hop in hops {
        let task = hop["fields"]["task"].clone();
        assert_completion(hop, ok("Info"), &json!({"task": task}));
        assert!(parent_span_id(hop).is_null());
        let for_task = |action: &str| {
            let matching: Vec<_> = with_action(events, action)
                .into_iter()
                .filter(|event| event["fields"]["task"] == task)
                .collect();
            assert_eq!(matching.len(), 1, "{action} for task {task}");
            matching[0]
        };
        let before = for_task("async.hop.before");
        let after = for_task("async.hop.after");
        assert_eq!(before["trace"], hop["trace"], "context before .await");
        assert_eq!(
            after["trace"], hop["trace"],
            "context restored after .await"
        );
        let inner = for_task("async.hop.inner");
        assert_completion(
            inner,
            ok("Info"),
            &json!({"task": task, "return": task.to_string()}),
        );
        assert_eq!(trace_id(inner), trace_id(hop));
        assert_eq!(parent_span_id(inner), span_id(hop));
        let bridge = by_message(events, &format!("hop bridge {task}"));
        assert_eq!(bridge["trace"], inner["trace"], "bridge record context");
    }

    let cross = one(events, "async.cross_thread");
    let before = one(events, "cross.before");
    let after = one(events, "cross.after");
    assert_ne!(before["fields"]["thread"], after["fields"]["thread"]);
    assert_eq!(before["trace"], cross["trace"]);
    assert_eq!(
        after["trace"], cross["trace"],
        "context restored on another thread"
    );
    assert_completion(cross, ok("Info"), &json!({}));

    let cancelled = one(events, "async.cancelled");
    assert_completion(
        cancelled,
        ("Info", m, Some("cancelled")),
        &json!({"sleep_ms": 60_000}),
    );
    assert_eq!(
        one(events, "async.cancelled.started")["trace"],
        cancelled["trace"]
    );
    assert_completion(
        one(events, "async.panics"),
        ("Info", m, Some("panicked")),
        &json!({"n": 1}),
    );
    assert!(with_action(events, "async.never_polled").is_empty());
    let async_err_events = with_action(events, "async.err");
    assert_eq!(async_err_events.len(), 2);
    assert_completion(async_err_events[0], ok("Info"), &json!({}));
    assert_completion(
        async_err_events[1],
        ("Warn", m, Some("error")),
        &json!({"error": "failure 9"}),
    );
}

#[test]
fn instrument_emits_completion_events_with_trace_context() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("instrument-jsonl").unwrap(),
        root.path().into(),
    );
    config.level = LevelFilter::Debug;
    let options = BridgeOptions {
        default_action: ActionName::new(DEFAULT_ACTION).unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let path = guard.active_log_path().unwrap().to_path_buf();

    call_argument_table();
    call_early_exits_and_panics();
    let invalid_before = guard.dropped_events().get(DropCause::InvalidEvent);
    label_failures();
    // `name = ""` (1) plus the empty and reserved `{ KEY }` fields (2).
    assert_eq!(
        guard.dropped_events().get(DropCause::InvalidEvent),
        invalid_before + 3
    );
    let events = flushed_events(&guard, &path);
    assert_argument_table(&events);
    assert_early_exits_and_panics(&events);
    assert_label_failures(&events);

    call_and_assert_nested(&guard, &path);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    call_async(&runtime);
    drop(runtime);
    let events = flushed_events(&guard, &path);
    assert_async(&events);

    // Only the three counted label failures were dropped.
    assert_eq!(guard.dropped_events().total(), 3);
    guard.shutdown(Duration::from_secs(5)).unwrap();
}
