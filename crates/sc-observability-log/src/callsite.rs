//! Runtime support for the event macro expansions.
//!
//! Each expansion declares one `static` [`Callsite`] (and one `static`
//! [`DynamicKey`] per `{ KEY } = v` field). Labels are produced on first use by
//! the single a-1 sanitizer (`target_label`, `action_label`, `field_key_label`)
//! and cached in a `OnceLock`, so each call site labels once. Field values are
//! recorded through the autoref kind selection documented in
//! `sc-observability-log-macros/docs/field-value-dispatch.md`.
//!
//! Nothing here panics: label failures are counted as
//! `DropCause::InvalidEvent`, and serialization failures are recorded as `null`
//! plus an entry under `sc_observability_log.serialize_errors`.

use std::borrow::Cow;
use std::sync::OnceLock;

use sc_observability_types::{ActionName, OutcomeLabel, TargetCategory};

use crate::__private::{
    EventParts, LabelError, Level, Map, Value, action_label, emit, field_key_label, record_drop,
    target_label,
};
use crate::DropCause;

/// Reserved field holding serialization errors, keyed by field name.
const SERIALIZE_ERRORS_KEY: &str = "sc_observability_log.serialize_errors";

/// One per expansion site; `new` is const so it can initialize a `static`.
#[derive(Debug)]
pub struct Callsite {
    /// Literal, const, `module_path!()` or `concat!(..)`: unsanitized.
    target: &'static str,
    /// `name:`; unsanitized.
    action: Option<&'static str>,
    labels: OnceLock<CallsiteLabels>,
}

#[derive(Debug)]
struct CallsiteLabels {
    target: Result<TargetCategory, LabelError>,
    /// `None`: no `name:`.
    action: Option<Result<ActionName, LabelError>>,
}

impl Callsite {
    /// Creates a call site from the unsanitized `target:` and `name:` values.
    #[must_use]
    pub const fn new(target: &'static str, action: Option<&'static str>) -> Callsite {
        Callsite {
            target,
            action,
            labels: OnceLock::new(),
        }
    }

    /// Runs the a-1 label functions once per call site, on first use.
    fn labels(&self) -> &CallsiteLabels {
        self.labels.get_or_init(|| CallsiteLabels {
            target: target_label(self.target),
            action: self.action.map(action_label),
        })
    }
}

/// Builds `EventParts` from the cached labels and calls `emit`; never panics.
///
/// An invalid `name:` is counted as `DropCause::InvalidEvent` and the event still
/// emits with the bridge default action. An invalid target (unreachable: an
/// empty target becomes `log`) is counted and the event is not emitted.
pub fn emit_callsite(
    callsite: &'static Callsite,
    level: Level,
    message: Option<String>,
    fields: Map<String, Value>,
) {
    if let Some(parts) = callsite_parts(callsite, level, message, None, fields) {
        emit(parts);
    }
}

/// Builds `EventParts` from the cached call-site labels; shared with `#[instrument]`.
///
/// Returns `None`, after counting `DropCause::InvalidEvent`, when the target
/// failed `target_label`. An invalid `name` is counted and yields `action = None`.
pub(crate) fn callsite_parts(
    callsite: &'static Callsite,
    level: Level,
    message: Option<String>,
    outcome: Option<OutcomeLabel>,
    fields: Map<String, Value>,
) -> Option<EventParts> {
    let labels = callsite.labels();
    let Ok(target) = labels.target.clone() else {
        record_drop(DropCause::InvalidEvent);
        return None;
    };
    let action = match &labels.action {
        None => None,
        Some(Ok(name)) => Some(name.clone()),
        Some(Err(_)) => {
            record_drop(DropCause::InvalidEvent);
            None
        }
    };
    Some(EventParts {
        level,
        target,
        action,
        message,
        outcome,
        fields,
    })
}

/// One per `{ KEY } = v` expansion; `KEY` is a constant `&'static str`, as tracing requires.
#[derive(Debug)]
pub struct DynamicKey {
    raw: &'static str,
    key: OnceLock<Result<String, LabelError>>,
}

impl DynamicKey {
    /// Creates a key from the unsanitized constant.
    #[must_use]
    pub const fn new(raw: &'static str) -> DynamicKey {
        DynamicKey {
            raw,
            key: OnceLock::new(),
        }
    }

    /// Runs a-1 `field_key_label` once and caches the result.
    pub(crate) fn key(&self) -> &Result<String, LabelError> {
        self.key
            .get_or_init(|| field_key_label(self.raw).map(Cow::into_owned))
    }
}

/// Inserts under the cached `field_key_label(KEY)`; an empty or reserved key omits the field and is counted.
pub fn record_dynamic_field(
    fields: &mut Map<String, Value>,
    key: &'static DynamicKey,
    record: FieldRecord,
) {
    match key.key() {
        Ok(clean) => insert_record(fields, clean.clone(), record),
        Err(_) => record_drop(DropCause::InvalidEvent),
    }
}

/// Outcome of recording one field value.
#[derive(Debug)]
pub enum FieldRecord {
    /// The JSON value to store.
    Value(Value),
    /// The value's `Serialize` impl failed; stored as `null` plus the error text.
    SerializeFailed(serde_json::Error),
}

/// Inserts `record` under the canonical `field_key_label(key)`; failures are counted and omitted.
pub fn record_field(fields: &mut Map<String, Value>, key: &'static str, record: FieldRecord) {
    match field_key_label(key) {
        Ok(clean) => insert_record(fields, clean.into_owned(), record),
        Err(_) => record_drop(DropCause::InvalidEvent),
    }
}

fn insert_record(fields: &mut Map<String, Value>, key: String, record: FieldRecord) {
    match record {
        FieldRecord::Value(value) => {
            fields.insert(key, value);
        }
        FieldRecord::SerializeFailed(err) => {
            let errors = fields
                .entry(SERIALIZE_ERRORS_KEY)
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(errors) = errors {
                errors.insert(key.clone(), Value::String(err.to_string()));
            }
            fields.insert(key, Value::Null);
        }
    }
}

/// Records `?v`: the `Debug` string.
#[must_use]
pub fn debug_value<T: ?Sized + core::fmt::Debug>(v: &T) -> FieldRecord {
    FieldRecord::Value(Value::String(format!("{v:?}")))
}

/// Records `%v`: the `Display` string.
#[must_use]
pub fn display_value<T: ?Sized + core::fmt::Display>(v: &T) -> FieldRecord {
    FieldRecord::Value(Value::String(v.to_string()))
}

/// Probe wrapper for the autoref kind selection of a bare field.
#[derive(Debug)]
pub struct FieldValue<'a, T: ?Sized>(pub &'a T);

/// Kind selected for values implementing `serde::Serialize`.
#[derive(Debug, Clone, Copy)]
pub struct SerializeKind;

/// Kind selected for every other value; the call checks `FieldDebug`.
#[derive(Debug, Clone, Copy)]
pub struct DebugKind;

/// Autoref level 0: selected only when `T: Serialize`.
pub trait SerializeKindTag {
    /// Returns the kind that records the value as JSON.
    fn __sc_field_kind(&self) -> SerializeKind {
        SerializeKind
    }
}

impl<T: ?Sized + serde::Serialize> SerializeKindTag for &FieldValue<'_, T> {}

/// Autoref level 1: selected for every `T`; the bound is checked by `DebugKind::record`.
pub trait DebugKindTag {
    /// Returns the kind that records the value as its `Debug` string.
    fn __sc_field_kind(&self) -> DebugKind {
        DebugKind
    }
}

impl<T: ?Sized> DebugKindTag for FieldValue<'_, T> {}

/// Carries the diagnostic for a field value implementing neither trait.
#[diagnostic::on_unimplemented(
    message = "field value `{Self}` implements neither `serde::Serialize` nor `core::fmt::Debug`",
    label = "this field value cannot be recorded",
    note = "record it with `?value` (Debug) or `%value` (Display), or implement `serde::Serialize`"
)]
pub trait FieldDebug {
    /// The value's `Debug` string.
    fn field_debug(&self) -> String;
}

impl<T: ?Sized + core::fmt::Debug> FieldDebug for T {
    fn field_debug(&self) -> String {
        format!("{self:?}")
    }
}

impl SerializeKind {
    /// Serializes `v` to JSON; a failure becomes [`FieldRecord::SerializeFailed`].
    pub fn record<T: ?Sized + serde::Serialize>(self, v: &T) -> FieldRecord {
        match serde_json::to_value(v) {
            Ok(value) => FieldRecord::Value(value),
            Err(err) => FieldRecord::SerializeFailed(err),
        }
    }
}

impl DebugKind {
    /// Records the `Debug` string of `v`.
    pub fn record<T: ?Sized + FieldDebug>(self, v: &T) -> FieldRecord {
        FieldRecord::Value(Value::String(v.field_debug()))
    }
}

#[cfg(test)]
mod tests {
    use crate::__private::LabelKind;

    use super::*;

    #[test]
    fn callsite_caches_sanitized_labels() {
        let callsite = Callsite::new("", Some("bad name"));
        let labels = callsite.labels();
        assert_eq!(
            labels.target.as_ref().map(TargetCategory::as_str),
            Ok("log")
        );
        assert_eq!(
            labels
                .action
                .as_ref()
                .map(|action| action.as_ref().map(ActionName::as_str)),
            Some(Ok("bad_name"))
        );
        assert!(std::ptr::eq(labels, callsite.labels()), "labels are cached");

        let empty_name = Callsite::new("t", Some(""));
        assert_eq!(
            empty_name.labels().action,
            Some(Err(LabelError::Empty {
                kind: LabelKind::Action
            }))
        );
    }

    #[test]
    fn dynamic_key_caches_field_key_label() {
        static SPACED: DynamicKey = DynamicKey::new("a b");
        assert_eq!(
            DynamicKey::new("").key(),
            &Err(LabelError::Empty {
                kind: LabelKind::FieldKey
            })
        );
        for reserved in ["sc_observability_log.x", "sc_observability_log::y"] {
            assert_eq!(
                DynamicKey::new(reserved).key(),
                &Err(LabelError::ReservedPrefix {
                    kind: LabelKind::FieldKey
                })
            );
        }
        let mut fields = Map::new();
        record_dynamic_field(&mut fields, &SPACED, FieldRecord::Value(Value::from(4)));
        assert_eq!(fields.get("a_b"), Some(&Value::from(4)));
    }

    #[test]
    fn static_fields_use_the_same_canonical_key_as_dynamic_fields() {
        let mut fields = Map::new();
        record_field(&mut fields, "a b", FieldRecord::Value(Value::from(4)));
        assert_eq!(fields.get("a_b"), Some(&Value::from(4)));
        assert!(!fields.contains_key("a b"));
    }

    #[test]
    fn serialize_errors_key_is_reserved() {
        assert!(SERIALIZE_ERRORS_KEY.starts_with(crate::__private::RESERVED_FIELD_PREFIX));
    }

    #[test]
    fn serialize_failures_record_null_and_error() {
        let mut fields = Map::new();
        let map = std::collections::HashMap::from([((1, 2), 3)]);
        record_field(&mut fields, "hm", SerializeKind.record(&map));
        record_field(&mut fields, "nan", SerializeKind.record(&f64::NAN));
        assert_eq!(fields.get("hm"), Some(&Value::Null));
        assert_eq!(fields.get("nan"), Some(&Value::Null));
        assert_eq!(
            fields[SERIALIZE_ERRORS_KEY]["hm"],
            Value::from("key must be a string")
        );
    }
}
