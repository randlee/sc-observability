//! Bounded Tauri command adapter for the shared native binding backend.
//!
//! The adapter owns neither logger configuration nor lifecycle. A host keeps
//! its `CoreLoggerOwner`/`LogGuard` and passes only the shared backend here.
use sc_observability_binding_runtime::HostLoggingBackend;
use sc_observability_dto::{
    AdmissionDto, Failure, HealthRequest, LogEventDto, LogHealthDto, LogSnapshotDto, QueryRequest,
    TryLogRequest, WireEnvelope,
    constants::{MAX_CONTAINER_DEPTH, MAX_WIRE_PAYLOAD_BYTES},
    decode_event, decode_query, decode_timeout, is_protected_key, normalize_field_key,
};
use sc_observability_types::v2;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};

const SCHEMA_VERSION: u32 = 1;
const MAX_QUERY_TARGETS: usize = 64;
const DEFAULT_QUERY_TIMEOUT_MS: u32 = 2_000;
const REDACTED: &str = "[REDACTED]";

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
        if self.allowed_targets.len() > MAX_QUERY_TARGETS {
            return Err(invalid(
                "policy.allowed_targets",
                "target allowlist exceeds the query fan-out bound",
            ));
        }
        if self.max_request_bytes == 0 || self.max_request_bytes as usize > MAX_WIRE_PAYLOAD_BYTES {
            return Err(invalid(
                "policy.max_request_bytes",
                format!("must be in 1..{MAX_WIRE_PAYLOAD_BYTES}"),
            ));
        }
        if self.max_depth == 0 || self.max_depth as usize > MAX_CONTAINER_DEPTH {
            return Err(invalid(
                "policy.max_depth",
                format!("must be in 1..{MAX_CONTAINER_DEPTH}"),
            ));
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
        for target in &self.allowed_targets {
            if let Err(error) = sc_observability_types::TargetCategory::new(target) {
                return Err(invalid(
                    "policy.allowed_targets",
                    format!("target {target:?} is invalid: {error}"),
                ));
            }
        }
        if self
            .redacted_field_keys
            .iter()
            .any(|key| is_protected_key(key))
        {
            return Err(invalid(
                "policy.redacted_field_keys",
                "protected provenance keys are host-owned",
            ));
        }
        Ok(())
    }
}

/// Optional observation settings which supplement the stable host policy.
///
/// `Adapter::new` and `plugin` retain the 2000 ms default. Hosts that need a
/// different query observation deadline can use `Adapter::with_settings` or
/// `plugin_with_settings` without changing the established `AdapterPolicy`
/// struct shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSettings {
    pub policy: AdapterPolicy,
    pub query_timeout_ms: u32,
}

impl AdapterSettings {
    pub fn validate(&self) -> Result<(), Failure> {
        self.policy.validate()?;
        decode_timeout(Value::from(self.query_timeout_ms)).map(|_| ())
    }
}

impl From<AdapterPolicy> for AdapterSettings {
    fn from(policy: AdapterPolicy) -> Self {
        Self {
            policy,
            query_timeout_ms: DEFAULT_QUERY_TIMEOUT_MS,
        }
    }
}

/// Serializable result discriminator used by every plugin command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireResult<T> {
    Ok { value: T },
    Error { error: Failure },
}

fn invalid(field: &str, message: impl Into<String>) -> Failure {
    Failure::Validation {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
            message.into(),
        )),
        field: field.to_owned(),
    }
}

fn query_timeout_failure() -> Failure {
    Failure::Timeout {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
            "query aggregate observation deadline elapsed",
        )),
        operation: "query".to_owned(),
    }
}

/// Projects a canonical event error into the stable Tauri failure envelope.
pub fn project_canonical_event(error: &v2::EventError) -> Failure {
    sc_observability_dto::failure_from_classification(
        error.diagnostic(),
        error.failure_classification(),
    )
}

/// Projects a canonical flush error into the stable Tauri failure envelope.
pub fn project_canonical_flush(error: &v2::FlushError) -> Failure {
    sc_observability_dto::failure_from_classification(
        error.diagnostic(),
        error.failure_classification(),
    )
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

#[cfg(feature = "tauri")]
fn blocking_task_failure() -> Failure {
    Failure::Internal {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
            "the blocking Tauri operation did not complete",
        )),
    }
}

fn inspect(value: &Value, depth: usize, limit: usize) -> Result<(), Failure> {
    match value {
        Value::Array(values) => {
            if depth >= limit {
                return Err(invalid(
                    "request",
                    format!("maximum container depth is {MAX_CONTAINER_DEPTH}"),
                ));
            }
            values
                .iter()
                .try_for_each(|item| inspect(item, depth + 1, limit))
        }
        Value::Object(values) => {
            if depth >= limit {
                return Err(invalid(
                    "request",
                    format!("maximum container depth is {MAX_CONTAINER_DEPTH}"),
                ));
            }
            values
                .values()
                .try_for_each(|item| inspect(item, depth + 1, limit))
        }
        _ => Ok(()),
    }
}

fn strict_object(value: &Value, allowed: &[&str], field: &str) -> Result<(), Failure> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(field, "request must be an object"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid(field, "unknown field"));
    }
    Ok(())
}

fn strict_value(value: &Value, field: &str) -> Result<(), Failure> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(field, "value must be a tagged object"))?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(field, "value kind is required"))?;
    let allowed = match kind {
        "null" => &["kind"][..],
        "boolean" | "string" | "integer" | "float" | "array" | "object" => &["kind", "value"][..],
        _ => return Err(invalid(field, "unknown value kind")),
    };
    strict_object(value, allowed, field)?;
    match kind {
        "array" => object
            .get("value")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(field, "array value is required"))?
            .iter()
            .enumerate()
            .try_for_each(|(index, child)| {
                strict_value(child, &format!("{field}.value[{index}]"))
            })?,
        "object" => object
            .get("value")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid(field, "object value is required"))?
            .iter()
            .try_for_each(|(key, child)| strict_value(child, &format!("{field}.value.{key}")))?,
        _ => {}
    }
    Ok(())
}

fn strict_request(value: &Value, operation: &str) -> Result<(), Failure> {
    match operation {
        "try_log" => {
            strict_object(value, &["schema_version", "event"], "request")?;
            let event = value
                .get("event")
                .ok_or_else(|| invalid("event", "event is required"))?;
            strict_object(
                event,
                &[
                    "schema_version",
                    "level",
                    "target",
                    "action",
                    "message",
                    "trace",
                    "request_id",
                    "correlation_id",
                    "outcome",
                    "fields",
                ],
                "event",
            )?;
            if let Some(fields) = event.get("fields") {
                fields
                    .as_object()
                    .ok_or_else(|| invalid("event.fields", "fields must be an object"))?
                    .iter()
                    .try_for_each(|(key, value)| {
                        strict_value(value, &format!("event.fields.{key}"))
                    })?;
            }
        }
        "query" => {
            strict_object(value, &["schema_version", "query"], "request")?;
            let query = value
                .get("query")
                .ok_or_else(|| invalid("query", "query is required"))?;
            strict_object(
                query,
                &[
                    "schema_version",
                    "service",
                    "levels",
                    "target",
                    "action",
                    "request_id",
                    "correlation_id",
                    "since",
                    "until",
                    "field_matches",
                    "limit",
                    "order",
                ],
                "query",
            )?;
            if let Some(matches) = query.get("field_matches").and_then(Value::as_array) {
                for (index, entry) in matches.iter().enumerate() {
                    strict_object(
                        entry,
                        &["field", "value"],
                        &format!("query.field_matches[{index}]"),
                    )?;
                    if let Some(value) = entry.get("value") {
                        strict_value(value, &format!("query.field_matches[{index}].value"))?;
                    }
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
        .map_err(|error| invalid(field, format!("request could not be serialized: {error}")))?;
    if bytes.len() > policy.max_request_bytes as usize {
        return Err(invalid(field, "request exceeds configured size limit"));
    }
    inspect(&value, 0, policy.max_depth as usize)?;
    strict_request(&value, operation)?;
    serde_json::from_value(value)
        .map_err(|error| invalid(field, format!("request does not match schema v1: {error}")))
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
                || keys
                    .iter()
                    .any(|configured| normalize_field_key(configured) == normalize_field_key(key))
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
    query_timeout: Duration,
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
        Self::with_settings(backend, AdapterSettings::from(policy))
    }

    /// Creates an adapter with host-selected query observation settings.
    pub fn with_settings(
        backend: Arc<dyn HostLoggingBackend>,
        settings: AdapterSettings,
    ) -> Result<Self, Failure> {
        settings.validate()?;
        Ok(Self {
            backend,
            query_timeout: Duration::from_millis(u64::from(settings.query_timeout_ms)),
            policy: settings.policy,
        })
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
        let query = decode_query(serde_json::to_value(request.query).map_err(|error| {
            invalid("query", format!("query could not be serialized: {error}"))
        })?)?;
        if let Some(target) = &query.target
            && !self.policy.allowed_targets.contains(target)
        {
            return Err(invalid("query.target", "target is not allowed"));
        }
        let targets: Vec<String> = query.target.clone().map_or_else(
            || self.policy.allowed_targets.iter().cloned().collect(),
            |target| vec![target],
        );
        let mut events = Vec::new();
        let mut truncated = false;
        let deadline = Instant::now() + self.query_timeout;
        for target in targets {
            if Instant::now() >= deadline {
                return Err(query_timeout_failure());
            }
            let mut target_query = query.clone();
            target_query.target = Some(target);
            let operation = self.backend.start_query(target_query)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(query_timeout_failure());
            }
            let snapshot = operation.completion(remaining).await?;
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
        let request: sc_observability_dto::FlushRequest =
            parse(value, &self.policy, "request", "flush")?;
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
    plugin_with_settings(backend, AdapterSettings::from(policy))
}

/// Registers the plugin with an explicit query observation deadline.
#[cfg(feature = "tauri")]
pub fn plugin_with_settings<R: tauri::Runtime>(
    backend: Arc<dyn HostLoggingBackend>,
    settings: AdapterSettings,
) -> Result<tauri::plugin::TauriPlugin<R>, Failure> {
    let adapter = Adapter::with_settings(backend, settings)?;
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
    window: tauri::WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    request: Value,
) -> Result<WireEnvelope<AdmissionDto>, WireEnvelope<AdmissionDto>> {
    // Admission calls can enter native code and are synchronous by contract;
    // keep them off Tauri's async executor even though the command is async.
    let adapter = app.state::<ManagedAdapter>().0.clone();
    let label = window.label().to_owned();
    let result = tauri::async_runtime::spawn_blocking(move || adapter.try_log(&label, request))
        .await
        .unwrap_or_else(|_| envelope(Err(blocking_task_failure())));
    Ok(result)
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn sc_observability_query<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    request: Value,
) -> WireEnvelope<LogSnapshotDto> {
    app.state::<ManagedAdapter>()
        .0
        .query(window.label(), request)
        .await
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn sc_observability_health<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    request: Value,
) -> Result<WireEnvelope<LogHealthDto>, WireEnvelope<LogHealthDto>> {
    // Native health may acquire backend locks; it belongs on the blocking
    // pool, not on the async executor thread.
    let adapter = app.state::<ManagedAdapter>().0.clone();
    let label = window.label().to_owned();
    let result = tauri::async_runtime::spawn_blocking(move || adapter.health(&label, request))
        .await
        .unwrap_or_else(|_| envelope(Err(blocking_task_failure())));
    Ok(result)
}

#[cfg(feature = "tauri")]
#[tauri::command]
async fn sc_observability_flush<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    request: Value,
) -> WireEnvelope<sc_observability_dto::CompletionDto> {
    app.state::<ManagedAdapter>()
        .0
        .flush(window.label(), request)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_binding_runtime::{Operation, ProducerOrigin};
    use sc_observability_dto::{CompletionDto, Diagnostic as DiagnosticDto, LogQueryDto};

    fn expected_diagnostic(diagnostic: &sc_observability_types::Diagnostic) -> DiagnosticDto {
        DiagnosticDto {
            at: diagnostic.timestamp.to_string(),
            code: diagnostic.code.as_str().to_owned(),
            message: diagnostic.message.clone(),
            remediation: diagnostic.remediation.clone().into(),
        }
    }

    fn assert_event_projection(error: &v2::EventError, expected: &Failure) {
        assert_eq!(&project_canonical_event(error), expected);
        assert_eq!(
            &project_canonical_event(error),
            &sc_observability_dto::failure_from_classification(
                error.diagnostic(),
                error.failure_classification(),
            )
        );
    }

    fn assert_flush_projection(error: &v2::FlushError, expected: &Failure) {
        assert_eq!(&project_canonical_flush(error), expected);
        assert_eq!(
            &project_canonical_flush(error),
            &sc_observability_dto::failure_from_classification(
                error.diagnostic(),
                error.failure_classification(),
            )
        );
    }

    #[test]
    fn canonical_event_failures_preserve_native_classification_and_diagnostics() {
        let validation = v2::EventError::Validation {
            context: Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::VALUE_VALIDATION_FAILED,
                "invalid event",
                sc_observability_types::Remediation::recoverable(
                    "correct the event",
                    [] as [&str; 0],
                ),
            )),
        };
        assert_event_projection(
            &validation,
            &Failure::Validation {
                diagnostic: Box::new(expected_diagnostic(validation.diagnostic())),
                field: "event".to_owned(),
            },
        );

        let queue_full = v2::EventError::classified_routing(
            Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::VALUE_VALIDATION_FAILED,
                "queue full",
                sc_observability_types::Remediation::recoverable(
                    "drain the queue",
                    [] as [&str; 0],
                ),
            )),
            v2::FailureClassification::QueueFull,
        );
        assert_event_projection(
            &queue_full,
            &Failure::QueueFull {
                diagnostic: Box::new(expected_diagnostic(queue_full.diagnostic())),
            },
        );

        let unavailable = v2::EventError::classified_routing(
            Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::VALUE_VALIDATION_FAILED,
                "writer is unavailable",
                sc_observability_types::Remediation::recoverable(
                    "restart the writer",
                    [] as [&str; 0],
                ),
            )),
            v2::FailureClassification::Unavailable,
        );
        assert_event_projection(
            &unavailable,
            &Failure::Unavailable {
                diagnostic: Box::new(expected_diagnostic(unavailable.diagnostic())),
            },
        );
    }

    #[test]
    fn canonical_flush_failures_preserve_native_classification_and_diagnostics() {
        let timeout = v2::FlushError::classified_drain(
            Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::SC_LOG_QUERY_IO,
                "flush deadline elapsed",
                sc_observability_types::Remediation::recoverable("retry flush", [] as [&str; 0]),
            )),
            v2::FailureClassification::timeout("flush"),
        );
        assert_flush_projection(
            &timeout,
            &Failure::Timeout {
                diagnostic: Box::new(expected_diagnostic(timeout.diagnostic())),
                operation: "flush".to_owned(),
            },
        );

        let export_cause = v2::FlushError::Drain {
            context: Box::new(
                sc_observability_types::ErrorContext::new(
                    sc_observability_types::error_codes::SC_LOG_QUERY_IO,
                    "flush transport queue is full",
                    sc_observability_types::Remediation::recoverable(
                        "drain the exporter queue",
                        [] as [&str; 0],
                    ),
                )
                .source(Box::new(v2::ExportError::QueueFull {
                    context: Box::new(sc_observability_types::ErrorContext::new(
                        sc_observability_types::error_codes::SC_LOG_QUERY_IO,
                        "export queue is full",
                        sc_observability_types::Remediation::recoverable(
                            "retry export",
                            [] as [&str; 0],
                        ),
                    )),
                })),
            ),
        };
        assert_flush_projection(
            &export_cause,
            &Failure::QueueFull {
                diagnostic: Box::new(expected_diagnostic(export_cause.diagnostic())),
            },
        );

        let fallback = v2::FlushError::Drain {
            context: Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::SC_LOG_QUERY_IO,
                "flush failed without a native classification",
                sc_observability_types::Remediation::recoverable("retry flush", [] as [&str; 0]),
            )),
        };
        assert_flush_projection(
            &fallback,
            &Failure::Io {
                diagnostic: Box::new(expected_diagnostic(fallback.diagnostic())),
            },
        );
    }

    struct IpcBackend;

    impl HostLoggingBackend for IpcBackend {
        fn try_log(&self, _: LogEventDto, _: ProducerOrigin) -> Result<AdmissionDto, Failure> {
            Err(Failure::Internal {
                diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                    sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                    "test backend",
                )),
            })
        }
        fn start_query(&self, _: LogQueryDto) -> Result<Operation<LogSnapshotDto>, Failure> {
            Err(Failure::Internal {
                diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                    sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                    "test backend",
                )),
            })
        }
        fn health(&self) -> Result<LogHealthDto, Failure> {
            Err(Failure::Internal {
                diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                    sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                    "test backend",
                )),
            })
        }
        fn start_flush(&self, _: Duration) -> Result<Operation<CompletionDto>, Failure> {
            Err(Failure::Internal {
                diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                    sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                    "test backend",
                )),
            })
        }
    }

    #[derive(Clone)]
    struct NativeProjectionBackend {
        event_failure: Failure,
        flush_failure: Failure,
    }

    impl HostLoggingBackend for NativeProjectionBackend {
        fn try_log(&self, _: LogEventDto, _: ProducerOrigin) -> Result<AdmissionDto, Failure> {
            Err(self.event_failure.clone())
        }

        fn start_query(&self, _: LogQueryDto) -> Result<Operation<LogSnapshotDto>, Failure> {
            Err(self.event_failure.clone())
        }

        fn health(&self) -> Result<LogHealthDto, Failure> {
            Err(self.event_failure.clone())
        }

        fn start_flush(&self, _: Duration) -> Result<Operation<CompletionDto>, Failure> {
            Err(self.flush_failure.clone())
        }
    }

    fn policy() -> AdapterPolicy {
        AdapterPolicy {
            allowed_window_labels: BTreeSet::from(["main".to_owned()]),
            allowed_targets: BTreeSet::from(["app".to_owned()]),
            max_request_bytes: u32::try_from(MAX_WIRE_PAYLOAD_BYTES)
                .expect("wire payload limit fits in u32"),
            max_depth: u32::try_from(MAX_CONTAINER_DEPTH)
                .expect("container depth limit fits in u32"),
            redacted_field_keys: BTreeSet::new(),
        }
    }

    fn event_request() -> Value {
        serde_json::json!({
            "schema_version": 1,
            "event": {
                "schema_version": 1,
                "level": "info",
                "target": "app",
                "action": "test.emit"
            }
        })
    }

    fn flush_request() -> Value {
        serde_json::json!({"schema_version": 1, "timeout_ms": 1})
    }

    #[test]
    fn adapter_try_log_preserves_native_event_projection_through_the_public_envelope() {
        let native = v2::EventError::classified_routing(
            Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::VALUE_VALIDATION_FAILED,
                "native queue is full",
                sc_observability_types::Remediation::recoverable(
                    "drain the queue",
                    [] as [&str; 0],
                ),
            )),
            v2::FailureClassification::QueueFull,
        );
        let expected = sc_observability_dto::failure_from_classification(
            native.diagnostic(),
            native.failure_classification(),
        );
        let adapter = Adapter::new(
            Arc::new(NativeProjectionBackend {
                event_failure: expected.clone(),
                flush_failure: Failure::Internal {
                    diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                        sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                        "unused flush failure",
                    )),
                },
            }),
            policy(),
        )
        .expect("valid adapter policy");

        assert_eq!(
            adapter.try_log("main", event_request()),
            WireEnvelope::Error {
                schema_version: SCHEMA_VERSION,
                error: expected,
            }
        );
    }

    #[cfg(feature = "tauri")]
    #[test]
    fn adapter_flush_preserves_native_flush_projection_through_the_public_envelope() {
        let native = v2::FlushError::classified_drain(
            Box::new(sc_observability_types::ErrorContext::new(
                sc_observability_types::error_codes::SC_LOG_QUERY_IO,
                "native flush deadline elapsed",
                sc_observability_types::Remediation::recoverable(
                    "retry flush",
                    [] as [&str; 0],
                ),
            )),
            v2::FailureClassification::timeout("flush"),
        );
        let expected = sc_observability_dto::failure_from_classification(
            native.diagnostic(),
            native.failure_classification(),
        );
        let adapter = Adapter::new(
            Arc::new(NativeProjectionBackend {
                event_failure: Failure::Internal {
                    diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
                        sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                        "unused event failure",
                    )),
                },
                flush_failure: expected.clone(),
            }),
            policy(),
        )
        .expect("valid adapter policy");

        assert_eq!(
            tauri::async_runtime::block_on(adapter.flush("main", flush_request())),
            WireEnvelope::Error {
                schema_version: SCHEMA_VERSION,
                error: expected,
            }
        );
    }
    #[test]
    fn policy_rejects_bad_limits_and_provenance() {
        let policy = AdapterPolicy {
            allowed_window_labels: ["main".into()].into(),
            allowed_targets: ["app".into()].into(),
            max_request_bytes: u32::try_from(MAX_WIRE_PAYLOAD_BYTES)
                .expect("wire payload limit fits in u32")
                .checked_add(1)
                .expect("one byte over the wire payload limit fits in u32"),
            max_depth: u32::try_from(MAX_CONTAINER_DEPTH)
                .expect("container depth limit fits in u32"),
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
                max_request_bytes: MAX_WIRE_PAYLOAD_BYTES as u32,
                max_depth: MAX_CONTAINER_DEPTH as u32,
                redacted_field_keys: BTreeSet::new(),
            }
        };
        assert!(
            matches!(policy.validate(), Err(Failure::Validation { ref field, .. }) if field == "policy.allowed_window_labels")
        );
        let policy = AdapterPolicy {
            allowed_targets: ["bad target".into()].into(),
            ..AdapterPolicy {
                allowed_window_labels: ["main".into()].into(),
                allowed_targets: ["app".into()].into(),
                max_request_bytes: MAX_WIRE_PAYLOAD_BYTES as u32,
                max_depth: MAX_CONTAINER_DEPTH as u32,
                redacted_field_keys: BTreeSet::new(),
            }
        };
        assert!(matches!(
            policy.validate(),
            Err(Failure::Validation { ref field, ref diagnostic })
                if field == "policy.allowed_targets"
                    && diagnostic
                        .message
                        .contains("identifier must match [A-Za-z0-9._-]+")
        ));
    }

    #[test]
    fn parse_preserves_serde_diagnostic() {
        let policy = AdapterPolicy {
            allowed_window_labels: BTreeSet::from(["main".to_owned()]),
            allowed_targets: BTreeSet::from(["app".to_owned()]),
            max_request_bytes: MAX_WIRE_PAYLOAD_BYTES as u32,
            max_depth: MAX_CONTAINER_DEPTH as u32,
            redacted_field_keys: BTreeSet::new(),
        };
        let result = parse::<QueryRequest>(
            serde_json::json!({
                "schema_version": 1,
                "query": {"schema_version": 1, "limit": "not an integer"}
            }),
            &policy,
            "request",
            "query",
        );

        assert!(matches!(
            result,
            Err(Failure::Validation { ref diagnostic, .. })
                if diagnostic.message.contains("invalid type")
        ));
    }

    #[test]
    fn adapter_settings_select_query_timeout_without_changing_host_policy() {
        let settings = AdapterSettings {
            policy: AdapterPolicy {
                allowed_window_labels: ["main".into()].into(),
                allowed_targets: ["app".into()].into(),
                max_request_bytes: MAX_WIRE_PAYLOAD_BYTES as u32,
                max_depth: MAX_CONTAINER_DEPTH as u32,
                redacted_field_keys: BTreeSet::new(),
            },
            query_timeout_ms: 17,
        };
        let adapter = Adapter::with_settings(Arc::new(IpcBackend), settings).unwrap();
        assert_eq!(adapter.query_timeout, Duration::from_millis(17));
    }

    #[test]
    fn query_target_count_is_bounded() {
        let policy = AdapterPolicy {
            allowed_window_labels: ["main".into()].into(),
            allowed_targets: (0..=MAX_QUERY_TARGETS)
                .map(|index| format!("app.{index}"))
                .collect(),
            max_request_bytes: MAX_WIRE_PAYLOAD_BYTES as u32,
            max_depth: MAX_CONTAINER_DEPTH as u32,
            redacted_field_keys: BTreeSet::new(),
        };
        assert!(
            matches!(policy.validate(), Err(Failure::Validation { ref field, .. }) if field == "policy.allowed_targets")
        );
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

        assert!(
            inspect(
                &nested_objects(MAX_CONTAINER_DEPTH - 1, Value::Object(Default::default())),
                0,
                MAX_CONTAINER_DEPTH
            )
            .is_ok()
        );
        assert!(
            inspect(
                &nested_objects(MAX_CONTAINER_DEPTH, Value::Null),
                0,
                MAX_CONTAINER_DEPTH
            )
            .is_ok()
        );
        assert!(
            inspect(
                &nested_objects(MAX_CONTAINER_DEPTH, Value::Object(Default::default())),
                0,
                MAX_CONTAINER_DEPTH
            )
            .is_err()
        );
    }

    #[test]
    fn exact_request_limit_does_not_reject_omitted_nullable_event_fields() {
        let policy = AdapterPolicy {
            allowed_window_labels: BTreeSet::from(["main".to_owned()]),
            allowed_targets: BTreeSet::from(["app".to_owned()]),
            max_request_bytes: MAX_WIRE_PAYLOAD_BYTES as u32,
            max_depth: MAX_CONTAINER_DEPTH as u32,
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
        request["event"]["message"] =
            serde_json::json!("x".repeat(MAX_WIRE_PAYLOAD_BYTES - overhead));
        assert_eq!(
            serde_json::to_vec(&request).unwrap().len(),
            MAX_WIRE_PAYLOAD_BYTES
        );
        let result = adapter.try_log("main", request);
        assert!(matches!(
            result,
            WireEnvelope::Error {
                error: Failure::Internal { .. },
                ..
            }
        ));
    }

    #[test]
    fn redaction_replaces_nested_keys() {
        let mut value = sc_observability_dto::ValueDto::Object {
            value: [(
                "password".to_owned(),
                sc_observability_dto::ValueDto::String {
                    value: "secret".to_owned(),
                },
            )]
            .into_iter()
            .collect(),
        };
        redact_value(&mut value, &BTreeSet::from(["password".to_owned()]));
        assert_eq!(
            value,
            sc_observability_dto::ValueDto::Object {
                value: [(
                    "password".to_owned(),
                    sc_observability_dto::ValueDto::String {
                        value: REDACTED.to_owned(),
                    }
                )]
                .into_iter()
                .collect(),
            }
        );
    }

    #[cfg(feature = "tauri")]
    #[test]
    fn mock_ipc_returns_wire_envelope_from_registered_command() {
        let policy = AdapterPolicy {
            allowed_window_labels: BTreeSet::from(["main".to_owned()]),
            allowed_targets: BTreeSet::from(["app".to_owned()]),
            max_request_bytes: MAX_WIRE_PAYLOAD_BYTES as u32,
            max_depth: MAX_CONTAINER_DEPTH as u32,
            redacted_field_keys: BTreeSet::new(),
        };
        let app = tauri::test::mock_builder()
            .manage(ManagedAdapter(
                Adapter::new(Arc::new(IpcBackend), policy).unwrap(),
            ))
            .invoke_handler(tauri::generate_handler![
                sc_observability_try_log,
                sc_observability_health
            ])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
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
        )
        .unwrap();
        let value = response.deserialize::<Value>().unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["kind"], "error");

        let response = tauri::test::get_ipc_response(
            &window,
            tauri::webview::InvokeRequest {
                cmd: "sc_observability_try_log".into(),
                callback: tauri::ipc::CallbackFn(2),
                error: tauri::ipc::CallbackFn(3),
                url: window.url().unwrap(),
                body: serde_json::json!({
                    "request": {
                        "schema_version": 1,
                        "event": {
                            "schema_version": 1,
                            "level": "info",
                            "target": "app",
                            "action": "test",
                            "message": "test",
                            "fields": {}
                        }
                    }
                })
                .into(),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_owned(),
            },
        )
        .unwrap();
        let value = response.deserialize::<Value>().unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["kind"], "error");
    }
}
