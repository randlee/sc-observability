//! Pure record mapping and the single phase-a label sanitizer.
//!
//! Nothing in this module touches global state: `record_to_parts` turns a
//! `log::Record` into [`EventParts`], `structured_to_parts` does the same for a
//! direct bridge event, `assemble_event` completes the envelope, and the
//! sanitizer functions turn arbitrary strings into valid `TargetCategory` /
//! `ActionName` values and field keys. The unified field-key and collision rules
//! shared by every producer are documented in `docs/mapping.md`, "Field keys". The sanitizer items are
//! `pub` inside this private module so `__private` can re-export them; they are
//! reachable from outside the crate only through `__private`.

use std::borrow::Cow;

use sc_observability_types::{
    ActionName, ErrorContext, IdentityError, Level, LogEvent, Observation, ProcessIdentity,
    ProcessIdentityPolicy, Remediation, ServiceName, TargetCategory,
};
use serde_json::{Map, Value};

use crate::__private::EventParts;
use crate::BridgeOptions;

/// Field keys starting with this prefix are reserved for the bridge itself.
pub const RESERVED_FIELD_PREFIX: &str = "sc_observability_log.";

/// Reserved field holding user values displaced by a crate-owned key.
///
/// Crate-owned keys (`code.module` / `code.file` / `code.line` on `log`
/// records; `duration_ms` / `return` / `error` on `#[instrument]` completion
/// events) are always authoritative: a user field already occupying one is
/// moved here under its original key instead of being silently overwritten.
pub(crate) const SHADOWED_FIELDS_KEY: &str = "sc_observability_log.shadowed_fields";

/// Inserts `value` at the crate-owned key `key`, which always wins.
///
/// Uses [`Map::insert`]'s returned `Option<Value>` to detect a collision with
/// an existing user field; the displaced value is preserved under
/// `fields[SHADOWED_FIELDS_KEY][key]` rather than dropped. Never panics.
pub(crate) fn insert_authoritative(fields: &mut Map<String, Value>, key: &str, value: Value) {
    if let Some(previous) = fields.insert(key.to_owned(), value) {
        let shadowed = fields
            .entry(SHADOWED_FIELDS_KEY)
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(shadowed) = shadowed {
            shadowed.insert(key.to_owned(), previous);
        }
    }
}

/// Which kind of label failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    /// A `LogEvent.target` label.
    Target,
    /// A `LogEvent.action` label.
    Action,
    /// A `LogEvent.fields` key.
    FieldKey,
}

/// Why a label could not be produced. Counted as `DropCause::InvalidEvent` by callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelError {
    /// The label was empty after sanitizing.
    Empty {
        /// Which label failed.
        kind: LabelKind,
    },
    /// The field key starts with [`RESERVED_FIELD_PREFIX`] after sanitizing.
    ReservedPrefix {
        /// Which label failed.
        kind: LabelKind,
    },
    /// sc-observability rejected the sanitized value (not expected for sanitized input).
    Rejected {
        /// Which label failed.
        kind: LabelKind,
        /// Validation error reported by sc-observability.
        source: sc_observability_types::ValueValidationError,
    },
}

fn valid_label_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')
}

/// Rewrites `::` to `.` and every char outside `[A-Za-z0-9._-]` to `_`.
///
/// Borrows when `raw` is already valid and never fails.
#[must_use]
pub fn sanitize_label(raw: &str) -> Cow<'_, str> {
    if !raw.contains("::") && raw.chars().all(valid_label_char) {
        return Cow::Borrowed(raw);
    }
    Cow::Owned(
        raw.replace("::", ".")
            .chars()
            .map(|ch| if valid_label_char(ch) { ch } else { '_' })
            .collect(),
    )
}

/// Sanitizes a target label; an empty result maps to `log`.
///
/// # Errors
///
/// Returns [`LabelError::Rejected`] if sc-observability rejects the sanitized value.
pub fn target_label(raw: &str) -> Result<TargetCategory, LabelError> {
    let clean = sanitize_label(raw);
    let clean = if clean.is_empty() {
        Cow::Borrowed("log")
    } else {
        clean
    };
    TargetCategory::new(clean.into_owned()).map_err(|source| LabelError::Rejected {
        kind: LabelKind::Target,
        source,
    })
}

/// Sanitizes an action label.
///
/// # Errors
///
/// Returns [`LabelError::Empty`] for an empty result and [`LabelError::Rejected`]
/// if sc-observability rejects the sanitized value.
pub fn action_label(raw: &str) -> Result<ActionName, LabelError> {
    let clean = sanitize_label(raw);
    if clean.is_empty() {
        return Err(LabelError::Empty {
            kind: LabelKind::Action,
        });
    }
    ActionName::new(clean.into_owned()).map_err(|source| LabelError::Rejected {
        kind: LabelKind::Action,
        source,
    })
}

/// Sanitizes a field key.
///
/// # Errors
///
/// Returns [`LabelError::Empty`] for an empty result and
/// [`LabelError::ReservedPrefix`] when the result starts with
/// [`RESERVED_FIELD_PREFIX`].
pub fn field_key_label(raw: &str) -> Result<Cow<'_, str>, LabelError> {
    let clean = sanitize_label(raw);
    if clean.is_empty() {
        return Err(LabelError::Empty {
            kind: LabelKind::FieldKey,
        });
    }
    if clean.starts_with(RESERVED_FIELD_PREFIX) {
        return Err(LabelError::ReservedPrefix {
            kind: LabelKind::FieldKey,
        });
    }
    Ok(clean)
}

/// Resolves `LoggerConfig.process_identity` once, at `init`.
///
/// sc-observability 1.2.0 stores the policy but never applies it, so the bridge
/// resolves it and stamps every `LogEvent.identity` with the cached value. Every
/// failure carries the stable code `SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED`
/// and a remediation for its own path; a resolver's error is kept as the source.
pub(crate) fn resolve_identity(
    policy: &ProcessIdentityPolicy,
) -> Result<ProcessIdentity, IdentityError> {
    match policy {
        ProcessIdentityPolicy::Auto => resolve_auto_identity(hostname::get),
        ProcessIdentityPolicy::Fixed { hostname, pid } => Ok(ProcessIdentity {
            hostname: hostname.clone(),
            pid: *pid,
        }),
        ProcessIdentityPolicy::Resolver(resolver) => resolver.resolve().map_err(|source| {
            IdentityError(Box::new(
                ErrorContext::new(
                    crate::error_codes::SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED,
                    "the configured ProcessIdentityResolver failed",
                    Remediation::recoverable(
                        "fix the ProcessIdentityResolver",
                        [
                            "or use ProcessIdentityPolicy::Auto or Fixed",
                            "call sc_observability_log::init again",
                        ],
                    ),
                )
                .source(Box::new(source)),
            ))
        }),
    }
}

/// Resolves `ProcessIdentityPolicy::Auto` with `hostname_of` (production: `hostname::get`).
///
/// A failed or empty hostname lookup is an error; the pid is the current process id.
fn resolve_auto_identity(
    hostname_of: impl FnOnce() -> std::io::Result<std::ffi::OsString>,
) -> Result<ProcessIdentity, IdentityError> {
    let hostname = hostname_of().map_err(|source| {
        IdentityError(Box::new(
            ErrorContext::new(
                crate::error_codes::SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED,
                "automatic hostname resolution failed",
                auto_hostname_remediation(),
            )
            .source(Box::new(source)),
        ))
    })?;
    let hostname = hostname.to_string_lossy().into_owned();
    if hostname.is_empty() {
        return Err(IdentityError(Box::new(ErrorContext::new(
            crate::error_codes::SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED,
            "automatic hostname resolution returned an empty hostname",
            auto_hostname_remediation(),
        ))));
    }
    Ok(ProcessIdentity {
        hostname: Some(hostname),
        pid: Some(std::process::id()),
    })
}

/// Remediation for an `Auto` hostname lookup that failed or returned an empty hostname.
fn auto_hostname_remediation() -> Remediation {
    Remediation::recoverable(
        "ensure the operating system resolves a non-empty hostname",
        [
            "or use ProcessIdentityPolicy::Fixed",
            "call sc_observability_log::init again",
        ],
    )
}

pub(crate) fn map_level(level: log::Level) -> Level {
    match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warn,
        log::Level::Info => Level::Info,
        log::Level::Debug => Level::Debug,
        log::Level::Trace => Level::Trace,
    }
}

/// Splits a leading `[tag]` off `message` when `tag` is a valid, non-empty label.
///
/// Returns the tag and the message with the tag and one following space removed.
fn split_bracket_tag(message: &str) -> Option<(&str, &str)> {
    let rest = message.strip_prefix('[')?;
    let (tag, after) = rest.split_once(']')?;
    if tag.is_empty() || !matches!(sanitize_label(tag), Cow::Borrowed(_)) {
        return None;
    }
    Some((tag, after.strip_prefix(' ').unwrap_or(after)))
}

/// Collects `log` key-values into JSON fields.
///
/// A key rejected by [`field_key_label`] (empty or reserved after sanitizing) is
/// omitted and counted in `omitted`; accepted keys are stored in their canonical
/// sanitized form, matching every other runtime producer.
struct FieldCollector<'a> {
    fields: &'a mut Map<String, Value>,
    omitted: u64,
}

impl<'kvs> log::kv::VisitSource<'kvs> for FieldCollector<'_> {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'kvs>,
        value: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        let Ok(key) = field_key_label(key.as_str()) else {
            self.omitted = self.omitted.saturating_add(1);
            return Ok(());
        };
        self.fields
            .insert(key.into_owned(), kv_value_to_json(&value));
        Ok(())
    }
}

/// A mapped `log` record plus the number of key-values omitted by the key rules.
#[derive(Debug)]
pub(crate) struct MappedRecord {
    /// The call-site-controlled parts of the event.
    pub(crate) parts: EventParts,
    /// Key-values with an empty or reserved key; each is counted as `DropCause::InvalidEvent`.
    pub(crate) omitted_fields: u64,
}

/// Numbers, bools and strings stay typed; everything else is rendered with `to_string()`.
fn kv_value_to_json(value: &log::kv::Value<'_>) -> Value {
    if let Some(flag) = value.to_bool() {
        return Value::Bool(flag);
    }
    if let Some(number) = value.to_i64() {
        return Value::from(number);
    }
    if let Some(number) = value.to_u64() {
        return Value::from(number);
    }
    if let Some(number) = value.to_f64().and_then(serde_json::Number::from_f64) {
        return Value::Number(number);
    }
    if let Some(text) = value.to_borrowed_str() {
        return Value::String(text.to_owned());
    }
    Value::String(value.to_string())
}

/// Maps one `log` record to the call-site-controlled parts of a `LogEvent`.
///
/// Returns the label error when the sanitized target is rejected; the caller
/// counts it as `DropCause::InvalidEvent`.
pub(crate) fn record_to_parts(
    record: &log::Record<'_>,
    options: &BridgeOptions,
) -> Result<MappedRecord, LabelError> {
    let target = target_label(record.target())?;
    let formatted = record
        .args()
        .as_str()
        .map_or_else(|| record.args().to_string(), ToOwned::to_owned);

    let (action, message) = match options
        .parse_bracket_action
        .then(|| split_bracket_tag(&formatted))
        .flatten()
    {
        Some((tag, rest)) => (Some(action_label(tag)?), rest.to_owned()),
        None => (None, formatted),
    };

    let mut fields = Map::new();
    let mut collector = FieldCollector {
        fields: &mut fields,
        omitted: 0,
    };
    let _ = record.key_values().visit(&mut collector);
    let omitted_fields = collector.omitted;
    if let Some(module) = record.module_path() {
        insert_authoritative(&mut fields, "code.module", Value::from(module));
    }
    if let Some(file) = record.file() {
        insert_authoritative(&mut fields, "code.file", Value::from(file));
    }
    if let Some(line) = record.line() {
        insert_authoritative(&mut fields, "code.line", Value::from(line));
    }

    Ok(MappedRecord {
        parts: EventParts {
            level: map_level(record.level()),
            target,
            action,
            message: Some(message),
            outcome: None,
            fields,
        },
        omitted_fields,
    })
}

/// Completes the envelope: version, timestamp, service and identity (`trace` is set by `emit`).
pub(crate) fn assemble_event(
    parts: EventParts,
    service: &ServiceName,
    identity: &ProcessIdentity,
    default_action: &ActionName,
) -> LogEvent {
    // `Observation::new` yields the shared, pre-validated envelope version and a
    // UTC timestamp without a fallible `SchemaVersion::new` call in this crate.
    let Observation {
        version, timestamp, ..
    } = Observation::new(service.clone(), ());
    LogEvent {
        version,
        timestamp,
        level: parts.level,
        service: service.clone(),
        target: parts.target,
        action: parts.action.unwrap_or_else(|| default_action.clone()),
        message: parts.message,
        identity: identity.clone(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: parts.outcome,
        diagnostic: None,
        state_transition: None,
        fields: parts.fields,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sc_observability_types::{
        ErrorCode, ErrorContext, ProcessIdentityResolver, Remediation, constants,
    };

    use super::*;

    fn options(parse: bool) -> BridgeOptions {
        BridgeOptions {
            default_action: ActionName::new("log.record").unwrap(),
            parse_bracket_action: parse,
        }
    }

    fn parts_for(target: &str, args: std::fmt::Arguments<'_>, parse: bool) -> EventParts {
        let record = log::Record::builder()
            .level(log::Level::Info)
            .target(target)
            .args(args)
            .build();
        record_to_parts(&record, &options(parse)).unwrap().parts
    }

    // ---- sanitizer ----

    #[test]
    fn sanitizer_rewrites_paths_and_invalid_chars() {
        assert_eq!(sanitize_label("app_lib::sync"), "app_lib.sync");
        assert_eq!(sanitize_label("a b"), "a_b");
        assert_eq!(sanitize_label("üö/x"), "___x");
        assert!(matches!(sanitize_label("a.b-c_d"), Cow::Borrowed(_)));
        assert!(matches!(sanitize_label("a::b"), Cow::Owned(_)));
    }

    #[test]
    fn target_label_maps_empty_to_log() {
        assert_eq!(target_label("").unwrap().as_str(), "log");
        assert_eq!(
            target_label("app_lib::sync").unwrap().as_str(),
            "app_lib.sync"
        );
        assert_eq!(target_label("   ").unwrap().as_str(), "___");
    }

    #[test]
    fn action_label_rejects_empty() {
        assert_eq!(
            action_label(""),
            Err(LabelError::Empty {
                kind: LabelKind::Action
            })
        );
        assert_eq!(action_label("sync start").unwrap().as_str(), "sync_start");
    }

    #[test]
    fn field_key_label_rules() {
        assert_eq!(
            field_key_label("sc_observability_log.x"),
            Err(LabelError::ReservedPrefix {
                kind: LabelKind::FieldKey
            })
        );
        assert_eq!(
            field_key_label("sc_observability_log::x"),
            Err(LabelError::ReservedPrefix {
                kind: LabelKind::FieldKey
            })
        );
        assert_eq!(
            field_key_label(""),
            Err(LabelError::Empty {
                kind: LabelKind::FieldKey
            })
        );
        assert!(matches!(field_key_label("a.b"), Ok(Cow::Borrowed("a.b"))));
        assert!(matches!(field_key_label("a b"), Ok(Cow::Owned(ref s)) if s == "a_b"));
    }

    // ---- mapping rows ----

    #[test]
    fn level_row_maps_every_level() {
        for (from, to) in [
            (log::Level::Error, Level::Error),
            (log::Level::Warn, Level::Warn),
            (log::Level::Info, Level::Info),
            (log::Level::Debug, Level::Debug),
            (log::Level::Trace, Level::Trace),
        ] {
            let record = log::Record::builder()
                .level(from)
                .target("t")
                .args(format_args!("m"))
                .build();
            assert_eq!(
                record_to_parts(&record, &options(true))
                    .unwrap()
                    .parts
                    .level,
                to
            );
        }
    }

    #[test]
    fn target_row_sanitizes_invalid_and_empty_targets() {
        assert_eq!(
            parts_for("app_lib::commands::sync", format_args!("m"), true)
                .target
                .as_str(),
            "app_lib.commands.sync"
        );
        assert_eq!(
            parts_for("weird target!", format_args!("m"), true)
                .target
                .as_str(),
            "weird_target_"
        );
        assert_eq!(
            parts_for("", format_args!("m"), true).target.as_str(),
            "log"
        );
    }

    #[test]
    fn tag_row_extracts_valid_tags() {
        let parts = parts_for("t", format_args!("[sync.start] hello world"), true);
        assert_eq!(parts.action.unwrap().as_str(), "sync.start");
        assert_eq!(parts.message.as_deref(), Some("hello world"));

        // A tag without a following space: only the tag is removed.
        let parts = parts_for("t", format_args!("[tag]hello"), true);
        assert_eq!(parts.action.unwrap().as_str(), "tag");
        assert_eq!(parts.message.as_deref(), Some("hello"));

        // Exactly one following space is removed.
        let parts = parts_for("t", format_args!("[tag]  two"), true);
        assert_eq!(parts.message.as_deref(), Some(" two"));
    }

    #[test]
    fn tag_row_leaves_invalid_tags_in_message() {
        for text in [
            "[bad tag] m",
            "[] m",
            "[unclosed m",
            "no tag",
            "[a/b] m",
            " [lead] m",
        ] {
            let parts = parts_for("t", format_args!("{text}"), true);
            assert!(parts.action.is_none(), "{text}");
            assert_eq!(parts.message.as_deref(), Some(text));
        }
    }

    #[test]
    fn otherwise_row_uses_default_action() {
        let parts = parts_for("t", format_args!("[tag] m"), false);
        assert!(parts.action.is_none());
        assert_eq!(parts.message.as_deref(), Some("[tag] m"));
        let event = assemble_event(
            parts,
            &ServiceName::new("svc").unwrap(),
            &ProcessIdentity::default(),
            &ActionName::new("log.record").unwrap(),
        );
        assert_eq!(event.action.as_str(), "log.record");
    }

    #[test]
    fn message_row_formats_args() {
        let value = 42;
        let parts = parts_for("t", format_args!("value={value}"), true);
        assert_eq!(parts.message.as_deref(), Some("value=42"));
    }

    #[test]
    fn kv_row_types_json_values() {
        struct Shown;
        impl std::fmt::Display for Shown {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("shown")
            }
        }
        let shown = Shown;
        let kvs: [(&str, log::kv::Value<'_>); 8] = [
            ("int", log::kv::Value::from(-7_i64)),
            ("uint", log::kv::Value::from(u64::MAX)),
            ("float", log::kv::Value::from(1.5_f64)),
            ("nan", log::kv::Value::from(f64::NAN)),
            ("flag", log::kv::Value::from(true)),
            ("text", log::kv::Value::from("hello")),
            ("ch", log::kv::Value::from('c')),
            ("display", log::kv::Value::from_display(&shown)),
        ];
        let record = log::Record::builder()
            .level(log::Level::Info)
            .target("t")
            .args(format_args!("m"))
            .key_values(&kvs)
            .build();
        let mapped = record_to_parts(&record, &options(true)).unwrap();
        assert_eq!(mapped.omitted_fields, 0);
        let fields = mapped.parts.fields;
        assert_eq!(fields["int"], Value::from(-7));
        assert_eq!(fields["uint"], Value::from(u64::MAX));
        assert_eq!(fields["float"], Value::from(1.5));
        assert_eq!(fields["nan"], Value::from("NaN"));
        assert_eq!(fields["flag"], Value::Bool(true));
        assert_eq!(fields["text"], Value::from("hello"));
        assert_eq!(fields["ch"], Value::from("c"));
        assert_eq!(fields["display"], Value::from("shown"));
    }

    #[test]
    fn code_location_rows_present_and_omitted() {
        let record = log::Record::builder()
            .level(log::Level::Info)
            .target("t")
            .args(format_args!("m"))
            .module_path(Some("app_lib::sync"))
            .file(Some("src/sync.rs"))
            .line(Some(12))
            .build();
        let fields = record_to_parts(&record, &options(true))
            .unwrap()
            .parts
            .fields;
        assert_eq!(fields["code.module"], Value::from("app_lib::sync"));
        assert_eq!(fields["code.file"], Value::from("src/sync.rs"));
        assert_eq!(fields["code.line"], Value::from(12));

        let fields = parts_for("t", format_args!("m"), true).fields;
        assert!(!fields.contains_key("code.module"));
        assert!(!fields.contains_key("code.file"));
        assert!(!fields.contains_key("code.line"));
    }

    #[test]
    fn envelope_rows_filled_by_assemble_event() {
        let service = ServiceName::new("svc").unwrap();
        let identity = ProcessIdentity {
            hostname: Some("h".to_owned()),
            pid: Some(9),
        };
        let parts = parts_for("t", format_args!("[act] m"), true);
        let event = assemble_event(parts, &service, &identity, &ActionName::new("d").unwrap());
        assert_eq!(
            event.version.as_str(),
            constants::OBSERVATION_ENVELOPE_VERSION
        );
        assert_eq!(event.service, service);
        assert_eq!(event.identity, identity);
        assert_eq!(event.action.as_str(), "act");
        assert_eq!(event.message.as_deref(), Some("m"));
        assert!(event.trace.is_none());
        assert!(event.request_id.is_none());
        assert!(event.correlation_id.is_none());
        assert!(event.outcome.is_none());
        assert!(event.diagnostic.is_none());
        assert!(event.state_transition.is_none());
        let age = sc_observability_types::Timestamp::now_utc() - event.timestamp;
        assert!(age.is_positive() || age.is_zero());
    }

    #[test]
    fn kv_key_rules_omit_reserved_and_empty_keys_and_shadow_code_keys() {
        let kvs: [(&str, log::kv::Value<'_>); 5] = [
            (
                "sc_observability_log.shadowed_fields",
                log::kv::Value::from(1),
            ),
            ("sc_observability_log::x", log::kv::Value::from(2)),
            ("", log::kv::Value::from(3)),
            ("code.line", log::kv::Value::from("user line")),
            ("a b", log::kv::Value::from(4)),
        ];
        let record = log::Record::builder()
            .level(log::Level::Info)
            .target("t")
            .args(format_args!("m"))
            .line(Some(7))
            .key_values(&kvs)
            .build();
        let mapped = record_to_parts(&record, &options(true)).unwrap();
        assert_eq!(mapped.omitted_fields, 3);
        let fields = mapped.parts.fields;
        assert_eq!(fields["code.line"], Value::from(7));
        assert_eq!(
            fields[SHADOWED_FIELDS_KEY],
            serde_json::json!({"code.line": "user line"})
        );
        assert_eq!(
            fields["a_b"],
            Value::from(4),
            "accepted keys are stored in canonical form"
        );
        assert_eq!(fields.len(), 3);
    }

    // ---- process identity ----

    struct OkResolver;
    impl ProcessIdentityResolver for OkResolver {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
            Ok(ProcessIdentity {
                hostname: Some("resolved".to_owned()),
                pid: Some(1),
            })
        }
    }

    struct FailingResolver;
    impl ProcessIdentityResolver for FailingResolver {
        fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
            Err(IdentityError(Box::new(ErrorContext::new(
                ErrorCode::new_static("TEST_RESOLVER_FAILED"),
                "resolver failed",
                Remediation::not_recoverable("test"),
            ))))
        }
    }

    #[test]
    fn identity_auto_uses_hostname_and_current_pid() {
        let identity = resolve_identity(&ProcessIdentityPolicy::Auto).unwrap();
        assert_eq!(identity.pid, Some(std::process::id()));
        assert!(
            identity
                .hostname
                .is_some_and(|hostname| !hostname.is_empty())
        );
    }

    #[test]
    fn identity_fixed_is_used_as_given() {
        let policy = ProcessIdentityPolicy::Fixed {
            hostname: Some("host".to_owned()),
            pid: None,
        };
        assert_eq!(
            resolve_identity(&policy).unwrap(),
            ProcessIdentity {
                hostname: Some("host".to_owned()),
                pid: None
            }
        );
    }

    #[test]
    fn identity_resolver_ok_and_err() {
        let ok = ProcessIdentityPolicy::Resolver(Arc::new(OkResolver));
        assert_eq!(
            resolve_identity(&ok).unwrap().hostname.as_deref(),
            Some("resolved")
        );
        let failing = ProcessIdentityPolicy::Resolver(Arc::new(FailingResolver));
        let error = resolve_identity(&failing).unwrap_err();
        assert_eq!(
            error.0.diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED
        );
        assert_eq!(
            error.0.diagnostic().remediation,
            Remediation::recoverable(
                "fix the ProcessIdentityResolver",
                [
                    "or use ProcessIdentityPolicy::Auto or Fixed",
                    "call sc_observability_log::init again",
                ],
            )
        );
        // The resolver's own error is kept as the source.
        let source = std::error::Error::source(&*error.0)
            .and_then(|source| source.downcast_ref::<IdentityError>())
            .unwrap();
        assert_eq!(source.0.diagnostic().code.as_str(), "TEST_RESOLVER_FAILED");
    }

    fn assert_auto_hostname_failure(error: &IdentityError) {
        assert_eq!(
            error.0.diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_IDENTITY_RESOLUTION_FAILED
        );
        assert_eq!(
            error.0.diagnostic().remediation,
            auto_hostname_remediation()
        );
    }

    #[test]
    fn identity_auto_fails_when_the_hostname_lookup_fails() {
        let error =
            resolve_auto_identity(|| Err(std::io::Error::other("no hostname"))).unwrap_err();
        assert_auto_hostname_failure(&error);
        assert!(std::error::Error::source(&*error.0).is_some());
    }

    #[test]
    fn identity_auto_fails_on_an_empty_hostname() {
        let error = resolve_auto_identity(|| Ok(std::ffi::OsString::new())).unwrap_err();
        assert_auto_hostname_failure(&error);
    }
}
