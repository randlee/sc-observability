//! Field-specific configuration precedence without file or environment discovery.
use super::{TelemetryConfigError, error_codes, errors::context};
use crate::constants;
use serde::Deserialize;
use std::{fmt, path::PathBuf, time::Duration};

/// Secret value; Debug and Display never reveal its contents.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);
impl Secret {
    /// Wraps a credential supplied by the caller or environment.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }
    /// Exposes the credential only for transport construction.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}
impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}
/// Transport implementation selected by the caller.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExporterBackendId {
    /// Runtime-free synchronous HTTP.
    #[default]
    SyncHttp,
    /// Official SDK with caller-owned runtime.
    OpenTelemetrySdk,
}
/// Explicit behavior when the durable byte bound is reached.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskBoundPolicy {
    /// Reject new admissions without deleting pending work.
    #[default]
    RejectNew,
    /// Explicitly evict oldest pending submissions.
    EvictOldest,
}

/// Caller-supplied fields; absent values defer to eligible lower-precedence sources.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    /// Configuration `service_name` field.
    pub service_name: Option<String>,
    /// Configuration `endpoint` field.
    pub endpoint: Option<String>,
    /// Configuration `backend` field.
    pub backend: Option<ExporterBackendId>,
    /// Configuration `request_timeout` field.
    pub request_timeout: Option<Duration>,
    /// Configuration `auth_header` field.
    pub auth_header: Option<Secret>,
    /// Configuration `store_path` field.
    pub store_path: Option<PathBuf>,
    /// Configuration `max_store_bytes` field.
    pub max_store_bytes: Option<u64>,
    /// Configuration `disk_bound_policy` field.
    pub disk_bound_policy: Option<DiskBoundPolicy>,
    /// Configuration `delivered_retention` field.
    pub delivered_retention: Option<Duration>,
    /// Configuration `record_key_retention` field.
    pub record_key_retention: Option<Duration>,
    /// Configuration `emit_flush_deadline` field.
    pub emit_flush_deadline: Option<Duration>,
    /// Configuration `flush_deadline` field.
    pub flush_deadline: Option<Duration>,
    /// Configuration `lease_duration` field.
    pub lease_duration: Option<Duration>,
    /// Configuration `sync_http_retry` field.
    pub sync_http_retry: Option<SyncHttpRetryPolicyDto>,
}

/// Resolved telemetry configuration shared by both front ends.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TelemetryClientConfig {
    /// Configuration `service_name` field.
    pub service_name: String,
    /// Configuration `endpoint` field.
    pub endpoint: String,
    /// Configuration `backend` field.
    pub backend: ExporterBackendId,
    /// Configuration `request_timeout` field.
    pub request_timeout: Duration,
    /// Configuration `auth_header` field.
    pub auth_header: Option<Secret>,
    /// Configuration `store_path` field.
    pub store_path: PathBuf,
    /// Configuration `max_store_bytes` field.
    pub max_store_bytes: u64,
    /// Configuration `disk_bound_policy` field.
    pub disk_bound_policy: DiskBoundPolicy,
    /// Configuration `delivered_retention` field.
    pub delivered_retention: Duration,
    /// Configuration `record_key_retention` field.
    pub record_key_retention: Duration,
    /// Configuration `emit_flush_deadline` field.
    pub emit_flush_deadline: Duration,
    /// Configuration `flush_deadline` field.
    pub flush_deadline: Duration,
    /// Configuration `lease_duration` field.
    pub lease_duration: Duration,
    /// Configuration `sync_http_retry` field.
    pub sync_http_retry: Option<SyncHttpRetryPolicyDto>,
}

/// Field-for-field retry overrides; durations are milliseconds.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncHttpRetryPolicyDto {
    /// Additional attempts after the initial request.
    pub max_retries: Option<u32>,
    /// Initial exponential backoff.
    pub initial_backoff_ms: Option<u64>,
    /// Backoff ceiling.
    pub max_backoff_ms: Option<u64>,
    /// Entire retry sequence budget.
    pub retry_sequence_timeout_ms: Option<u64>,
    /// Cap on collector Retry-After delays.
    pub retry_after_cap_ms: Option<u64>,
    /// Jitter percentage.
    pub retry_jitter_percent: Option<u8>,
}
/// Core YAML keys; consumer-owned keys are deliberately ignored.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TelemetryFileConfig {
    /// Set by the loader to the containing directory, never read from YAML.
    #[serde(skip)]
    pub base_dir: PathBuf,
    /// Producer service.
    pub service: Option<String>,
    /// OTLP endpoint and timeout.
    pub otlp: Option<FileOtlp>,
    /// Store location and bounds.
    pub store: Option<FileStore>,
}
/// Core OTLP file keys.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileOtlp {
    /// Base endpoint URL.
    pub endpoint: Option<String>,
    /// Per-request milliseconds.
    pub timeout_ms: Option<u64>,
}
/// Core durable store file keys.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileStore {
    /// Store path, relative to YAML directory when not absolute.
    pub path: Option<PathBuf>,
    /// Durable byte bound.
    pub max_bytes: Option<u64>,
    /// Explicit bound policy.
    pub disk_bound_policy: Option<DiskBoundPolicy>,
    /// Hours to retain delivered records.
    pub delivered_retention_hours: Option<u64>,
}
/// Explicit, file and injectable environment sources.
#[non_exhaustive]
#[derive(Clone, Copy)]
pub struct ConfigSources<'a> {
    /// Highest-priority caller input.
    pub explicit: &'a ConfigOverrides,
    /// Parsed file, if supplied.
    pub file: Option<&'a TelemetryFileConfig>,
    /// Injectable environment lookup; only the three approved keys are read.
    pub env: &'a dyn Fn(&str) -> Option<String>,
}
impl fmt::Debug for ConfigSources<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ConfigSources")
            .field("explicit", self.explicit)
            .field("file", &self.file)
            .finish_non_exhaustive()
    }
}
impl<'a> ConfigSources<'a> {
    /// Combines sources without performing environment or filesystem discovery.
    #[must_use]
    pub fn new(
        explicit: &'a ConfigOverrides,
        file: Option<&'a TelemetryFileConfig>,
        env: &'a dyn Fn(&str) -> Option<String>,
    ) -> Self {
        Self {
            explicit,
            file,
            env,
        }
    }
}
/// Resolves only each field's allowed sources: explicit, file, environment, then default.
/// # Errors
/// Rejects missing store paths, zero bounds/deadlines, malformed endpoint/auth, and overflow.
pub fn resolve_config(
    sources: ConfigSources<'_>,
) -> Result<TelemetryClientConfig, TelemetryConfigError> {
    let e = sources.explicit;
    let f = sources.file;
    let otlp = f.and_then(|f| f.otlp.as_ref());
    let store = f.and_then(|f| f.store.as_ref());
    let store_path = e
        .store_path
        .clone()
        .or_else(|| {
            store.and_then(|s| s.path.clone()).map(|p| {
                if p.is_absolute() {
                    p
                } else {
                    f.map_or(p.clone(), |f| f.base_dir.join(p))
                }
            })
        })
        .ok_or_else(|| TelemetryConfigError::MissingField {
            field: "store_path",
            context: context(
                error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_MISSING,
                "store_path is required",
            ),
        })?;
    let retained = if let Some(d) = e.delivered_retention {
        d
    } else if let Some(hours) = store.and_then(|s| s.delivered_retention_hours) {
        Duration::from_secs(
            hours
                .checked_mul(constants::TELEMETRY_SECONDS_PER_HOUR)
                .ok_or_else(|| invalid("delivered_retention"))?,
        )
    } else {
        constants::TELEMETRY_DELIVERED_RETENTION
    };
    let result = TelemetryClientConfig {
        service_name: e
            .service_name
            .clone()
            .or_else(|| f.and_then(|f| f.service.clone()))
            .or_else(|| (sources.env)("OTEL_SERVICE_NAME"))
            .unwrap_or_else(|| constants::TELEMETRY_DEFAULT_SERVICE.into()),
        endpoint: e
            .endpoint
            .clone()
            .or_else(|| otlp.and_then(|o| o.endpoint.clone()))
            .or_else(|| (sources.env)("OTEL_EXPORTER_OTLP_ENDPOINT"))
            .unwrap_or_else(|| constants::TELEMETRY_DEFAULT_ENDPOINT.into()),
        backend: e.backend.unwrap_or_default(),
        request_timeout: e
            .request_timeout
            .or_else(|| otlp.and_then(|o| o.timeout_ms.map(Duration::from_millis)))
            .unwrap_or(constants::TELEMETRY_REQUEST_TIMEOUT),
        auth_header: e
            .auth_header
            .clone()
            .or_else(|| (sources.env)("SC_OTEL_AUTH_HEADER").map(Secret::new)),
        store_path,
        max_store_bytes: e
            .max_store_bytes
            .or_else(|| store.and_then(|s| s.max_bytes))
            .unwrap_or(constants::TELEMETRY_MAX_STORE_BYTES),
        disk_bound_policy: e
            .disk_bound_policy
            .or_else(|| store.and_then(|s| s.disk_bound_policy))
            .unwrap_or_default(),
        delivered_retention: retained,
        record_key_retention: e
            .record_key_retention
            .unwrap_or(constants::TELEMETRY_RECORD_KEY_RETENTION),
        emit_flush_deadline: e
            .emit_flush_deadline
            .unwrap_or(constants::TELEMETRY_EMIT_FLUSH_DEADLINE),
        flush_deadline: e
            .flush_deadline
            .unwrap_or(constants::TELEMETRY_FLUSH_DEADLINE),
        lease_duration: e
            .lease_duration
            .unwrap_or(constants::TELEMETRY_LEASE_DURATION),
        sync_http_retry: e.sync_http_retry.clone(),
    };
    result.validate()?;
    Ok(result)
}
fn invalid(field: &'static str) -> TelemetryConfigError {
    TelemetryConfigError::InvalidField {
        field,
        context: context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
            format!("invalid {field}"),
        ),
    }
}
impl TelemetryClientConfig {
    /// Checks resolved bounds without exposing credential values.
    /// # Errors
    /// Rejects empty identities/paths, invalid base URLs/headers, and zero bounds.
    pub fn validate(&self) -> Result<(), TelemetryConfigError> {
        if self.service_name.trim().is_empty() {
            return Err(invalid("service_name"));
        }
        if self.store_path.as_os_str().is_empty() {
            return Err(invalid("store_path"));
        }
        let host = self
            .endpoint
            .strip_prefix("http://")
            .or_else(|| self.endpoint.strip_prefix("https://"))
            .ok_or_else(|| invalid("endpoint"))?;
        if host.is_empty()
            || host.starts_with('/')
            || host.contains('@')
            || host.contains('#')
            || host.contains('?')
            || host.chars().any(char::is_whitespace)
        {
            return Err(invalid("endpoint"));
        }
        if self
            .auth_header
            .as_ref()
            .is_some_and(|h| h.expose().bytes().any(|b| b < 0x20 || b == 0x7f))
        {
            return Err(invalid("auth_header"));
        }
        if self.max_store_bytes == 0 {
            return Err(invalid("max_store_bytes"));
        }
        for (name, value) in [
            ("request_timeout", self.request_timeout),
            ("delivered_retention", self.delivered_retention),
            ("record_key_retention", self.record_key_retention),
            ("emit_flush_deadline", self.emit_flush_deadline),
            ("flush_deadline", self.flush_deadline),
            ("lease_duration", self.lease_duration),
        ] {
            if value.is_zero() {
                return Err(invalid(name));
            }
        }
        if let Some(retry) = &self.sync_http_retry {
            if retry.retry_jitter_percent.is_some_and(|v| v > 100) {
                return Err(invalid("sync_http_retry.retry_jitter_percent"));
            }
            if let (Some(initial), Some(max)) = (retry.initial_backoff_ms, retry.max_backoff_ms)
                && initial > max
            {
                return Err(invalid("sync_http_retry.max_backoff_ms"));
            }
        }
        Ok(())
    }
}
