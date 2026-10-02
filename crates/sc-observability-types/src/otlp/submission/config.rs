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
///
/// When no submission request timeout is supplied, this entry point uses the
/// 10-second `TELEMETRY_REQUEST_TIMEOUT` default. The released facade's
/// `OtelConfig` default remains 3 seconds; choosing one entry point does not
/// change the other's default.
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
                .ok_or_else(|| invalid("delivered_retention", "hours overflow a duration"))?,
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
fn invalid(field: &'static str, reason: impl fmt::Display) -> TelemetryConfigError {
    TelemetryConfigError::InvalidField {
        field,
        context: context(
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
            format!("invalid {field}: {reason}"),
        ),
    }
}
/// Rejects a duration outside `TELEMETRY_DURATION_MIN..=max`.
fn check_duration(
    field: &'static str,
    value: Duration,
    max: Duration,
) -> Result<(), TelemetryConfigError> {
    if value < constants::TELEMETRY_DURATION_MIN || value > max {
        return Err(invalid(
            field,
            format_args!(
                "{value:?} is outside {:?}..={max:?}",
                constants::TELEMETRY_DURATION_MIN
            ),
        ));
    }
    Ok(())
}
/// Rejects an explicitly supplied retry delay outside `1..=TELEMETRY_RETRY_DURATION_MAX_MS`.
fn check_retry_ms(field: &'static str, value: Option<u64>) -> Result<(), TelemetryConfigError> {
    match value {
        Some(ms) if ms == 0 || ms > constants::TELEMETRY_RETRY_DURATION_MAX_MS => Err(invalid(
            field,
            format_args!(
                "{ms} ms is outside 1..={} ms",
                constants::TELEMETRY_RETRY_DURATION_MAX_MS
            ),
        )),
        _ => Ok(()),
    }
}
/// Requires `lower <= upper` for two supplied values.
fn check_order<T: PartialOrd + fmt::Debug>(
    field: &'static str,
    lower: (&str, Option<T>),
    upper: (&str, Option<T>),
) -> Result<(), TelemetryConfigError> {
    if let ((lower_name, Some(l)), (upper_name, Some(u))) = (lower, upper)
        && l > u
    {
        return Err(invalid(
            field,
            format_args!("{lower_name} {l:?} must not exceed {upper_name} {u:?}"),
        ));
    }
    Ok(())
}
/// Parses an `http(s)://host[:port][/path]` base URL: non-empty host,
/// port in `1..=65535`, no userinfo, query, fragment or whitespace.
fn check_endpoint(endpoint: &str) -> Result<(), &'static str> {
    let rest = endpoint
        .strip_prefix("http://")
        .or_else(|| endpoint.strip_prefix("https://"))
        .ok_or("scheme must be http:// or https://")?;
    if endpoint
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("must not contain whitespace or control characters");
    }
    if rest.contains(['?', '#']) {
        return Err("must not contain a query or fragment");
    }
    let authority = rest
        .split_once('/')
        .map_or(rest, |(authority, _)| authority);
    if authority.contains('@') {
        return Err("must not contain userinfo");
    }
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (address, after) = bracketed
            .split_once(']')
            .ok_or("IPv6 host is missing its closing bracket")?;
        address
            .parse::<std::net::Ipv6Addr>()
            .map_err(|_| "IPv6 host is not a valid address")?;
        let port = match after {
            "" => None,
            _ => Some(
                after
                    .strip_prefix(':')
                    .ok_or("unexpected text after IPv6 host")?,
            ),
        };
        (address, port)
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if host.is_empty() {
        return Err("host must not be empty");
    }
    if !authority.starts_with('[')
        && !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
    {
        return Err("host contains a character outside the URL host grammar");
    }
    if let Some(port) = port
        && (port.is_empty()
            || !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u16>().map_or(true, |p| p == 0))
    {
        return Err("port must be a number in 1..=65535");
    }
    Ok(())
}
impl TelemetryClientConfig {
    /// Checks resolved bounds without exposing credential values.
    /// # Errors
    /// Rejects empty identities/paths, invalid base URLs/headers, out-of-range
    /// bounds and inconsistent deadline or retry ordering.
    pub fn validate(&self) -> Result<(), TelemetryConfigError> {
        if self.service_name.trim().is_empty() {
            return Err(invalid("service_name", "must not be blank"));
        }
        if self.store_path.as_os_str().is_empty() {
            return Err(invalid("store_path", "must not be empty"));
        }
        check_endpoint(&self.endpoint).map_err(|reason| invalid("endpoint", reason))?;
        if self
            .auth_header
            .as_ref()
            .is_some_and(|h| h.expose().bytes().any(|b| b.is_ascii_control()))
        {
            return Err(invalid("auth_header", "must not contain control bytes"));
        }
        if self.max_store_bytes == 0
            || self.max_store_bytes > constants::TELEMETRY_MAX_STORE_BYTES_LIMIT
        {
            return Err(invalid(
                "max_store_bytes",
                format_args!(
                    "{} is outside 1..={}",
                    self.max_store_bytes,
                    constants::TELEMETRY_MAX_STORE_BYTES_LIMIT
                ),
            ));
        }
        self.validate_durations()?;
        if let Some(retry) = &self.sync_http_retry {
            self.validate_retry(retry)?;
        }
        Ok(())
    }
    /// Duration bounds plus the deadline and lease ordering rules.
    fn validate_durations(&self) -> Result<(), TelemetryConfigError> {
        for (name, value, max) in [
            (
                "request_timeout",
                self.request_timeout,
                constants::TELEMETRY_REQUEST_TIMEOUT_MAX,
            ),
            (
                "delivered_retention",
                self.delivered_retention,
                constants::TELEMETRY_RETENTION_MAX,
            ),
            (
                "record_key_retention",
                self.record_key_retention,
                constants::TELEMETRY_RETENTION_MAX,
            ),
            (
                "emit_flush_deadline",
                self.emit_flush_deadline,
                constants::TELEMETRY_DEADLINE_MAX,
            ),
            (
                "flush_deadline",
                self.flush_deadline,
                constants::TELEMETRY_DEADLINE_MAX,
            ),
            (
                "lease_duration",
                self.lease_duration,
                constants::TELEMETRY_DEADLINE_MAX,
            ),
        ] {
            check_duration(name, value, max)?;
        }
        check_order(
            "emit_flush_deadline",
            ("emit_flush_deadline", Some(self.emit_flush_deadline)),
            ("flush_deadline", Some(self.flush_deadline)),
        )?;
        if self.lease_duration <= self.request_timeout {
            return Err(invalid(
                "lease_duration",
                format_args!(
                    "{:?} must exceed request_timeout {:?} so a lease outlives one export",
                    self.lease_duration, self.request_timeout
                ),
            ));
        }
        Ok(())
    }
    /// Retry bounds and ordering, matching the OTLP retry validator.
    fn validate_retry(&self, retry: &SyncHttpRetryPolicyDto) -> Result<(), TelemetryConfigError> {
        check_retry_ms(
            "sync_http_retry.initial_backoff_ms",
            retry.initial_backoff_ms,
        )?;
        check_retry_ms("sync_http_retry.max_backoff_ms", retry.max_backoff_ms)?;
        check_retry_ms(
            "sync_http_retry.retry_sequence_timeout_ms",
            retry.retry_sequence_timeout_ms,
        )?;
        check_retry_ms(
            "sync_http_retry.retry_after_cap_ms",
            retry.retry_after_cap_ms,
        )?;
        if let Some(jitter) = retry
            .retry_jitter_percent
            .filter(|v| *v > constants::TELEMETRY_RETRY_JITTER_PERCENT_MAX)
        {
            return Err(invalid(
                "sync_http_retry.retry_jitter_percent",
                format_args!(
                    "{jitter} is outside 0..={}",
                    constants::TELEMETRY_RETRY_JITTER_PERCENT_MAX
                ),
            ));
        }
        check_order(
            "sync_http_retry.max_backoff_ms",
            ("initial_backoff_ms", retry.initial_backoff_ms),
            ("max_backoff_ms", retry.max_backoff_ms),
        )?;
        check_order(
            "sync_http_retry.retry_after_cap_ms",
            ("retry_after_cap_ms", retry.retry_after_cap_ms),
            ("retry_sequence_timeout_ms", retry.retry_sequence_timeout_ms),
        )?;
        let request_ms = u64::try_from(self.request_timeout.as_millis()).unwrap_or(u64::MAX);
        check_order(
            "sync_http_retry.retry_sequence_timeout_ms",
            ("request_timeout_ms", Some(request_ms)),
            ("retry_sequence_timeout_ms", retry.retry_sequence_timeout_ms),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigOverrides, ConfigSources, Secret, SyncHttpRetryPolicyDto, TelemetryClientConfig,
        TelemetryConfigError, resolve_config,
    };
    use crate::constants;
    use std::time::Duration;

    fn valid() -> TelemetryClientConfig {
        let overrides = ConfigOverrides {
            store_path: Some("store.db".into()),
            ..ConfigOverrides::default()
        };
        resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap()
    }
    fn rejected(config: &TelemetryClientConfig) -> (&'static str, String) {
        match config.validate() {
            Err(TelemetryConfigError::InvalidField { field, context }) => {
                (field, context.diagnostic().message.clone())
            }
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }
    fn retry(edit: impl FnOnce(&mut SyncHttpRetryPolicyDto)) -> TelemetryClientConfig {
        let mut config = valid();
        let mut policy = SyncHttpRetryPolicyDto::default();
        edit(&mut policy);
        config.sync_http_retry = Some(policy);
        config
    }

    #[test]
    fn endpoint_is_parsed_as_a_base_url() {
        for endpoint in [
            "http://localhost:4318",
            "https://collector.example.com",
            "http://10.0.0.1:4318/base/path",
            "http://[::1]:4318",
            "http://otel_collector",
        ] {
            let mut config = valid();
            config.endpoint = endpoint.into();
            assert!(config.validate().is_ok(), "{endpoint}");
        }
        for (endpoint, reason) in [
            ("ftp://host", "scheme"),
            ("http://", "host must not be empty"),
            ("http://:80", "host must not be empty"),
            ("http://host:99999", "port"),
            ("http://host:0", "port"),
            ("http://host:abc", "port"),
            ("http://host:", "port"),
            ("http://[", "closing bracket"),
            ("http://[zz]:1", "IPv6"),
            ("http://user@host", "userinfo"),
            ("http://host/?q=1", "query"),
            ("http://host/#f", "fragment"),
            ("http://ho st", "whitespace"),
            ("http://ho^st", "host grammar"),
        ] {
            let mut config = valid();
            config.endpoint = endpoint.into();
            let (field, message) = rejected(&config);
            assert_eq!(field, "endpoint", "{endpoint}");
            assert!(message.contains(reason), "{endpoint}: {message}");
        }
    }

    #[test]
    fn each_bound_and_ordering_rule_rejects_with_a_reason() {
        let mut cases: Vec<(TelemetryClientConfig, &str)> = Vec::new();
        let mut c = valid();
        c.service_name = " ".into();
        cases.push((c, "service_name"));
        let mut c = valid();
        c.store_path = "".into();
        cases.push((c, "store_path"));
        let mut c = valid();
        c.auth_header = Some(Secret::new("Bearer \u{7f}".into()));
        cases.push((c, "auth_header"));
        let mut c = valid();
        c.max_store_bytes = 0;
        cases.push((c, "max_store_bytes"));
        let mut c = valid();
        c.max_store_bytes = constants::TELEMETRY_MAX_STORE_BYTES_LIMIT + 1;
        cases.push((c, "max_store_bytes"));
        let mut c = valid();
        c.request_timeout = Duration::ZERO;
        cases.push((c, "request_timeout"));
        let mut c = valid();
        c.request_timeout = constants::TELEMETRY_REQUEST_TIMEOUT_MAX + Duration::from_millis(1);
        cases.push((c, "request_timeout"));
        let mut c = valid();
        c.delivered_retention = constants::TELEMETRY_RETENTION_MAX + Duration::from_secs(1);
        cases.push((c, "delivered_retention"));
        let mut c = valid();
        c.record_key_retention = Duration::from_micros(1);
        cases.push((c, "record_key_retention"));
        let mut c = valid();
        c.flush_deadline = constants::TELEMETRY_DEADLINE_MAX + Duration::from_secs(1);
        cases.push((c, "flush_deadline"));
        let mut c = valid();
        c.emit_flush_deadline = c.flush_deadline + Duration::from_secs(1);
        cases.push((c, "emit_flush_deadline"));
        let mut c = valid();
        c.lease_duration = c.request_timeout;
        cases.push((c, "lease_duration"));
        cases.push((
            retry(|r| r.initial_backoff_ms = Some(0)),
            "sync_http_retry.initial_backoff_ms",
        ));
        cases.push((
            retry(|r| r.max_backoff_ms = Some(constants::TELEMETRY_RETRY_DURATION_MAX_MS + 1)),
            "sync_http_retry.max_backoff_ms",
        ));
        cases.push((
            retry(|r| r.retry_sequence_timeout_ms = Some(0)),
            "sync_http_retry.retry_sequence_timeout_ms",
        ));
        cases.push((
            retry(|r| r.retry_after_cap_ms = Some(0)),
            "sync_http_retry.retry_after_cap_ms",
        ));
        cases.push((
            retry(|r| {
                r.retry_jitter_percent = Some(constants::TELEMETRY_RETRY_JITTER_PERCENT_MAX + 1);
            }),
            "sync_http_retry.retry_jitter_percent",
        ));
        cases.push((
            retry(|r| {
                r.initial_backoff_ms = Some(20);
                r.max_backoff_ms = Some(10);
            }),
            "sync_http_retry.max_backoff_ms",
        ));
        cases.push((
            retry(|r| {
                r.retry_after_cap_ms = Some(60_000);
                r.retry_sequence_timeout_ms = Some(30_000);
            }),
            "sync_http_retry.retry_after_cap_ms",
        ));
        cases.push((
            retry(|r| r.retry_sequence_timeout_ms = Some(1)),
            "sync_http_retry.retry_sequence_timeout_ms",
        ));
        for (config, expected) in &cases {
            let (field, message) = rejected(config);
            assert_eq!(field, *expected);
            assert!(
                message.starts_with(&format!("invalid {expected}: "))
                    && message.len() > expected.len() + 10,
                "{expected}: {message}"
            );
        }
        let mut ordered = valid();
        ordered.sync_http_retry = Some(SyncHttpRetryPolicyDto::default());
        assert!(ordered.validate().is_ok());
    }

    #[test]
    fn file_hour_overflow_names_its_reason() {
        let file: super::TelemetryFileConfig = serde_json::from_str(
            r#"{"store":{"path":"s.db","delivered_retention_hours":18446744073709551615}}"#,
        )
        .unwrap();
        let overrides = ConfigOverrides::default();
        match resolve_config(ConfigSources::new(&overrides, Some(&file), &|_| None)) {
            Err(TelemetryConfigError::InvalidField { field, context }) => {
                assert_eq!(field, "delivered_retention");
                assert!(context.diagnostic().message.contains("overflow"));
            }
            other => panic!("expected overflow, got {other:?}"),
        }
    }
}
