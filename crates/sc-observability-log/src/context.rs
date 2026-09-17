//! Call context for `#[instrument]`: trace ids, the per-thread context stack and
//! the completion event.
//!
//! Each instrumented call owns one [`CallSpan`] (`Send + Sync`). The span
//! records a `TraceContext`: a new `span_id`, and the `trace_id` and parent
//! `span_id` of the innermost context entered on the calling thread, or a new
//! `trace_id` when there is none. [`CallSpan::enter`] pushes that context onto a
//! thread-local stack for one synchronous section or one `poll`, and the
//! returned [`Entered`] guard (`!Send + !Sync`) pops only its own entry.
//! `__private::emit` reads [`current_trace`] so every event emitted inside the
//! call, from the event macros, the `log` bridge or the completion event, carries
//! the call's context.
//!
//! # Guarantees
//!
//! - The stack is reached only through `LocalKey::try_with` and
//!   `RefCell::try_borrow` / `try_borrow_mut`. When the thread-local is gone
//!   (thread teardown) or already borrowed, `enter` pushes nothing and
//!   [`current_trace`] returns `None`. No borrow is held across `emit` or user code.
//! - Each call emits exactly one completion event: `finish_ok` / `finish_err`, or
//!   `Drop for CallSpan` with `panicked` or `cancelled` when neither ran. A user
//!   panic is never caught: the guards only record it while it unwinds.
//! - Ids use std only (`RandomState` hashing of a process-wide counter) and are
//!   validated with `TraceId::new` / `SpanId::new`; a failure yields `trace = None`.

use std::cell::RefCell;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::marker::PhantomData;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use sc_observability_types::{OutcomeLabel, SpanId, TraceContext, TraceId};

use crate::__private::{FieldRecord, Level, Map, Value, emit, enabled, record_drop, record_field};
use crate::DropCause;
use crate::callsite::{Callsite, callsite_parts};

/// Outcome of an instrumented call, recorded as the completion event's `outcome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    /// The call returned; with `err`, it returned `Ok`.
    Ok,
    /// With `err`, the call returned `Err`.
    Error,
    /// The call unwound (sync panic, or an async panic inside `poll`).
    Panicked,
    /// The async call was dropped after its first poll and before completion.
    Cancelled,
}

impl CallOutcome {
    /// The `LogEvent.outcome` label: `ok`, `error`, `panicked` or `cancelled`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Panicked => "panicked",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Validated outcome labels in `CallOutcome` declaration order.
static OUTCOME_LABELS: OnceLock<[Option<OutcomeLabel>; 4]> = OnceLock::new();

/// Converts the four labels once; the unit test asserts every entry is `Some`.
fn outcome_labels() -> &'static [Option<OutcomeLabel>; 4] {
    OUTCOME_LABELS.get_or_init(|| {
        [
            CallOutcome::Ok,
            CallOutcome::Error,
            CallOutcome::Panicked,
            CallOutcome::Cancelled,
        ]
        .map(|outcome| OutcomeLabel::new(outcome.label()).ok())
    })
}

/// Never panics: a label that failed validation is counted and becomes `None`.
fn outcome_label(outcome: CallOutcome) -> Option<OutcomeLabel> {
    let [ok, error, panicked, cancelled] = outcome_labels();
    let label = match outcome {
        CallOutcome::Ok => ok,
        CallOutcome::Error => error,
        CallOutcome::Panicked => panicked,
        CallOutcome::Cancelled => cancelled,
    };
    if label.is_none() {
        record_drop(DropCause::InvalidEvent);
    }
    label.clone()
}

#[derive(Debug)]
struct StackEntry {
    /// The owning `CallSpan::key`.
    key: u64,
    ctx: TraceContext,
}

thread_local! {
    static STACK: RefCell<Vec<StackEntry>> = const { RefCell::new(Vec::new()) };
}

/// Source of `CallSpan::key`, unique per span in the process.
static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

/// Mixed into every generated id so two ids never hash the same input.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Innermost entered context on this thread; `None` if none, or if the thread-local is unavailable.
#[must_use]
pub fn current_trace() -> Option<TraceContext> {
    STACK
        .try_with(|stack| {
            stack
                .try_borrow()
                .ok()
                .and_then(|entries| entries.last().map(|entry| entry.ctx.clone()))
        })
        .ok()
        .flatten()
}

/// 64 pseudo-random bits from std's randomly keyed `SipHash` over a counter.
fn random_u64() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(ID_COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.finish()
}

/// W3C trace context treats all-zero ids as invalid, so zero is replaced by one.
fn non_zero(value: u64) -> u64 {
    if value == 0 { 1 } else { value }
}

/// A new 32-hex-digit trace id; `None` only if validation rejected it.
fn new_trace_id() -> Option<TraceId> {
    TraceId::new(format!(
        "{:016x}{:016x}",
        random_u64(),
        non_zero(random_u64())
    ))
    .ok()
}

/// A new 16-hex-digit span id; `None` only if validation rejected it.
fn new_span_id() -> Option<SpanId> {
    SpanId::new(format!("{:016x}", non_zero(random_u64()))).ok()
}

/// The context of a new span below `parent` (or at the root of a new trace).
fn child_context(parent: Option<TraceContext>) -> Option<TraceContext> {
    let span_id = new_span_id()?;
    match parent {
        Some(parent) => Some(TraceContext {
            trace_id: parent.trace_id,
            span_id,
            parent_span_id: Some(parent.span_id),
        }),
        None => Some(TraceContext {
            trace_id: new_trace_id()?,
            span_id,
            parent_span_id: None,
        }),
    }
}

/// Levels of the completion event, resolved by the `#[instrument]` expansion.
#[derive(Debug, Clone, Copy)]
pub struct CallLevels {
    /// `level` (default `INFO`): used for `panicked` and `cancelled`.
    pub level: Level,
    /// `ret(level = ..)` if given, else `level`: used for `ok`.
    pub ok: Level,
    /// `err(level = ..)` if given, else `ERROR`; equals `level` without `err`.
    pub error: Level,
}

impl CallLevels {
    fn for_outcome(self, outcome: CallOutcome) -> Level {
        match outcome {
            CallOutcome::Ok => self.ok,
            CallOutcome::Error => self.error,
            CallOutcome::Panicked | CallOutcome::Cancelled => self.level,
        }
    }

    fn any_enabled(self) -> bool {
        enabled(self.level) || enabled(self.ok) || enabled(self.error)
    }
}

/// One instrumented call: its trace context, recorded fields and start time.
///
/// `Send + Sync`: owned by the instrumented call frame or its async body.
#[derive(Debug)]
pub struct CallSpan {
    callsite: &'static Callsite,
    levels: CallLevels,
    key: u64,
    trace: Option<TraceContext>,
    fields: Map<String, Value>,
    started: Instant,
    panicked: AtomicBool,
    finished: bool,
}

impl CallSpan {
    /// Starts a span below the current context, or at the root of a new trace.
    ///
    /// `fields` runs once, only when a completion level is enabled; otherwise
    /// the span records no fields.
    pub fn new(
        callsite: &'static Callsite,
        levels: CallLevels,
        fields: impl FnOnce() -> Map<String, Value>,
    ) -> CallSpan {
        let fields = if levels.any_enabled() {
            fields()
        } else {
            Map::new()
        };
        CallSpan {
            callsite,
            levels,
            key: NEXT_KEY.fetch_add(1, Ordering::Relaxed),
            trace: child_context(current_trace()),
            fields,
            started: Instant::now(),
            panicked: AtomicBool::new(false),
            finished: false,
        }
    }

    /// `true` when the completion event for `outcome` would be emitted at the current level.
    #[must_use]
    pub fn enabled(&self, outcome: CallOutcome) -> bool {
        enabled(self.levels.for_outcome(outcome))
    }

    /// Pushes this span's context; the guard pops only its own entry.
    ///
    /// No-op when the span has no context or the thread-local is unavailable.
    /// The borrow ends before this returns.
    pub fn enter(&self) -> Entered<'_> {
        let pushed = match &self.trace {
            Some(ctx) => STACK
                .try_with(|stack| match stack.try_borrow_mut() {
                    Ok(mut entries) => {
                        entries.push(StackEntry {
                            key: self.key,
                            ctx: ctx.clone(),
                        });
                        true
                    }
                    Err(_) => false,
                })
                .unwrap_or(false),
            None => false,
        };
        Entered {
            span: self,
            pushed,
            _not_send: PhantomData,
        }
    }

    /// Completes the call with `ok`; `ret` is recorded as `fields["return"]`.
    pub fn finish_ok(mut self, ret: Option<FieldRecord>) {
        self.complete(CallOutcome::Ok, ret);
    }

    /// Completes the call with `error`; `error` is recorded as `fields["error"]`.
    pub fn finish_err(mut self, error: FieldRecord) {
        self.complete(CallOutcome::Error, Some(error));
    }

    /// Emits the completion event once, inside this call's context; never panics.
    ///
    /// The reserved completion keys `duration_ms` and `return`/`error` always
    /// win over a same-named recorded argument or `fields(..)` entry; see
    /// `sc_observability_log.shadowed_fields`.
    fn complete(&mut self, outcome: CallOutcome, value: Option<FieldRecord>) {
        self.finished = true;
        let level = self.levels.for_outcome(outcome);
        if !enabled(level) {
            return;
        }
        let mut fields = std::mem::take(&mut self.fields);
        let duration_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        crate::mapping::insert_authoritative(&mut fields, "duration_ms", Value::from(duration_ms));
        if let Some(record) = value {
            let key = match outcome {
                CallOutcome::Error => "error",
                CallOutcome::Ok | CallOutcome::Panicked | CallOutcome::Cancelled => "return",
            };
            match record {
                FieldRecord::Value(v) => crate::mapping::insert_authoritative(&mut fields, key, v),
                // Unreachable from the `#[instrument]` expansion: `ret`/`err`
                // values always format through `debug_value`/`display_value`
                // (`FieldRecord::Value`). Kept for `FieldRecord` completeness;
                // falls back to the pre-existing serialize-failure bookkeeping
                // (`sc_observability_log.serialize_errors`) without shadow
                // protection, since a value that failed to serialize was never
                // an authoritative completion value to protect against loss.
                FieldRecord::SerializeFailed(_) => record_field(&mut fields, key, record),
            }
        }
        let outcome = outcome_label(outcome);
        let Some(parts) = callsite_parts(self.callsite, level, None, outcome, fields) else {
            return;
        };
        // Re-enter: `emit` attaches this call's TraceContext. No borrow is held here.
        let _entered = self.enter();
        emit(parts);
    }
}

impl Drop for CallSpan {
    /// Emits `panicked` (flag set or thread panicking) or `cancelled` when no `finish_*` ran.
    fn drop(&mut self) {
        if !self.finished {
            let outcome = if self.panicked.load(Ordering::Relaxed) || std::thread::panicking() {
                CallOutcome::Panicked
            } else {
                CallOutcome::Cancelled
            };
            self.complete(outcome, None);
        }
    }
}

/// Guard of one entered section; `!Send + !Sync`, valid for one synchronous section or one poll.
#[derive(Debug)]
pub struct Entered<'a> {
    span: &'a CallSpan,
    pushed: bool,
    _not_send: PhantomData<*const ()>,
}

impl Drop for Entered<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.span.panicked.store(true, Ordering::Relaxed);
        }
        if self.pushed {
            let key = self.span.key;
            let _ = STACK.try_with(|stack| {
                if let Ok(mut entries) = stack.try_borrow_mut()
                    && let Some(pos) = entries.iter().rposition(|entry| entry.key == key)
                {
                    entries.remove(pos);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    // These tests never install the logger, so every level is disabled: dropping a
    // `CallSpan` returns before `outcome_label`, `callsite_parts` or `emit`, and no
    // path reaches `record_drop` (the a-1 counter-test rule).
    static CALLSITE: Callsite = Callsite::new("context.tests", Some("unit"));

    const LEVELS: CallLevels = CallLevels {
        level: Level::Info,
        ok: Level::Info,
        error: Level::Error,
    };

    fn stack_len() -> usize {
        STACK
            .try_with(|stack| stack.try_borrow().map(|entries| entries.len()))
            .unwrap()
            .unwrap()
    }

    #[test]
    fn all_outcome_labels_validate() {
        let labels = outcome_labels();
        let outcomes = [
            CallOutcome::Ok,
            CallOutcome::Error,
            CallOutcome::Panicked,
            CallOutcome::Cancelled,
        ];
        for (label, outcome) in labels.iter().zip(outcomes) {
            assert_eq!(
                label.as_ref().map(OutcomeLabel::as_str),
                Some(outcome.label())
            );
        }
    }

    #[test]
    fn generated_ids_always_validate() {
        let mut spans = HashSet::new();
        for _ in 0..10_000 {
            let trace = new_trace_id().expect("trace id validates");
            let span = new_span_id().expect("span id validates");
            assert_eq!(trace.as_str().len(), 32);
            assert_eq!(span.as_str().len(), 16);
            assert!(
                TraceId::new(trace.as_str()).is_ok() && SpanId::new(span.as_str()).is_ok(),
                "round trip"
            );
            spans.insert(span);
        }
        assert_eq!(spans.len(), 10_000, "span ids are unique");
    }

    #[test]
    fn current_trace_is_none_on_a_fresh_thread() {
        let seen = std::thread::spawn(current_trace).join().unwrap();
        assert!(seen.is_none());
    }

    #[test]
    fn nested_spans_link_and_the_stack_unwinds() {
        std::thread::spawn(|| {
            let outer = CallSpan::new(&CALLSITE, LEVELS, Map::new);
            let outer_ctx = outer.trace.clone().unwrap();
            assert!(outer_ctx.parent_span_id.is_none());
            {
                let _outer_entered = outer.enter();
                assert_eq!(current_trace(), Some(outer_ctx.clone()));
                let inner = CallSpan::new(&CALLSITE, LEVELS, Map::new);
                let inner_ctx = inner.trace.clone().unwrap();
                assert_eq!(inner_ctx.trace_id, outer_ctx.trace_id);
                assert_eq!(inner_ctx.parent_span_id, Some(outer_ctx.span_id.clone()));
                assert_ne!(inner_ctx.span_id, outer_ctx.span_id);
                {
                    let _inner_entered = inner.enter();
                    assert_eq!(current_trace(), Some(inner_ctx));
                    assert_eq!(stack_len(), 2);
                }
                assert_eq!(current_trace(), Some(outer_ctx));
            }
            assert_eq!(stack_len(), 0);
            assert!(current_trace().is_none());
        })
        .join()
        .unwrap();
    }

    #[test]
    fn entered_pops_only_its_own_entry() {
        std::thread::spawn(|| {
            let first = CallSpan::new(&CALLSITE, LEVELS, Map::new);
            let second = CallSpan::new(&CALLSITE, LEVELS, Map::new);
            let first_entered = first.enter();
            let second_entered = second.enter();
            drop(first_entered); // out of order
            assert_eq!(current_trace(), second.trace.clone());
            drop(second_entered);
            assert_eq!(stack_len(), 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn fields_are_not_recorded_when_every_level_is_disabled() {
        let span = CallSpan::new(&CALLSITE, LEVELS, || {
            let mut fields = Map::new();
            fields.insert("never".to_owned(), Value::from(1));
            fields
        });
        assert!(span.fields.is_empty());
        assert!(!span.enabled(CallOutcome::Ok));
    }

    #[test]
    fn entered_records_a_panic_while_unwinding() {
        let span = CallSpan::new(&CALLSITE, LEVELS, Map::new);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _entered = span.enter();
            panic!("instrumented body panic (expected by this test)");
        }));
        assert!(result.is_err());
        assert!(span.panicked.load(Ordering::Relaxed));
        assert!(current_trace().is_none());
    }

    #[test]
    fn call_span_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CallSpan>();
    }
}
