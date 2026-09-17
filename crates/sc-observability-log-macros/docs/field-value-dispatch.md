# Field value dispatch (design record)

How a bare event field (`k = v`, shorthand `v`, `a.b`) is recorded as a JSON
value. `?v` and `%v` always record the `Debug` / `Display` string
(`__private::debug_value` / `__private::display_value`). Sprint a-2; the code
lives in `crates/sc-observability-log/src/callsite.rs`, and the expansion in
`crates/sc-observability-log-macros/src/event.rs`.

## Kind selection by autoref

A bare field `k = v` / shorthand `v` expands to

```rust
{ let __v = &v; (&&FieldValue(__v)).__sc_field_kind().record(__v) }
```

with `SerializeKindTag` and `DebugKindTag` in scope. `SerializeKindTag` is
implemented for `&FieldValue<'_, T>` only `where T: Serialize` and resolves at
the first probe step; `DebugKindTag` is implemented for `FieldValue<'_, T>` for
**every** `T` and resolves one autoderef later. A type implementing both takes
the Serialize path.

## Static-type dispatch

The kind is chosen at the expansion site from the expression's *static* type.
In generic code bounded only by `T: Debug`, `T: Serialize` is not provable
there, so a value whose concrete type also implements `Serialize` records its
**Debug string**, not JSON. Add `Serialize` to the generic bound to get JSON.
Verified while planning: `fn generic_debug<T: Debug>(v: T)` called with a
`Serialize + Debug` struct records `"Both { a: 4 }"`, while
`fn generic_both<T: Debug + Serialize>(v: T)` records `{"a":3}`. a-3's
generic-method fixture covers this.

## Bound-checked call

`#[diagnostic::on_unimplemented]` (stable since 1.78) only fires on an unmet
trait bound, not on rustc's "no method found". The fallback kind therefore has
no bound at selection time; the bound is checked by the *call*
`DebugKind::record<T: ?Sized + FieldDebug>(self, v: &T)`, where `FieldDebug`
carries the diagnostic and has the blanket impl
`impl<T: ?Sized + Debug> FieldDebug for T`. The proc-macro emits that call with
`quote_spanned!` on the field expression, so the error points at the field
(checked in by `tests/ui/field_not_serialize_or_debug.stderr`):

```text
error[E0277]: field value `Neither` implements neither `serde::Serialize` nor `core::fmt::Debug`
 --> tests/ui/field_not_serialize_or_debug.rs:8:19
  |
8 |     info!(field = neither, "message");
  |                   ^^^^^^^ this field value cannot be recorded
```

## Trait and method signatures

```rust
/// Outcome of recording one field value.
pub enum FieldRecord {
    Value(Value),
    SerializeFailed(serde_json::Error),
}

pub fn record_field(fields: &mut Map<String, Value>, key: &'static str, record: FieldRecord);
pub fn record_dynamic_field(fields: &mut Map<String, Value>, key: &'static DynamicKey, record: FieldRecord);
pub fn debug_value<T: ?Sized + core::fmt::Debug>(v: &T) -> FieldRecord;     // `?v`
pub fn display_value<T: ?Sized + core::fmt::Display>(v: &T) -> FieldRecord; // `%v`

pub struct FieldValue<'a, T: ?Sized>(pub &'a T);
pub struct SerializeKind;
pub struct DebugKind;

/// Autoref level 0: selected only when `T: Serialize`.
pub trait SerializeKindTag {
    fn __sc_field_kind(&self) -> SerializeKind { SerializeKind }
}
impl<T: ?Sized + serde::Serialize> SerializeKindTag for &FieldValue<'_, T> {}

/// Autoref level 1: selected for every `T`; the bound is checked by `DebugKind::record`.
pub trait DebugKindTag {
    fn __sc_field_kind(&self) -> DebugKind { DebugKind }
}
impl<T: ?Sized> DebugKindTag for FieldValue<'_, T> {}

#[diagnostic::on_unimplemented(
    message = "field value `{Self}` implements neither `serde::Serialize` nor `core::fmt::Debug`",
    label = "this field value cannot be recorded",
    note = "record it with `?value` (Debug) or `%value` (Display), or implement `serde::Serialize`"
)]
pub trait FieldDebug {
    fn field_debug(&self) -> String;
}
impl<T: ?Sized + core::fmt::Debug> FieldDebug for T {
    fn field_debug(&self) -> String { format!("{self:?}") }
}

impl SerializeKind {
    pub fn record<T: ?Sized + serde::Serialize>(self, v: &T) -> FieldRecord {
        match serde_json::to_value(v) {
            Ok(value) => FieldRecord::Value(value),
            Err(err) => FieldRecord::SerializeFailed(err),
        }
    }
}
impl DebugKind {
    pub fn record<T: ?Sized + FieldDebug>(self, v: &T) -> FieldRecord {
        FieldRecord::Value(Value::String(v.field_debug()))
    }
}
```

All of these are `#[doc(hidden)]` through `sc_observability_log::__private` and
outside semver.

## Verified behavior

Asserted by `crates/sc-observability-log/tests/macros_jsonl.rs` and the
`callsite.rs` unit tests, on 1.94.1 (MSRV check) and 1.98.1:

| Value | Recorded |
|---|---|
| a `Serialize + Debug` struct | its JSON (`{"a":1}`) |
| a `Debug`-only struct | its Debug string |
| `5_u32` | `5` |
| `f64::NAN` | `null` (not an error: `to_value(f64::NAN)` is `Value::Null`) |
| `HashMap<(i32, i32), i32>` | `null` plus `serialize_errors["hm"] = "key must be a string"` |
| a custom `Serialize` returning `Err` | `null` plus its message |
| a type with neither trait | compile error above, primary span on the field expression |

Nothing on these paths panics.

## Stability

Relies only on documented method-call autoref/autoderef probing and a stable
diagnostic attribute; no `specialization`. CI checks it on 1.94.1 (MSRV step)
and 1.98.1.

## Size

Field JSON is inserted as produced by `serde_json::to_value`, with no size or
depth cap (matching `LogEvent.fields: serde_json::Map<String, Value>`).

## Serialization failure

`SerializeKind::record` returns `FieldRecord::SerializeFailed(serde_json::Error)`;
`record_field` stores `null` under the key and the error's `Display` text under
`fields["sc_observability_log.serialize_errors"][key]`. No `unwrap`/`expect`
appears on this path.
