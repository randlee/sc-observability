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
            .any(|label| label.is_empty() || label.contains('\0'))
        {
            return Err(invalid(
                "policy.allowed_window_labels",
                "labels must be nonempty and NUL-free",
            ));
        }
        if self
            .allowed_targets
            .iter()
            .any(|target| target.is_empty() || target.contains('\0'))
        {
            return Err(invalid(
                "policy.allowed_targets",
                "targets must be nonempty and NUL-free",
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
    if depth > limit {
        return Err(invalid("request", "maximum container depth is 32"));
    }
    match value {
        Value::Array(values) => values
            .iter()
            .try_for_each(|item| inspect(item, depth + 1, limit)),
        Value::Object(values) => values
            .values()
            .try_for_each(|item| inspect(item, depth + 1, limit)),
        _ => Ok(()),
    }
}

fn parse<T: DeserializeOwned>(
    value: Value,
    policy: &AdapterPolicy,
    field: &str,
) -> Result<T, Failure> {
    let bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid(field, "request could not be serialized"))?;
    if bytes.len() > policy.max_request_bytes as usize {
        return Err(invalid(field, "request exceeds configured size limit"));
    }
    inspect(&value, 0, policy.max_depth as usize)?;
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
            if keys.contains(key) {
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
        let request: TryLogRequest = parse(value, &self.policy, "request")?;
        let event = decode_event(
            serde_json::to_value(request.event)
                .map_err(|_| invalid("event", "event could not be serialized"))?,
        )?;
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
        let request: QueryRequest = parse(value, &self.policy, "request")?;
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
            let snapshot = operation.completion(Duration::from_secs(60)).await?;
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
        let _: HealthRequest = parse(value, &self.policy, "request")?;
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
        let request: sc_observability_dto::FlushRequest = parse(value, &self.policy, "request")?;
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
async fn sc_observability_try_log<R: tauri::Runtime>(
    window: tauri::Window<R>,
    request: Value,
    state: tauri::State<'_, ManagedAdapter>,
) -> Result<WireEnvelope<AdmissionDto>, tauri::Error> {
    Ok(state.0.try_log(window.label(), request))
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn sc_observability_query<R: tauri::Runtime>(
    window: tauri::Window<R>,
    request: Value,
    state: tauri::State<'_, ManagedAdapter>,
) -> Result<WireEnvelope<LogSnapshotDto>, tauri::Error> {
    Ok(state.0.query(window.label(), request).await)
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
    window: tauri::Window<R>,
    request: Value,
    state: tauri::State<'_, ManagedAdapter>,
) -> Result<WireEnvelope<sc_observability_dto::CompletionDto>, tauri::Error> {
    Ok(state.0.flush(window.label(), request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }
}
