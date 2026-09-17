//! Bounded Tauri command adapter for the shared native binding backend.
//!
//! The adapter owns neither logger configuration nor lifecycle. A host keeps
//! its `CoreLoggerOwner`/`LogGuard` and passes only the shared backend here.
use sc_observability_binding_runtime::HostLoggingBackend;
use sc_observability_dto::{
    AdmissionDto, Failure, HealthRequest, LogEventDto, LogHealthDto, LogSnapshotDto, QueryRequest,
    TryLogRequest, WireEnvelope, decode_event, decode_query, decode_timeout,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeSet, sync::Arc, time::Duration};

const SCHEMA_VERSION: u32 = 1;
const MAX_REQUEST_BYTES: usize = 65_536;
const MAX_DEPTH: usize = 32;
const REDACTED: &str = "[REDACTED]";
const PROTECTED_PREFIX: &str = "sc_observability.binding.";

/// Host-selected policy applied before any backend call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPolicy {
    pub allowed_window_labels: BTreeSet<String>,
    pub allowed_targets: BTreeSet<String>,
    pub max_request_bytes: u32,
    pub max_depth: u32,
    pub redacted_field_keys: BTreeSet<String>,
}

impl AdapterPolicy {
    pub fn validate(&self) -> Result<(), Failure> {
        if self.allowed_window_labels.is_empty() || self.allowed_targets.is_empty() {
            return Err(invalid(
                "policy",
                "window and target allowlists must not be empty",
            ));
        }
        if self.max_request_bytes == 0 || self.max_request_bytes as usize > MAX_REQUEST_BYTES {
            return Err(invalid("policy.max_request_bytes", "must be in 1..65536"));
        }
        if self.max_depth == 0 || self.max_depth as usize > MAX_DEPTH {
            return Err(invalid("policy.max_depth", "must be in 1..32"));
        }
        if self
            .allowed_window_labels
            .iter()
            .any(|label| label.is_empty() || !tauri_runtime::window::is_label_valid(label))
        {
            return Err(invalid(
                "policy.allowed_window_labels",
                "labels must be valid Tauri window labels",
            ));
        }
        if self
            .allowed_targets
            .iter()
            .any(|target| sc_observability_types::TargetCategory::new(target).is_err())
        {
            return Err(invalid(
                "policy.allowed_targets",
                "targets must be valid target categories",
            ));
        }
        if self.redacted_field_keys.iter().any(|key| {
            key.starts_with(PROTECTED_PREFIX) || normalize_key(key).starts_with(PROTECTED_PREFIX)
        }) {
            return Err(invalid(
                "policy.redacted_field_keys",
                "protected provenance keys are host-owned",
            ));
        }
        Ok(())
    }
}

/// Serializable result discriminator used by every plugin command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireResult<T> {
    Ok { value: T },
    Error { error: Failure },
}

fn invalid(field: &str, message: &str) -> Failure {
    Failure::Validation {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
            message,
        )),
        field: field.to_owned(),
    }
}

fn envelope<T>(result: Result<T, Failure>) -> WireEnvelope<T> {
    match result {
        Ok(value) => WireEnvelope::Ok {
            schema_version: SCHEMA_VERSION,
            value,
        },
        Err(error) => WireEnvelope::Error {
            schema_version: SCHEMA_VERSION,
            error,
        },
    }
}

fn normalize_key(value: &str) -> String {
    value
        .replace("::", ".")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn inspect(value: &Value, depth: usize, limit: usize) -> Result<(), Failure> {
    match value {
        Value::Array(values) => {
            if depth >= limit {
                return Err(invalid("request", "maximum container depth is 32"));
            }
            values.iter().try_for_each(|item| inspect(item, depth + 1, limit))
        }
        Value::Object(values) => {
            if depth >= limit {
                return Err(invalid("request", "maximum container depth is 32"));
            }
            values.values().try_for_each(|item| inspect(item, depth + 1, limit))
        }
        _ => Ok(()),
    }
}

fn strict_object(value: &Value, allowed: &[&str], field: &str) -> Result<(), Failure> {
    let object = value.as_object().ok_or_else(|| invalid(field, "request must be an object"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid(field, "unknown field"));
    }
    Ok(())
}

fn strict_value(value: &Value, field: &str) -> Result<(), Failure> {
    let object = value.as_object().ok_or_else(|| invalid(field, "value must be a tagged object"))?;
    let kind = object.get("kind").and_then(Value::as_str).ok_or_else(|| invalid(field, "value kind is required"))?;
    let allowed = match kind {
        "null" => &["kind"][..],
        "boolean" | "string" | "integer" | "float" | "array" | "object" => &["kind", "value"][..],
        _ => return Err(invalid(field, "unknown value kind")),
    };
    strict_object(value, allowed, field)?;
    match kind {
        "array" => object.get("value").and_then(Value::as_array).ok_or_else(|| invalid(field, "array value is required"))?
            .iter().enumerate().try_for_each(|(index, child)| strict_value(child, &format!("{field}.value[{index}]")))?,
        "object" => object.get("value").and_then(Value::as_object).ok_or_else(|| invalid(field, "object value is required"))?
            .iter().try_for_each(|(key, child)| strict_value(child, &format!("{field}.value.{key}")))?,
        _ => {}
    }
    Ok(())
}

fn strict_request(value: &Value, operation: &str) -> Result<(), Failure> {
    match operation {
        "try_log" => {
            strict_object(value, &["schema_version", "event"], "request")?;
            let event = value.get("event").ok_or_else(|| invalid("event", "event is required"))?;
            strict_object(event, &["schema_version", "level", "target", "action", "message", "trace", "request_id", "correlation_id", "outcome", "fields"], "event")?;
            if let Some(fields) = event.get("fields") {
                fields.as_object().ok_or_else(|| invalid("event.fields", "fields must be an object"))?
                    .iter().try_for_each(|(key, value)| strict_value(value, &format!("event.fields.{key}")))?;
            }
        }
        "query" => {
            strict_object(value, &["schema_version", "query"], "request")?;
            let query = value.get("query").ok_or_else(|| invalid("query", "query is required"))?;
            strict_object(query, &["schema_version", "service", "levels", "target", "action", "request_id", "correlation_id", "since", "until", "field_matches", "limit", "order"], "query")?;
            if let Some(matches) = query.get("field_matches").and_then(Value::as_array) {
                for (index, entry) in matches.iter().enumerate() {
                    strict_object(entry, &["field", "value"], &format!("query.field_matches[{index}]"))?;
                    if let Some(value) = entry.get("value") { strict_value(value, &format!("query.field_matches[{index}].value"))?; }
                }
            }
        }
        "health" => strict_object(value, &["schema_version"], "request")?,
        "flush" => strict_object(value, &["schema_version", "timeout_ms"], "request")?,
        _ => return Err(invalid("request", "unknown operation")),
    }
    Ok(())
}

fn parse<T: DeserializeOwned>(
    value: Value,
    policy: &AdapterPolicy,
    field: &str,
    operation: &str,
) -> Result<T, Failure> {
    let bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid(field, "request could not be serialized"))?;
    if bytes.len() > policy.max_request_bytes as usize {
        return Err(invalid(field, "request exceeds configured size limit"));
    }
    inspect(&value, 0, policy.max_depth as usize)?;
    strict_request(&value, operation)?;
    serde_json::from_value(value).map_err(|_| invalid(field, "request does not match schema v1"))
}

fn schema(value: &Value, field: &str) -> Result<(), Failure> {
    let version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| invalid(field, "schema_version must be an integer"))?;
    if version != SCHEMA_VERSION {
        return Err(Failure::UnsupportedVersion {
            diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION,
                "unsupported schema version",
            )),
            received: version,
        });
    }
    Ok(())
}

fn authorize(policy: &AdapterPolicy, window: &str) -> Result<(), Failure> {
    if policy.allowed_window_labels.contains(window) {
        Ok(())
    } else {
        Err(Failure::PermissionDenied {
            diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_PERMISSION_DENIED,
                "invoking window is not authorized",
            )),
        })
    }
}

fn redact_value(value: &mut sc_observability_dto::ValueDto, keys: &BTreeSet<String>) {
    if let sc_observability_dto::ValueDto::Object { value: object } = value {
        for (key, child) in object.iter_mut() {
            if keys.contains(key)
                || keys.iter().any(|configured| normalize_key(configured) == normalize_key(key))
            {
                *child = sc_observability_dto::ValueDto::String {
                    value: REDACTED.to_owned(),
                };
            } else {
                redact_value(child, keys);
            }
        }
    } else if let sc_observability_dto::ValueDto::Array { value: values } = value {
        values
            .iter_mut()
            .for_each(|child| redact_value(child, keys));
    }
}

fn redact(mut event: LogEventDto, keys: &BTreeSet<String>) -> LogEventDto {
    event
        .fields
        .values_mut()
        .for_each(|value| redact_value(value, keys));
    event
}

/// Adapter state that can be managed by a Tauri application or exercised by a host test.
#[derive(Clone)]
pub struct Adapter {
    backend: Arc<dyn HostLoggingBackend>,
    policy: AdapterPolicy,
}

impl std::fmt::Debug for Adapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Adapter")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl Adapter {
    pub fn new(
        backend: Arc<dyn HostLoggingBackend>,
        policy: AdapterPolicy,
    ) -> Result<Self, Failure> {
        policy.validate()?;
        Ok(Self { backend, policy })
    }

    pub fn try_log(&self, window: &str, value: Value) -> WireEnvelope<AdmissionDto> {
        envelope(self.try_log_inner(window, value))
    }

    fn try_log_inner(&self, window: &str, value: Value) -> Result<AdmissionDto, Failure> {
        authorize(&self.policy, window)?;
        schema(&value, "request")?;
        let event_value = value
            .get("event")
            .cloned()
            .ok_or_else(|| invalid("event", "event is required"))?;
        let _request: TryLogRequest = parse(value, &self.policy, "request", "try_log")?;
        // Decode the original nested value so serde's nullable-field defaults
        // cannot make an exactly-at-limit request appear oversized.
        let event = decode_event(event_value)?;
        if !self.policy.allowed_targets.contains(&event.target) {
            return Err(invalid("event.target", "target is not allowed"));
        }
        self.backend.try_log(
            redact(event, &self.policy.redacted_field_keys),
            sc_observability_binding_runtime::ProducerOrigin::TauriFrontend,
        )
    }

    pub async fn query(&self, window: &str, value: Value) -> WireEnvelope<LogSnapshotDto> {
        envelope(self.query_inner(window, value).await)
    }

    async fn query_inner(&self, window: &str, value: Value) -> Result<LogSnapshotDto, Failure> {
        authorize(&self.policy, window)?;
        schema(&value, "request")?;
        let request: QueryRequest = parse(value, &self.policy, "request", "query")?;
        let query = decode_query(
            serde_json::to_value(request.query)
                .map_err(|_| invalid("query", "query could not be serialized"))?,
        )?;
        if let Some(target) = &query.target {
            if !self.policy.allowed_targets.contains(target) {
                return Err(invalid("query.target", "target is not allowed"));
            }
        }
        let targets: Vec<String> = query.target.clone().map_or_else(
            || self.policy.allowed_targets.iter().cloned().collect(),
            |target| vec![target],
        );
        let mut events = Vec::new();
        let mut truncated = false;
        for target in targets {
            let mut target_query = query.clone();
            target_query.target = Some(target);
            let operation = self.backend.start_query(target_query)?;
            let snapshot = operation.completion(Duration::from_millis(2_000)).await?;
            truncated |= snapshot.truncated;
            events.extend(snapshot.events);
        }
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        if matches!(query.order, sc_observability_dto::LogOrderDto::NewestFirst) {
            events.reverse();
        }
        if events.len() > query.limit {
            events.truncate(query.limit);
            truncated = true;
        }
        Ok(LogSnapshotDto {
            schema_version: SCHEMA_VERSION,
            events,
            truncated,
        })
    }

    pub fn health(&self, window: &str, value: Value) -> WireEnvelope<LogHealthDto> {
        envelope(self.health_inner(window, value))
    }

    fn health_inner(&self, window: &str, value: Value) -> Result<LogHealthDto, Failure> {
        authorize(&self.policy, window)?;
        schema(&value, "request")?;
        let _: HealthRequest = parse(value, &self.policy, "request", "health")?;
        self.backend.health()
    }

    pub async fn flush(
        &self,
        window: &str,
        value: Value,
    ) -> WireEnvelope<sc_observability_dto::CompletionDto> {
        envelope(self.flush_inner(window, value).await)
    }

    async fn flush_inner(
        &self,
        window: &str,
        value: Value,
    ) -> Result<sc_observability_dto::CompletionDto, Failure> {
        authorize(&self.policy, window)?;
        schema(&value, "request")?;
        let request: sc_observability_dto::FlushRequest = parse(value, &self.policy, "request", "flush")?;
        let timeout = decode_timeout(Value::from(request.timeout_ms))?;
        self.backend
            .start_flush(Duration::from_millis(u64::from(timeout)))?
            .completion(Duration::from_millis(u64::from(timeout)))
            .await?;
        Ok(sc_observability_dto::CompletionDto::Completed)
    }
}

#[cfg(feature = "tauri")]
struct ManagedAdapter(Adapter);

/// Register the isolated plugin after validating all host policy.
#[cfg(feature = "tauri")]
pub fn plugin<R: tauri::Runtime>(
    backend: Arc<dyn HostLoggingBackend>,
    policy: AdapterPolicy,
) -> Result<tauri::plugin::TauriPlugin<R>, Failure> {
    let adapter = Adapter::new(backend, policy)?;
    Ok(tauri::plugin::Builder::new("sc-observability")
        .setup(move |app, _api| {
            app.manage(ManagedAdapter(adapter.clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sc_observability_try_log,
            sc_observability_query,
            sc_observability_health,
            sc_observability_flush
        ])
        .build())
}

#[cfg(feature = "tauri")]
use tauri::Manager;

#[cfg(feature = "tauri")]
#[tauri::command]
fn sc_observability_try_log<R: tauri::Runtime>(
    window: tauri::Window<R>,
    request: Value,
    state: tauri::State<'_, ManagedAdapter>,
) -> WireEnvelope<AdmissionDto> {
    state.0.try_log(window.label(), request)
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn sc_observability_query<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    request: Value,
) -> WireEnvelope<LogSnapshotDto> {
    app.state::<ManagedAdapter>().0.query(window.label(), request).await
}

#[cfg(feature = "tauri")]
#[tauri::command]
fn sc_observability_health<R: tauri::Runtime>(
    window: tauri::Window<R>,
    request: Value,
    state: tauri::State<'_, ManagedAdapter>,
) -> WireEnvelope<LogHealthDto> {
    state.0.health(window.label(), request)
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn sc_observability_flush<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    request: Value,
) -> WireEnvelope<sc_observability_dto::CompletionDto> {
    app.state::<ManagedAdapter>().0.flush(window.label(), request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_binding_runtime::{Operation, ProducerOrigin};
    use sc_observability_dto::{CompletionDto, LogQueryDto};

    struct IpcBackend;

    impl HostLoggingBackend for IpcBackend {
        fn try_log(&self, _: LogEventDto, _: ProducerOrigin) -> Result<AdmissionDto, Failure> {
            Err(Failure::Internal { diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                "test backend",
            )) })
        }
        fn start_query(&self, _: LogQueryDto) -> Result<Operation<LogSnapshotDto>, Failure> {
            Err(Failure::Internal { diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                "test backend",
            )) })
        }
        fn health(&self) -> Result<LogHealthDto, Failure> {
            Err(Failure::Internal { diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                "test backend",
            )) })
        }
        fn start_flush(&self, _: Duration) -> Result<Operation<CompletionDto>, Failure> {
            Err(Failure::Internal { diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                "test backend",
            )) })
        }
    }
    #[test]
    fn policy_rejects_bad_limits_and_provenance() {
        let policy = AdapterPolicy {
            allowed_window_labels: ["main".into()].into(),
            allowed_targets: ["app".into()].into(),
            max_request_bytes: 65_537,
            max_depth: 32,
            redacted_field_keys: BTreeSet::new(),
        };
        assert!(policy.validate().is_err());
        let policy = AdapterPolicy {
            max_request_bytes: 1,
            redacted_field_keys: ["sc_observability::binding::language".into()].into(),
            ..policy
        };
        assert!(policy.validate().is_err());
        let policy = AdapterPolicy {
            allowed_window_labels: ["bad\nlabel".into()].into(),
            ..AdapterPolicy {
                allowed_window_labels: ["main".into()].into(),
                allowed_targets: ["app".into()].into(),
                max_request_bytes: MAX_REQUEST_BYTES as u32,
                max_depth: MAX_DEPTH as u32,
                redacted_field_keys: BTreeSet::new(),
            }
        };
        assert!(matches!(policy.validate(), Err(Failure::Validation { ref field, .. }) if field == "policy.allowed_window_labels"));
        let policy = AdapterPolicy {
            allowed_targets: ["bad target".into()].into(),
            ..AdapterPolicy {
                allowed_window_labels: ["main".into()].into(),
                allowed_targets: ["app".into()].into(),
                max_request_bytes: MAX_REQUEST_BYTES as u32,
                max_depth: MAX_DEPTH as u32,
                redacted_field_keys: BTreeSet::new(),
            }
        };
        assert!(matches!(policy.validate(), Err(Failure::Validation { ref field, .. }) if field == "policy.allowed_targets"));
    }

    #[test]
    fn boundary_rejects_unknown_nested_fields_and_wrong_versions() {
        let request = serde_json::json!({
            "schema_version": 1,
            "event": {
                "schema_version": 1,
                "level": "info",
                "target": "app",
                "action": "test",
                "message": null,
                "trace": null,
                "request_id": null,
                "correlation_id": null,
                "outcome": null,
                "fields": {},
                "future": true
            }
        });
        assert!(strict_request(&request, "try_log").is_err());
        assert!(schema(&serde_json::json!({"schema_version": 2}), "request").is_err());
    }

    #[test]
    fn boundary_rejects_container_at_limit_but_allows_primitive_leaf() {
        fn nested_objects(count: usize, leaf: Value) -> Value {
            (0..count).fold(leaf, |value, _| serde_json::json!({"child": value}))
        }

        assert!(inspect(
            &nested_objects(31, Value::Object(Default::default())),
            0,
            MAX_DEPTH
        )
        .is_ok());
        assert!(inspect(&nested_objects(32, Value::Null), 0, MAX_DEPTH).is_ok());
        assert!(inspect(
            &nested_objects(32, Value::Object(Default::default())),
            0,
            MAX_DEPTH
        )
        .is_err());
    }

    #[test]
    fn exact_request_limit_does_not_reject_omitted_nullable_event_fields() {
        let policy = AdapterPolicy {
            allowed_window_labels: BTreeSet::from(["main".to_owned()]),
            allowed_targets: BTreeSet::from(["app".to_owned()]),
            max_request_bytes: MAX_REQUEST_BYTES as u32,
            max_depth: MAX_DEPTH as u32,
            redacted_field_keys: BTreeSet::new(),
        };
        let adapter = Adapter::new(Arc::new(IpcBackend), policy).unwrap();
        let mut request = serde_json::json!({
            "schema_version": 1,
            "event": {
                "schema_version": 1,
                "level": "info",
                "target": "app",
                "action": "test",
                "message": ""
            }
        });
        let overhead = serde_json::to_vec(&request).unwrap().len();
        request["event"]["message"] = serde_json::json!("x".repeat(MAX_REQUEST_BYTES - overhead));
        assert_eq!(serde_json::to_vec(&request).unwrap().len(), MAX_REQUEST_BYTES);
        let result = adapter.try_log("main", request);
        assert!(matches!(
            result,
            WireEnvelope::Error { error: Failure::Internal { .. }, .. }
        ));
    }

    #[test]
    fn redaction_replaces_nested_keys() {
        let mut value = sc_observability_dto::ValueDto::Object {
            value: [("password".to_owned(), sc_observability_dto::ValueDto::String {
                value: "secret".to_owned(),
            })].into_iter().collect(),
        };
        redact_value(&mut value, &BTreeSet::from(["password".to_owned()]));
        assert_eq!(value, sc_observability_dto::ValueDto::Object {
            value: [("password".to_owned(), sc_observability_dto::ValueDto::String {
                value: REDACTED.to_owned(),
            })].into_iter().collect(),
        });
    }

    #[cfg(feature = "tauri")]
    #[test]
    fn mock_ipc_returns_wire_envelope_from_registered_command() {
        let policy = AdapterPolicy {
            allowed_window_labels: BTreeSet::from(["main".to_owned()]),
            allowed_targets: BTreeSet::from(["app".to_owned()]),
            max_request_bytes: MAX_REQUEST_BYTES as u32,
            max_depth: MAX_DEPTH as u32,
            redacted_field_keys: BTreeSet::new(),
        };
        let app = tauri::test::mock_builder()
            .manage(ManagedAdapter(Adapter::new(Arc::new(IpcBackend), policy).unwrap()))
            .invoke_handler(tauri::generate_handler![sc_observability_health])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let url = window.url().unwrap();
        let response = tauri::test::get_ipc_response(
            &window,
            tauri::webview::InvokeRequest {
                cmd: "sc_observability_health".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url,
                body: serde_json::json!({"request": {"schema_version": 1}}).into(),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_owned(),
            },
        ).unwrap();
        let value = response.deserialize::<Value>().unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["kind"], "error");
    }
}
