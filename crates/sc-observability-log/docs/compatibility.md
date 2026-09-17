# tracing / log API compatibility

`sc-observability-log` matches the call syntax of the commonly used logging
crates, so migrating is a dependency change plus an import rename.

- **`log` 0.4:** `log::{trace,debug,info,warn,error}!` call sites, including
  `target:` and `key = value; "msg"`, keep working unchanged through the bridge
  installed by `sc_observability_log::init` (see `mapping.md`).
- **`tracing` 0.1 events:** this document, section "Events" (sprint a-2).
- **`tracing::instrument`:** this document, section "`#[instrument]`" (sprint a-3).

## Events

`sc_observability_log::{trace, debug, info, warn, error, event}` and
`sc_observability_log::Level` accept the `tracing` 0.1 event syntax (verified
against `tracing` 0.1.44). Each call builds a structured `LogEvent`: fields are
JSON values, not formatted text.

### Migration how-to

Change the import:

```rust
// before: use tracing::{info, warn, event, Level};
use sc_observability_log::{info, warn, event, Level};
```

Nothing else changes for any form in the grammar table. The shared fixture
`tests/compat/events.rs` proves it: the same source compiles against `tracing`
0.1 and against `sc_observability_log`, and inside
`sc-observability-log-consumer-check`, whose only dependency is
`sc-observability-log`.

### Grammar table (supported forms)

| Form | Mapping |
|---|---|
| `info!("fmt {}", a)` | `message = format!(..)`; `target = module_path!()`, sanitized at runtime; `action = None` (the bridge default action) |
| `info!(target: "t", ...)`, `info!(target: TGT, ...)`, `info!(target: module_path!(), ...)`, `info!(target: concat!("a", ".b"), ...)` | `target` = the expression's value, stored in the `static` `Callsite` and sanitized at runtime on first use. The expression must be a constant `&'static str`, as in tracing (a non-constant value fails with rustc E0015 in both). |
| `info!(name: "n", ...)`, `info!(name: NAME, ...)`, `info!(name: concat!(..), ...)` | `action` = the value, sanitized at runtime on first use, same rules as `target:` |
| `info!(name: "n", target: "t", ...)` | both; **`name:` must precede `target:`**, as in tracing |
| `info!(k = v, ...)` / `info!(a.b = v, ...)` | `fields["k"]` / `fields["a.b"]` from the bare-field dispatch (Serialize JSON, else Debug string) |
| `info!("literal key" = v, ...)` | `fields["literal_key"]`, same dispatch |
| `info!(r#type = v, ...)` | `fields["type"]`: the `r#` prefix is stripped, as tracing does |
| `info!({ KEY } = v, ...)` / `info!({ KEY } = ?v, ...)` / `info!({ KEY } = %v, ...)` | `fields[field_key_label(KEY)]` with the bare/`?`/`%` rule of the other rows; an empty or reserved key is omitted and counted (see "Runtime labels and keys"). `KEY` must be a constant `&'static str`, as in tracing |
| `info!(?v)` / `info!(k = ?v)` | `fields["v"\|"k"] = format!("{:?}", v)` |
| `info!(%v)` / `info!(k = %v)` | `fields["v"\|"k"] = format!("{}", v)` |
| `info!(v)` / `info!(a.b)` (shorthand, dotted allowed) | `fields["v"\|"a.b"]` as for `k = v` |
| brace field set: `info!({ k = 1, ?v }, "fmt {}", a)`, `info!({ k = 1 })`, `info!(name: "n", { k = 1 }, "m")`, `info!(target: "t", { k = 1 }, "m")`, `info!(name: "n", target: "t", { k = 1 }, "m")`, `event!(Level::INFO, { k = 1 }, "m")` | the braces are unwrapped and their contents parsed as the field list, with every field row above; a brace group followed by `=` is a `{ KEY } = v` key instead |
| fields followed by a message: `info!(k = v, "fmt {}", a)` | fields plus message |
| `event!(Level::INFO, ...)` / `event!(target: "t", Level::WARN, ...)` / `event!(name: "n", Level::INFO, ...)` / `event!(name: "n", target: "t", Level::ERROR, ...)` / `event!(LVL, ...)` with `const LVL: Level` | level from the `Level` expression, converted with `From<Level> for sc_observability_types::Level` |

A string literal not followed by `=`, a macro call (`concat!(..)`), or any other
token that cannot start a field begins the format message, as in tracing.

**Laziness.** Every form expands inside an `if` on the configured level
(`LoggerConfig.level`). When the level is disabled, no field value or message
argument is evaluated, formatted or allocated.

**`Level`.** `sc_observability_log::Level` has the associated consts `TRACE`,
`DEBUG`, `INFO`, `WARN` and `ERROR`. It deliberately has no `PartialOrd`/`Ord`:
tracing orders levels by verbosity, which would surprise here.

### Rejected forms

#### Valid in tracing, deliberately rejected

Each fails to compile with the message shown; the trybuild case under
`tests/ui/` checks in the expected stderr.

| Form | trybuild case | Message |
|---|---|---|
| `parent: ...` | `ui/event_parent.rs` | `` `parent:` is not supported (no span parents) `` |
| deferred field `k = tracing::field::Empty` (any value path ending in `field::Empty`) | `ui/event_field_empty.rs` | `` deferred fields (`field::Empty`) are not supported `` |
| empty string-literal key `"" = v` | `ui/event_empty_key.rs` | `` empty field key is not supported `` |
| reserved string key `"sc_observability_log.x" = v` (also inside braces) | `ui/event_reserved_key_string.rs` | `` field keys starting with `sc_observability_log.` are reserved `` |
| reserved dotted key `sc_observability_log.x = v` (also inside braces) | `ui/event_reserved_key_dotted.rs` | `` field keys starting with `sc_observability_log.` are reserved `` |
| span macros (`span!`, `trace_span!` … `error_span!`; not exported) | `ui/event_span_macro.rs` | rustc's unresolved-path error naming `info_span` |

Keep span parents, deferred fields and spans on `tracing`, or use
`#[instrument]` (sprint a-3).

#### Not valid in tracing either

| Form | trybuild case | Message |
|---|---|---|
| `target: .., name: ..` | `ui/event_target_before_name.rs` | `` `name:` must come before `target:` `` |
| a bare field or shorthand whose value implements neither `Serialize` nor `Debug` | `ui/field_not_serialize_or_debug.rs` | `` field value `T` implements neither `serde::Serialize` nor `core::fmt::Debug` `` (the `FieldDebug` `on_unimplemented` message, pointing at the field expression) |

Other malformed input fails with a `syn` parse error such as
`` expected `=` after field key ``.

### Reserved keys

Field keys starting with `sc_observability_log.` are reserved for the crate
itself (for example `sc_observability_log.serialize_errors`). Literal
(`"sc_observability_log.x" = v`) and dotted (`sc_observability_log.x = v`) keys
with that prefix, and the empty literal key, are rejected at compile time.
Literal and dotted keys are otherwise stored in their canonical sanitized form
(after removing `r#`). The same prefix is reserved at runtime for every field
key, including `{ KEY } = v` keys, `log` key-values and
`LogControl::try_log` fields (`mapping.md`, "Field keys").

### Runtime labels and keys

The proc-macro crate never sanitizes or validates labels. The single sanitizer
is `mapping.rs` (`::` becomes `.`, every char outside `[A-Za-z0-9._-]` becomes
`_`), applied at runtime:

- **Per-call-site cache.** Each expansion declares one `static` `Callsite`
  holding the unsanitized `target` (default `module_path!()`) and `name`. The
  first enabled event at that call site runs them through `target_label` and
  `action_label` and caches both results; later events clone the cached labels.
- **`name` fails `action_label`** (empty after sanitizing, or rejected by
  `ActionName::new`): the event **still emits** with the bridge default action,
  and each such event is counted as `DropCause::InvalidEvent`.
- **target fails `target_label`** (unreachable: an empty target becomes `log`):
  the event is **not emitted** and is counted as `DropCause::InvalidEvent`.
- **`{ KEY } = v`.** Each such field gets its own `static` `DynamicKey`; the
  first use runs `KEY` through `field_key_label` and caches the result. A key
  that is empty after sanitizing, or starts with `sc_observability_log.` after
  sanitizing (`sc_observability_log::y` included), omits **only that field**; the
  event still emits and each omission is counted as `DropCause::InvalidEvent`.

Example: `info!(name: "bad name", target: "bad target::x", "m")` records
`target = "bad_target.x"` and `action = "bad_name"` and counts nothing.

### Serialization failures

A bare field whose `Serialize` impl fails records `null` under its key and the
error text under `fields["sc_observability_log.serialize_errors"][key]`.
Non-finite floats are not failures: they record `null`. See
`sc-observability-log-macros/docs/field-value-dispatch.md`.

### Field value size

Field values are inserted into events as produced by `serde_json::to_value`
(bare and `{ key } =` fields), or as the `Debug` (`?v`) or `Display` (`%v`)
string. There is no size or depth cap on individual field values or on the
formatted message. The a-1 writer queue is bounded by event count
(`queue_capacity`), not by bytes, so callers must not log unbounded collections
or untrusted large payloads as field values without truncating them first.

### Evaluation cost

When the event's level passes the `LoggerConfig.level` threshold, field values
and the message are serialized or formatted synchronously on the calling thread
inside the macro expansion, as with `tracing`. When the level is disabled, the
field and message expressions are not evaluated. In async code, keep field
`Serialize`, `Debug` and `Display` implementations cheap: the work runs on the
executor thread that called the macro.

## `#[instrument]`

`#[sc_observability_log::instrument]` accepts the `tracing::instrument`
arguments below with the same meanings (verified against `tracing-attributes`
0.1.31). It works on sync and `async` free functions and methods, including
`self`, `&self` and `&mut self` receivers, and keeps the signature, visibility,
generics, `where` clause, attributes and `async`-ness unchanged. Instead of a
span, each call emits exactly one **completion** `LogEvent` carrying the
duration and the outcome, and every event emitted inside the call carries the
call's `TraceContext`.

### Migration how-to

```rust
// before: use tracing::{info, instrument};
use sc_observability_log::{info, instrument};

const ISSUES: &str = "btit.issues";

#[instrument(name = "bd_update", target = ISSUES, skip(payload), fields(project = %cwd), err(level = "warn"))]
async fn bd_update(id: String, cwd: String, payload: UpdatePayload) -> Result<Issue, String> {
    info!(name: "bd_update", id = %id, "updating"); // carries this call's TraceContext
    run(&id, &cwd, payload).await
}
```

Nothing else changes for any form in the argument table. The shared fixture
`tests/compat/instrument.rs` proves it: the same source compiles against
`tracing::instrument`, against `sc_observability_log::instrument` (where
`tests/compat_instrument.rs` asserts its JSONL output), and inside
`sc-observability-log-consumer-check`, whose only dependency is
`sc-observability-log`.

### Argument table (supported forms)

| Argument | Behavior |
|---|---|
| `name = "n"`, `name = NAME` (a `const &'static str`), or a positional string literal `#[instrument("n")]` | `action` = the value (default: the fn name), stored unchanged in the `static` `Callsite` and labelled at runtime by `action_label` on first use |
| `target = "t"`, `target = TGT` | `target` = the value (default: `module_path!()`), labelled at runtime by `target_label` through the same `Callsite` cache |
| `level = "info"` (case-insensitive), `level = Level::INFO`, `level = LVL` (a `const` of type `sc_observability_log::Level`), or `level = 1..=5` (1 = trace … 5 = error) | level of the completion event (default `INFO`) |
| `skip(a, b)` | the listed arguments (including `self`) are not recorded; naming a parameter that does not exist is a compile error, as in tracing |
| `skip_all` | no arguments are recorded |
| `fields(k = v, ?x, %y, a.b = 1, { C } = 1, { C } = ?v, r#type = 1)` | extra fields, parsed and recorded exactly like the event-macro field rows ("Grammar table" above): bare values use the Serialize-else-Debug dispatch, `?`/`%` record Debug/Display strings, `{ C } = v` is labelled at runtime by `field_key_label` (an empty or reserved key omits only that field and is counted), `r#type` records `"type"`. A field whose single-segment key equals a parameter name replaces that parameter's recording, as in tracing |
| *(default)* | every non-skipped typed argument is recorded as `fields["arg"]` with the bare-field dispatch; identifiers bound by tuple, struct and reference patterns are recorded individually. The receiver (`self`, `&self`, `&mut self`) **is** recorded as `fields["self"]` with Debug, as tracing does (a non-`Debug` receiver gets the `FieldDebug` diagnostic); `skip(self)` removes it |
| `ret` / `ret(Debug)` / `ret(Display)` | `fields["return"]`; bare `ret` formats with **Debug**. With `err` present it records the `Ok` value; without `err` it records the whole return value |
| `ret(level = L)`, `ret(Debug, level = L)`, `ret(Display, level = L)` | as `ret`; the completion level is `L` when the outcome is `ok` |
| `err` / `err(Debug)` / `err(Display)` | when the fn returns `Err`: outcome `error`, completion `level = ERROR` and `fields["error"]`; bare `err` formats with **Display** |
| `err(level = L)`, `err(Debug, level = L)`, `err(Display, level = L)` | as `err`; the completion level is `L` instead of `ERROR` |

`L` accepts the same forms as `level = ..`.

**Completion level precedence:** `ok` → the `ret(level = ..)` level if given,
else `level`; `error` → the `err(level = ..)` level if given, else `ERROR`;
`panicked` / `cancelled` → `level`.

**Static-type dispatch.** As for event macros, the Serialize-or-Debug choice is
made from the argument's static type: in `fn f<T: Debug>(t: T)` a value whose
concrete type also implements `Serialize` records its **Debug string**; bound
`T: Debug + Serialize` to record JSON.

**Laziness.** Arguments and `fields(..)` are recorded only when one of the
completion levels (`level`, the `ok` level, and the `err` level when `err` is
present) is enabled when the call starts; `ret`/`err` values are formatted only
when the completion event for that outcome is enabled. A call whose completion
level is disabled still creates its trace context, so events inside it keep
their `trace`. The event-macro "Field value size" and "Evaluation cost" rules
above apply equally to recorded arguments, `fields(..)` values and `ret`/`err`
values: they are recorded synchronously on the calling thread (for an async fn,
the executor thread polling it) with no size cap.

### Completion event

| Field | Value |
|---|---|
| `action` | `name`, labelled and cached by the `Callsite` |
| `target` | `target`, labelled and cached by the `Callsite` |
| `level` | by the completion level precedence above |
| `message` | `None` |
| `outcome` | `ok` / `error` (`err` present and `Err` returned) / `panicked` (sync or async unwind) / `cancelled` (async future dropped after its first poll, before completion) |
| `fields` | recorded arguments (including `self` unless skipped), plus `fields(..)`, plus `duration_ms` (u64 milliseconds, saturating), plus `return` / `error` when applicable |
| `trace` | this call's `TraceContext` (its own `span_id`) |

**Completion keys shadow same-named user fields.** `duration_ms`, `return` and
`error` are reserved completion keys, always inserted last and always
authoritative. If a recorded argument or a `fields(..)` entry already occupies
one of these keys (for example a parameter named `duration_ms`, or `err` on a
function with a parameter named `error`), the completion value replaces it
rather than compiling to an error: `error` is a common parameter name and
rejecting it would break tracing compatibility. The displaced user value is
not lost — it is moved to
`fields["sc_observability_log.shadowed_fields"][key]`, a JSON object keyed by
the original field name, mirroring how a serialize failure is recorded under
`sc_observability_log.serialize_errors` (see "Serialization failures" above).
A call with no such collision has no `shadowed_fields` key at all. The `log`
bridge applies the same rule to its `code.module` / `code.file` / `code.line`
keys; `mapping.md`, "Field keys", lists the unified rules for every producer.

### Outcomes

- **`ok` / `error`.** A sync body runs as `(move || body)()`, so `return` and
  `?` leave only the closure: an early `return Ok(..)` or `?` on the `Ok` path
  is `ok`, an early `return Err(..)` or `?` on an `Err` is `error` (with `err`),
  and a fn without `err` that returns early is `ok`. An async body is an
  `async move` block driven in place, with the same rules.
- **`panicked`.** The instrumented function's own panic is **never caught and
  never re-raised**: it unwinds through the call's guards, which only record
  `panicked` and emit the completion event while the panic propagates. For an
  async fn that panics inside `poll`, the flag is set during unwinding and the
  event is emitted when the runtime drops the future.
- **`cancelled`.** An async future dropped **after its first poll** and before
  completion (for example by `tokio::time::timeout`) emits `cancelled`.
- **Never polled.** The call context is created inside the async body, on the
  first poll. A future dropped before its first poll has not run its body, so
  it creates no context and emits **nothing**. This is not an outcome.

### Context rules

- Each call gets a new `span_id`. Its `trace_id` and `parent_span_id` are the
  `trace_id` and `span_id` of the innermost context entered on the calling
  thread when the call starts (the first poll for an async fn); with none, the
  call starts a new trace (`parent_span_id = None`).
- The context is entered for the sync body, and for **each poll** of an async
  body, so it is restored after every `.await`, on whichever thread resumes the
  future. It is never left entered between polls.
- Every event emitted while the context is entered carries it: event macros,
  `log` bridge records and the completion event itself, whose `trace.span_id`
  is the call's own `span_id`. Outside any instrumented call, `trace` is `None`.
- Ids are generated with std only: `trace_id` is 32 and `span_id` 16 lowercase
  hex digits, never all zero, validated by `TraceId::new` / `SpanId::new`. A
  validation failure (unreachable) records `trace = None` instead of panicking.
- The per-thread stack is reached only through `LocalKey::try_with` and
  `RefCell::try_borrow`/`try_borrow_mut`. During thread teardown, or if the
  stack is already borrowed, entering does nothing and the current context
  reads as `None`.
- The entry guard is `!Send`: holding it across `.await` makes the future
  `!Send` (`ui/instrument_entered_across_await.rs`). The generated async code
  never does, so an instrumented `async fn` whose body is `Send` still returns
  a `Send` future.

### Runtime labels

Label failures follow the event-macro rules ("Runtime labels and keys"):

- a `name` failing `action_label`: the completion event **still emits** with
  the bridge default action, counted as `DropCause::InvalidEvent`;
- a target failing `target_label` (unreachable): the completion event is not
  emitted, counted;
- a `fields({ C } = v)` key failing `field_key_label`: only that field is
  omitted, counted.

A `CallOutcome` label failing `OutcomeLabel::new` (unreachable; a unit test
proves all four validate) emits the completion event with `outcome = None` and
counts `DropCause::InvalidEvent`.

### Rejected `#[instrument]` forms

#### Valid in tracing, deliberately rejected

Each fails to compile with the message shown; the trybuild case under
`tests/ui/` checks in the expected stderr.

| Form | trybuild case | Message |
|---|---|---|
| `parent = ..` | `ui/instrument_parent.rs` | `` `parent` is not supported (no span parents) `` |
| `follows_from = ..` | `ui/instrument_follows_from.rs` | `` `follows_from` is not supported (no span links) `` |
| deferred field without a value: `fields(x)`, `fields(a.b)` | `ui/instrument_deferred_field.rs` | `` deferred field `x` without a value is not supported (tracing `field::Empty`) `` |
| deferred field `fields(x = tracing::field::Empty)` | `ui/instrument_field_empty.rs` | `` deferred fields (`field::Empty`) are not supported `` |
| reserved dotted key `fields(sc_observability_log.x = 1)` | `ui/instrument_reserved_key.rs` | `` field keys starting with `sc_observability_log.` are reserved `` |

Keep span parents, span links and deferred fields on `tracing`.

#### Not valid in tracing either

| Form | trybuild case | Message |
|---|---|---|
| string-literal key `fields("k" = 1)` (covers `""` and reserved strings) | `ui/instrument_literal_key.rs` | `` string-literal field keys are not supported in `#[instrument(fields(..))]`; use an identifier `` |
| applied to a non-fn item | `ui/instrument_non_fn.rs` | `` `#[instrument]` can only be applied to functions `` |

An unknown argument fails with `` unknown `#[instrument]` argument `x` ``
(tracing-attributes only warns), and other malformed arguments fail with the
same messages as tracing-attributes (for example
`` expected only a single `level` argument ``).
