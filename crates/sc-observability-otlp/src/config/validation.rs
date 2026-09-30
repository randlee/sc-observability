use std::path::PathBuf;
use std::time::Duration;

use super::types::{
    AuthHeader, ExporterBackend, LegacyRetryPolicy, OtelConfig, OtlpEndpoint, OtlpProtocol,
    TelemetryConfig,
};
use crate::{constants, error_codes};
use sc_observability_types::typed::InitFailure;
use sc_observability_types::v2::ConfigFailure;
use sc_observability_types::{DurationMs, ErrorContext, Remediation};
use serde_json::Value;

/// A named transport field used in deterministic validation diagnostics.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OtlpConfigField {
    /// OTLP collector endpoint.
    Endpoint,
    /// Authorization or credential header.
    Header,
    /// Per-request timeout.
    Timeout,
    /// Lifecycle flush timeout.
    LifecycleFlushTimeout,
    /// Lifecycle shutdown timeout.
    LifecycleShutdownTimeout,
    /// Record admission capacity.
    QueueCapacity,
    /// Aggregate byte admission capacity.
    QueueByteCapacity,
    /// Legacy maximum retries.
    MaxRetries,
    /// Legacy initial retry backoff.
    InitialBackoff,
    /// Legacy maximum retry backoff.
    MaxBackoff,
    /// Legacy complete retry-sequence timeout.
    RetrySequenceTimeout,
    /// Legacy Retry-After cap.
    RetryAfterCap,
    /// Legacy jitter percentage.
    RetryJitterPercent,
    /// TLS certificate-verification override.
    InsecureSkipVerify,
}

impl OtlpConfigField {
    const fn stable_name(self) -> &'static str {
        match self {
            Self::Endpoint => "endpoint",
            Self::Header => "auth_header",
            Self::Timeout => "timeout_ms",
            Self::LifecycleFlushTimeout => "lifecycle_flush_timeout_ms",
            Self::LifecycleShutdownTimeout => "lifecycle_shutdown_timeout_ms",
            Self::QueueCapacity => "queue_capacity",
            Self::QueueByteCapacity => "queue_byte_capacity",
            Self::MaxRetries => "legacy_retry.max_retries",
            Self::InitialBackoff => "legacy_retry.initial_backoff_ms",
            Self::MaxBackoff => "legacy_retry.max_backoff_ms",
            Self::RetrySequenceTimeout => "legacy_retry.retry_sequence_timeout_ms",
            Self::RetryAfterCap => "legacy_retry.retry_after_cap_ms",
            Self::RetryJitterPercent => "legacy_retry.retry_jitter_percent",
            Self::InsecureSkipVerify => "insecure_skip_verify",
        }
    }
}

/// Whether a resolved transport value came from the caller or the contract default.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueOrigin {
    /// Contract default.
    Default,
    /// Caller-supplied value.
    Explicit,
}

impl ValueOrigin {
    const fn stable_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Explicit => "explicit",
        }
    }
}

/// A resolved transport value with its stable field identity and source.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedField<T> {
    /// Field represented by this value.
    pub(crate) field: OtlpConfigField,
    /// Resolved value.
    pub(crate) value: T,
    /// Whether the value was explicit or defaulted.
    pub(crate) origin: ValueOrigin,
}

/// The target at which an inapplicable field was rejected.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OtlpConfigTarget {
    /// Transport is disabled and no backend is constructed.
    Disabled,
    /// A specific enabled backend was selected.
    Backend(ExporterBackend),
}

impl OtlpConfigTarget {
    fn stable_name(self) -> String {
        match self {
            Self::Disabled => "disabled".to_owned(),
            Self::Backend(backend) => format!("backend:{}", backend.stable_name()),
        }
    }
}

pub(crate) fn validate_config_typed(config: &TelemetryConfig) -> Result<(), InitFailure> {
    validated_telemetry_bounds(config).map(|_| ())
}

/// Validates a complete telemetry configuration once and returns its checked
/// transport bounds for factory construction.
pub(crate) fn validated_telemetry_bounds(
    config: &TelemetryConfig,
) -> Result<ValidatedTransportBounds, InitFailure> {
    validated_telemetry_bounds_with_delays(config, false)
}

/// Released conversion admits immediate retry delays; every other bound remains checked.
pub(crate) fn validated_released_telemetry_bounds(
    config: &TelemetryConfig,
) -> Result<ValidatedTransportBounds, InitFailure> {
    validated_telemetry_bounds_with_delays(config, true)
}

fn validated_telemetry_bounds_with_delays(
    config: &TelemetryConfig,
    immediate: bool,
) -> Result<ValidatedTransportBounds, InitFailure> {
    let bounds = validated_transport_bounds_with_delays(&config.transport, immediate)
        .map_err(config_failure_to_init_failure)?;
    if config.transport.enabled && config.transport.endpoint.is_none() {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
            "enabled telemetry requires an endpoint",
            Remediation::recoverable(
                "set OtelConfig.endpoint before constructing Telemetry",
                ["disable telemetry for local-only runs if OTLP is not required"],
            ),
        ))));
    }
    if config.transport.enabled
        && config.logs.is_none()
        && config.traces.is_none()
        && config.metrics.is_none()
    {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
            "at least one telemetry signal must be enabled",
            Remediation::recoverable(
                "enable logs, traces, or metrics before constructing Telemetry",
                ["disable the OTLP layer entirely if telemetry is not needed"],
            ),
        ))));
    }
    if config.logs.is_some_and(|cfg| cfg.batch_size == 0)
        || config.traces.is_some_and(|cfg| cfg.batch_size == 0)
        || config
            .metrics
            .is_some_and(|cfg| cfg.batch_size == 0 || u64::from(cfg.export_interval_ms) == 0)
    {
        return Err(InitFailure::from_context(Box::new(ErrorContext::new(
            error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
            "telemetry batch sizing and export intervals must be positive",
            Remediation::recoverable(
                "set batch sizes and export intervals above zero",
                ["use documented defaults"],
            ),
        ))));
    }
    Ok(bounds)
}

/// A duration checked as strictly positive by ordered config validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PositiveDuration(Duration);

impl PositiveDuration {
    #[cfg_attr(
        all(
            not(test),
            not(any(feature = "legacy-http-json", feature = "otlp-sdk"))
        ),
        expect(
            dead_code,
            reason = "validated durations are retained for configuration precedence without a compiled backend reader"
        )
    )]
    pub(crate) const fn get(self) -> Duration {
        self.0
    }
}

/// Checked maximum number of simultaneously admitted records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueCapacity(usize);

impl QueueCapacity {
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Checked aggregate serialized-byte budget for admitted records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueByteCapacity(usize);

impl QueueByteCapacity {
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// A percentage validated within zero through one hundred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundedPercent(u8);

#[cfg_attr(
    all(not(test), not(feature = "legacy-http-json")),
    expect(dead_code, reason = "D.21 checked contract consumed by D.6-D.8")
)]
impl BoundedPercent {
    pub(crate) const fn get(self) -> u8 {
        self.0
    }
}

/// Positive lifecycle deadlines, both at least the request timeout.
#[derive(Debug)]
pub(crate) struct LifecycleBounds {
    flush: PositiveDuration,
    shutdown: PositiveDuration,
}

impl LifecycleBounds {
    #[cfg_attr(
        all(
            not(test),
            not(any(feature = "legacy-http-json", feature = "otlp-sdk"))
        ),
        expect(
            dead_code,
            reason = "validated lifecycle deadline accessor is consumed by compiled backend paths"
        )
    )]
    pub(crate) const fn flush(&self) -> PositiveDuration {
        self.flush
    }
    #[cfg_attr(
        all(
            not(test),
            not(any(feature = "legacy-http-json", feature = "otlp-sdk"))
        ),
        expect(
            dead_code,
            reason = "validated lifecycle deadline accessor is consumed by compiled backend paths"
        )
    )]
    pub(crate) const fn shutdown(&self) -> PositiveDuration {
        self.shutdown
    }
}

/// Checked transport bounds; private fields prohibit unchecked factory construction.
#[derive(Debug)]
pub(crate) struct ValidatedTransportBounds {
    protocol: OtlpProtocol,
    queue_capacity: QueueCapacity,
    queue_byte_capacity: QueueByteCapacity,
    request_timeout: PositiveDuration,
    lifecycle: LifecycleBounds,
    backend: BackendTransportBounds,
}

/// Connection values admitted by the same validation path as transport
/// bounds. Backend constructors consume this view instead of consulting
/// ambient `OTEL_*` configuration.
#[derive(Debug, Clone)]
pub(crate) struct ValidatedBackendConnection {
    #[cfg_attr(
        not(any(feature = "legacy-http-json", feature = "otlp-sdk")),
        allow(
            dead_code,
            reason = "D.21 connection endpoint is consumed by enabled backends"
        )
    )]
    endpoint: OtlpEndpoint,
    #[cfg_attr(
        not(any(feature = "legacy-http-json", feature = "otlp-sdk")),
        allow(
            dead_code,
            reason = "D.21 connection auth is consumed by enabled backends"
        )
    )]
    auth_header: Option<AuthHeader>,
    #[cfg_attr(
        not(any(feature = "legacy-http-json", feature = "otlp-sdk")),
        allow(
            dead_code,
            reason = "D.21 connection CA is consumed by enabled backends"
        )
    )]
    ca_file: Option<PathBuf>,
}

impl ValidatedBackendConnection {
    #[cfg_attr(
        not(any(feature = "legacy-http-json", feature = "otlp-sdk")),
        allow(
            dead_code,
            reason = "D.21 endpoint view is consumed by enabled backends"
        )
    )]
    pub(crate) fn endpoint(&self) -> &OtlpEndpoint {
        &self.endpoint
    }

    #[cfg_attr(
        not(any(feature = "legacy-http-json", feature = "otlp-sdk")),
        allow(dead_code, reason = "D.21 auth view is consumed by enabled backends")
    )]
    pub(crate) fn auth_header(&self) -> Option<&AuthHeader> {
        self.auth_header.as_ref()
    }

    #[cfg_attr(
        not(any(feature = "legacy-http-json", feature = "otlp-sdk")),
        allow(dead_code, reason = "D.21 CA view is consumed by enabled backends")
    )]
    pub(crate) fn ca_file(&self) -> Option<&PathBuf> {
        self.ca_file.as_ref()
    }
}

impl ValidatedTransportBounds {
    pub(crate) const fn protocol(&self) -> OtlpProtocol {
        self.protocol
    }
    pub(crate) const fn queue_capacity(&self) -> QueueCapacity {
        self.queue_capacity
    }
    pub(crate) const fn queue_byte_capacity(&self) -> QueueByteCapacity {
        self.queue_byte_capacity
    }
    #[cfg_attr(
        all(
            not(test),
            not(any(feature = "legacy-http-json", feature = "otlp-sdk"))
        ),
        allow(
            dead_code,
            reason = "D.21 request timeout is consumed by enabled backends"
        )
    )]
    pub(crate) const fn request_timeout(&self) -> PositiveDuration {
        self.request_timeout
    }
    #[cfg_attr(
        all(
            not(test),
            not(any(feature = "legacy-http-json", feature = "otlp-sdk"))
        ),
        expect(
            dead_code,
            reason = "validated lifecycle bounds accessor is consumed by compiled backend paths"
        )
    )]
    pub(crate) const fn lifecycle(&self) -> &LifecycleBounds {
        &self.lifecycle
    }
    pub(crate) const fn backend(&self) -> &BackendTransportBounds {
        &self.backend
    }
}

/// Backend-specific state; SDK and disabled transports cannot carry retry policy.
#[cfg_attr(
    all(not(test), not(feature = "legacy-http-json")),
    allow(
        dead_code,
        reason = "D.21 legacy retry state is consumed by the legacy backend"
    )
)]
#[derive(Debug)]
pub(crate) enum BackendTransportBounds {
    Disabled,
    Sdk,
    Legacy(RetryPolicy),
}

/// Checked legacy retry policy produced only by ordered config validation.
#[derive(Debug)]
pub(crate) struct RetryPolicy {
    max_retries: u32,
    initial_backoff: RetryDelay,
    max_backoff: RetryDelay,
    sequence_timeout: PositiveDuration,
    retry_after_cap: PositiveDuration,
    jitter: BoundedPercent,
}

#[cfg_attr(
    not(feature = "legacy-http-json"),
    allow(dead_code, reason = "D.21 checked contract consumed by D.6-D.8")
)]
impl RetryPolicy {
    pub(crate) const fn max_retries(&self) -> u32 {
        self.max_retries
    }
    pub(crate) const fn initial_backoff(&self) -> RetryDelay {
        self.initial_backoff
    }
    pub(crate) const fn max_backoff(&self) -> RetryDelay {
        self.max_backoff
    }
    pub(crate) const fn sequence_timeout(&self) -> PositiveDuration {
        self.sequence_timeout
    }
    pub(crate) const fn retry_after_cap(&self) -> PositiveDuration {
        self.retry_after_cap
    }
    pub(crate) const fn jitter(&self) -> BoundedPercent {
        self.jitter
    }
}

/// Internal retry delay proven either immediate by released compatibility
/// validation or strictly positive by canonical validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryDelay {
    Immediate,
    Positive(PositiveDuration),
}

impl RetryDelay {
    #[cfg(any(test, feature = "legacy-http-json"))]
    pub(crate) const fn get(self) -> Duration {
        match self {
            Self::Immediate => Duration::ZERO,
            Self::Positive(value) => value.get(),
        }
    }

    const fn positive(value: PositiveDuration) -> Self {
        Self::Positive(value)
    }
}

/// Resolves defaults and validates a transport in the documented first-failure
/// order. This is crate-visible for backend factories and contract tests.
pub(crate) fn validated_transport_bounds(
    config: &OtelConfig,
) -> Result<ValidatedTransportBounds, ConfigFailure> {
    validated_transport_bounds_with_delays(config, false)
}

fn validated_transport_bounds_with_delays(
    config: &OtelConfig,
    immediate: bool,
) -> Result<ValidatedTransportBounds, ConfigFailure> {
    let legacy_retry_field = first_legacy_retry_field(config);
    let timeout = resolve_duration(
        OtlpConfigField::Timeout,
        config.timeout_ms,
        constants::DEFAULT_OTLP_TIMEOUT_MS,
    );
    let flush = resolve_duration(
        OtlpConfigField::LifecycleFlushTimeout,
        config.lifecycle_flush_timeout_ms,
        constants::DEFAULT_OTLP_LIFECYCLE_FLUSH_TIMEOUT_MS,
    );
    let shutdown = resolve_duration(
        OtlpConfigField::LifecycleShutdownTimeout,
        config.lifecycle_shutdown_timeout_ms,
        constants::DEFAULT_OTLP_LIFECYCLE_SHUTDOWN_TIMEOUT_MS,
    );
    let queue_capacity = resolve_usize(
        OtlpConfigField::QueueCapacity,
        config.queue_capacity,
        constants::DEFAULT_OTLP_QUEUE_CAPACITY,
    );
    let queue_byte_capacity = resolve_usize(
        OtlpConfigField::QueueByteCapacity,
        config.queue_byte_capacity,
        constants::DEFAULT_OTLP_QUEUE_BYTE_CAPACITY,
    );

    // The ordering below is normative: do not aggregate failures or move
    // checks without updating the D.21 contract tests.
    let request_timeout = checked_duration(&timeout)?;
    let lifecycle_flush_timeout = checked_duration(&flush)?;
    let lifecycle_shutdown_timeout = checked_duration(&shutdown)?;
    if shutdown.value < timeout.value {
        return Err(invalid_bound(&timeout, &shutdown));
    }
    if flush.value < timeout.value {
        return Err(invalid_bound(&timeout, &flush));
    }
    let legacy_retry =
        if config.enabled && matches!(config.backend, ExporterBackend::LegacyHttpJson) {
            Some(resolve_retry(
                config.legacy_retry.as_ref(),
                &timeout,
                immediate,
            )?)
        } else {
            None
        };
    let (queue_capacity, queue_byte_capacity) =
        checked_queue_bounds(&queue_capacity, &queue_byte_capacity)?;

    let backend = if config.enabled {
        match config.backend {
            ExporterBackend::OpenTelemetrySdk => {
                if let Some(field) = legacy_retry_field {
                    return Err(not_applicable(
                        field,
                        OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk),
                    ));
                }
                BackendTransportBounds::Sdk
            }
            ExporterBackend::LegacyHttpJson => BackendTransportBounds::Legacy(
                legacy_retry.expect("legacy backend resolves its retry policy"),
            ),
        }
    } else {
        if let Some(field) = legacy_retry_field {
            return Err(not_applicable(field, OtlpConfigTarget::Disabled));
        }
        BackendTransportBounds::Disabled
    };

    if config.insecure_skip_verify && config.enabled {
        return Err(insecure_transport_rejected(config.backend));
    }

    Ok(ValidatedTransportBounds {
        protocol: config.protocol,
        queue_capacity,
        queue_byte_capacity,
        request_timeout,
        lifecycle: LifecycleBounds {
            flush: lifecycle_flush_timeout,
            shutdown: lifecycle_shutdown_timeout,
        },
        backend,
    })
}

fn checked_queue_bounds(
    queue_capacity: &ResolvedField<usize>,
    queue_byte_capacity: &ResolvedField<usize>,
) -> Result<(QueueCapacity, QueueByteCapacity), ConfigFailure> {
    if !(1..=constants::MAX_OTLP_QUEUE_CAPACITY).contains(&queue_capacity.value) {
        return Err(config_failure(
            ConfigFailureKind::InvalidQueueCapacity,
            error_codes::OTLP_CONFIG_QUEUE_CAPACITY,
            format!(
                "queue capacity must be in 1..={}",
                constants::MAX_OTLP_QUEUE_CAPACITY
            ),
            queue_capacity.field,
            queue_capacity.origin,
        ));
    }
    if queue_byte_capacity.value == 0
        || queue_byte_capacity.value > constants::MAX_OTLP_QUEUE_BYTE_CAPACITY
    {
        return Err(config_failure(
            ConfigFailureKind::InvalidQueueByteCapacity,
            error_codes::OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
            "queue byte capacity must be in 1..=64 MiB",
            queue_byte_capacity.field,
            queue_byte_capacity.origin,
        ));
    }
    Ok((
        QueueCapacity(queue_capacity.value),
        QueueByteCapacity(queue_byte_capacity.value),
    ))
}

/// Returns the connection values only after the transport's ordinary ordered
/// validation has succeeded. Enabled factories need an explicit endpoint and
/// must never reconstruct it from environment defaults.
#[allow(
    dead_code,
    reason = "D.18 consumes the validated SDK connection view during facade composition"
)]
pub(crate) fn validated_backend_connection(
    config: &OtelConfig,
) -> Result<ValidatedBackendConnection, ConfigFailure> {
    let bounds = validated_transport_bounds(config)?;
    prepared_backend_connection(config, &bounds)
}

/// Consume checked bounds without revalidating their compatibility-only retry delays.
pub(crate) fn prepared_backend_connection(
    config: &OtelConfig,
    _bounds: &ValidatedTransportBounds,
) -> Result<ValidatedBackendConnection, ConfigFailure> {
    let endpoint = config.endpoint.clone().ok_or_else(|| {
        invalid_endpoint(
            "enabled telemetry requires an endpoint",
            "set OtelConfig.endpoint before constructing the backend",
        )
    })?;
    Ok(ValidatedBackendConnection {
        endpoint,
        auth_header: config.auth_header.clone(),
        ca_file: config.ca_file.clone(),
    })
}

/// Returns the first legacy-only retry setting supplied by the caller.
///
/// The order is part of the deterministic validation contract.
fn first_legacy_retry_field(config: &OtelConfig) -> Option<OtlpConfigField> {
    let retry = config.legacy_retry.as_ref();
    if retry.is_some_and(|value| value.max_retries.is_some()) {
        return Some(OtlpConfigField::MaxRetries);
    }
    if retry.is_some_and(|value| value.initial_backoff_ms.is_some()) {
        return Some(OtlpConfigField::InitialBackoff);
    }
    if retry.is_some_and(|value| value.max_backoff_ms.is_some()) {
        return Some(OtlpConfigField::MaxBackoff);
    }
    if retry.is_some_and(|value| value.retry_sequence_timeout_ms.is_some()) {
        return Some(OtlpConfigField::RetrySequenceTimeout);
    }
    if retry.is_some_and(|value| value.retry_after_cap_ms.is_some()) {
        return Some(OtlpConfigField::RetryAfterCap);
    }
    if retry.is_some_and(|value| value.retry_jitter_percent.is_some()) {
        return Some(OtlpConfigField::RetryJitterPercent);
    }

    // An explicitly supplied but empty compatibility block is still
    // inapplicable outside the legacy backend; retain the original field.
    config
        .legacy_retry
        .as_ref()
        .map(|_| OtlpConfigField::MaxRetries)
}

fn resolve_duration(
    field: OtlpConfigField,
    raw: Option<DurationMs>,
    default: u64,
) -> ResolvedField<u64> {
    ResolvedField {
        field,
        value: raw.map_or(default, u64::from),
        origin: if raw.is_some() {
            ValueOrigin::Explicit
        } else {
            ValueOrigin::Default
        },
    }
}

fn resolve_usize(
    field: OtlpConfigField,
    raw: Option<usize>,
    default: usize,
) -> ResolvedField<usize> {
    ResolvedField {
        field,
        value: raw.unwrap_or(default),
        origin: if raw.is_some() {
            ValueOrigin::Explicit
        } else {
            ValueOrigin::Default
        },
    }
}

fn checked_duration(value: &ResolvedField<u64>) -> Result<PositiveDuration, ConfigFailure> {
    if value.value == 0 {
        return Err(config_failure(
            ConfigFailureKind::ZeroDuration,
            error_codes::OTLP_CONFIG_ZERO_DURATION,
            "duration must be greater than zero",
            value.field,
            value.origin,
        ));
    }
    Ok(PositiveDuration(Duration::from_millis(value.value)))
}

fn resolve_retry(
    raw: Option<&LegacyRetryPolicy>,
    timeout: &ResolvedField<u64>,
    immediate: bool,
) -> Result<RetryPolicy, ConfigFailure> {
    let raw = raw.cloned().unwrap_or_default();
    let initial = resolve_duration(
        OtlpConfigField::InitialBackoff,
        raw.initial_backoff_ms,
        constants::DEFAULT_OTLP_INITIAL_BACKOFF_MS,
    );
    let maximum = resolve_duration(
        OtlpConfigField::MaxBackoff,
        raw.max_backoff_ms,
        constants::DEFAULT_OTLP_MAX_BACKOFF_MS,
    );
    let sequence = resolve_duration(
        OtlpConfigField::RetrySequenceTimeout,
        raw.retry_sequence_timeout_ms,
        constants::DEFAULT_OTLP_RETRY_SEQUENCE_TIMEOUT_MS,
    );
    let after_cap = resolve_duration(
        OtlpConfigField::RetryAfterCap,
        raw.retry_after_cap_ms,
        constants::DEFAULT_OTLP_RETRY_AFTER_CAP_MS,
    );
    let jitter = ResolvedField {
        field: OtlpConfigField::RetryJitterPercent,
        value: raw
            .retry_jitter_percent
            .unwrap_or(constants::DEFAULT_OTLP_RETRY_JITTER_PERCENT),
        origin: if raw.retry_jitter_percent.is_some() {
            ValueOrigin::Explicit
        } else {
            ValueOrigin::Default
        },
    };
    let delay = |value: &ResolvedField<u64>| {
        if immediate && value.value == 0 {
            Ok(RetryDelay::Immediate)
        } else {
            checked_duration(value).map(RetryDelay::positive)
        }
    };
    let initial_backoff = delay(&initial)?;
    let max_backoff = delay(&maximum)?;
    let sequence_timeout = checked_duration(&sequence)?;
    let retry_after_cap = checked_duration(&after_cap)?;
    if maximum.value < initial.value {
        return Err(invalid_bound(&initial, &maximum));
    }
    if sequence.value < timeout.value {
        return Err(invalid_bound(timeout, &sequence));
    }
    if after_cap.value > sequence.value {
        return Err(invalid_bound(&after_cap, &sequence));
    }
    if jitter.value > constants::MAX_OTLP_RETRY_JITTER_PERCENT {
        return Err(config_failure(
            ConfigFailureKind::InvalidJitterPercent,
            error_codes::OTLP_CONFIG_JITTER_PERCENT,
            format!(
                "retry jitter percent must be in 0..={}",
                constants::MAX_OTLP_RETRY_JITTER_PERCENT
            ),
            jitter.field,
            jitter.origin,
        ));
    }
    Ok(RetryPolicy {
        max_retries: raw
            .max_retries
            .unwrap_or(constants::DEFAULT_OTLP_MAX_RETRIES),
        initial_backoff,
        max_backoff,
        sequence_timeout,
        retry_after_cap,
        jitter: BoundedPercent(jitter.value),
    })
}

#[derive(Clone, Copy)]
enum ConfigFailureKind {
    ZeroDuration,
    InvalidJitterPercent,
    InvalidQueueCapacity,
    InvalidQueueByteCapacity,
}

fn invalid_bound(lower: &ResolvedField<u64>, upper: &ResolvedField<u64>) -> ConfigFailure {
    // Preserve both resolved fields in structured diagnostics without exposing
    // credentials or raw endpoint data.
    ConfigFailure::InvalidBoundOrdering {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_BOUND_ORDER,
                "resolved transport bounds are out of order",
                Remediation::recoverable(
                    "correct the named OTLP configuration fields",
                    ["use documented defaults"],
                ),
            )
            .detail("field", Value::String(lower.field.stable_name().to_owned()))
            .detail(
                "origin",
                Value::String(lower.origin.stable_name().to_owned()),
            )
            .detail("lower_value", Value::from(lower.value))
            .detail(
                "upper_field",
                Value::String(upper.field.stable_name().to_owned()),
            )
            .detail(
                "upper_origin",
                Value::String(upper.origin.stable_name().to_owned()),
            )
            .detail("upper_value", Value::from(upper.value)),
        ),
    }
}

fn not_applicable(field: OtlpConfigField, target: OtlpConfigTarget) -> ConfigFailure {
    ConfigFailure::ConfigFieldNotApplicable {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_FIELD_NOT_APPLICABLE,
                "configuration field is not applicable to the selected transport target",
                Remediation::recoverable(
                    "omit the field or choose an applicable backend",
                    ["use documented defaults"],
                ),
            )
            .detail("field", Value::String(field.stable_name().to_owned()))
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            )
            .detail("target", Value::String(target.stable_name())),
        ),
    }
}

fn config_failure(
    kind: ConfigFailureKind,
    code: sc_observability_types::ErrorCode,
    message: impl Into<String>,
    field: OtlpConfigField,
    origin: ValueOrigin,
) -> ConfigFailure {
    let context = Box::new(
        ErrorContext::new(
            code,
            message,
            Remediation::recoverable(
                "correct the named OTLP configuration field",
                ["use documented defaults"],
            ),
        )
        .detail("field", Value::String(field.stable_name().to_owned()))
        .detail("origin", Value::String(origin.stable_name().to_owned())),
    );
    match kind {
        ConfigFailureKind::ZeroDuration => ConfigFailure::ZeroDuration { context },
        ConfigFailureKind::InvalidJitterPercent => ConfigFailure::InvalidJitterPercent { context },
        ConfigFailureKind::InvalidQueueCapacity => ConfigFailure::InvalidQueueCapacity { context },
        ConfigFailureKind::InvalidQueueByteCapacity => {
            ConfigFailure::InvalidQueueByteCapacity { context }
        }
    }
}

fn insecure_transport_rejected(backend: ExporterBackend) -> ConfigFailure {
    ConfigFailure::InsecureTransportRejected {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED,
                "the selected backend does not support insecure certificate verification",
                Remediation::recoverable(
                    "leave certificate verification enabled",
                    ["use the documented TLS configuration"],
                ),
            )
            .detail(
                "field",
                Value::String(OtlpConfigField::InsecureSkipVerify.stable_name().to_owned()),
            )
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            )
            .detail("backend", Value::String(backend.stable_name().to_owned())),
        ),
    }
}

pub(super) fn config_failure_to_init_failure(error: ConfigFailure) -> InitFailure {
    InitFailure::from_context(error.into_context())
}

pub(super) fn is_valid_http_endpoint(value: &str) -> bool {
    // Preserve the retained raw-value behavior for harmless trailing spaces,
    // while validating the URL-shaped portion and rejecting control bytes.
    if value.chars().any(char::is_control) {
        return false;
    }
    let value = value.trim_end();
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") {
        return false;
    }
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains('@')
        || authority.contains('\\')
        || authority.chars().any(char::is_whitespace)
    {
        return false;
    }

    if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((host, port)) = bracketed.split_once(']') else {
            return false;
        };
        return !host.is_empty()
            && (port.is_empty()
                || port.strip_prefix(':').is_some_and(|value| {
                    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
                }));
    }

    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    !host.is_empty()
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
        && port.is_none_or(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
}

pub(super) fn invalid_endpoint(message: &str, remediation: &str) -> ConfigFailure {
    ConfigFailure::InvalidEndpoint {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_INVALID_ENDPOINT,
                message,
                Remediation::recoverable(
                    remediation,
                    ["use the documented OTLP transport defaults"],
                ),
            )
            .detail(
                "field",
                Value::String(OtlpConfigField::Endpoint.stable_name().to_owned()),
            )
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            ),
        ),
    }
}

pub(super) fn invalid_header(message: &str, remediation: &str) -> ConfigFailure {
    ConfigFailure::InvalidHeader {
        context: Box::new(
            ErrorContext::new(
                error_codes::OTLP_CONFIG_INVALID_HEADER,
                message,
                Remediation::recoverable(
                    remediation,
                    ["use the documented OTLP transport defaults"],
                ),
            )
            .detail(
                "field",
                Value::String(OtlpConfigField::Header.stable_name().to_owned()),
            )
            .detail(
                "origin",
                Value::String(ValueOrigin::Explicit.stable_name().to_owned()),
            ),
        ),
    }
}
