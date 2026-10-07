#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
use std::path::PathBuf;
#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
use std::time::Duration;

#[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
use super::types::{AuthHeader, OtlpEndpoint};
use super::types::{
    ExporterBackend, OtelConfig, OtlpProtocol, SyncHttpRetryPolicy, TelemetryConfig,
};
use crate::{constants, error_codes};
use sc_observability_types::v2::{ConfigFailure, InitError};
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
    /// Synchronous HTTP maximum retries.
    MaxRetries,
    /// Synchronous HTTP initial retry backoff.
    InitialBackoff,
    /// Synchronous HTTP maximum retry backoff.
    MaxBackoff,
    /// Synchronous HTTP complete retry-sequence timeout.
    RetrySequenceTimeout,
    /// Synchronous HTTP Retry-After cap.
    RetryAfterCap,
    /// Synchronous HTTP jitter percentage.
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
            Self::MaxRetries => "sync_http_retry.max_retries",
            Self::InitialBackoff => "sync_http_retry.initial_backoff_ms",
            Self::MaxBackoff => "sync_http_retry.max_backoff_ms",
            Self::RetrySequenceTimeout => "sync_http_retry.retry_sequence_timeout_ms",
            Self::RetryAfterCap => "sync_http_retry.retry_after_cap_ms",
            Self::RetryJitterPercent => "sync_http_retry.retry_jitter_percent",
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

pub(crate) fn validate_config_typed(config: &TelemetryConfig) -> Result<(), InitError> {
    validated_telemetry_bounds(config).map(|_| ())
}

/// Validates a complete telemetry configuration once and returns its checked
/// transport bounds for factory construction.
pub(crate) fn validated_telemetry_bounds(
    config: &TelemetryConfig,
) -> Result<ValidatedTransportBounds, InitError> {
    validated_telemetry_bounds_with_delays(config, false)
}

/// Test-only construction permits immediate retry delays so hermetic capture
/// fixtures do not depend on scheduler time.
#[cfg(test)]
pub(crate) fn validated_test_telemetry_bounds(
    config: &TelemetryConfig,
) -> Result<ValidatedTransportBounds, InitError> {
    validated_telemetry_bounds_with_delays(config, true)
}

fn validated_telemetry_bounds_with_delays(
    config: &TelemetryConfig,
    immediate: bool,
) -> Result<ValidatedTransportBounds, InitError> {
    let bounds = validated_transport_bounds_with_delays(&config.transport, immediate)
        .map_err(config_failure_to_init_failure)?;
    if config.transport.enabled && config.transport.endpoint.is_none() {
        return Err(InitError::Configuration {
            context: Box::new(ErrorContext::new(
                error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
                "enabled telemetry requires an endpoint",
                Remediation::recoverable(
                    "set OtelConfig.endpoint before constructing Telemetry",
                    ["disable telemetry for local-only runs if OTLP is not required"],
                ),
            )),
        });
    }
    if config.transport.enabled
        && config.logs.is_none()
        && config.traces.is_none()
        && config.metrics.is_none()
    {
        return Err(InitError::Configuration {
            context: Box::new(ErrorContext::new(
                error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
                "at least one telemetry signal must be enabled",
                Remediation::recoverable(
                    "enable logs, traces, or metrics before constructing Telemetry",
                    ["disable the OTLP layer entirely if telemetry is not needed"],
                ),
            )),
        });
    }
    if config.logs.is_some_and(|cfg| cfg.batch_size == 0)
        || config.traces.is_some_and(|cfg| cfg.batch_size == 0)
        || config
            .metrics
            .is_some_and(|cfg| cfg.batch_size == 0 || u64::from(cfg.export_interval_ms) == 0)
    {
        return Err(InitError::Configuration {
            context: Box::new(ErrorContext::new(
                error_codes::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
                "telemetry batch sizing and export intervals must be positive",
                Remediation::recoverable(
                    "set batch sizes and export intervals above zero",
                    ["use documented defaults"],
                ),
            )),
        });
    }
    Ok(bounds)
}

/// A duration stored after ordered config validation proves it is strictly positive.
#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PositiveDuration(Duration);

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
impl PositiveDuration {
    pub(crate) const fn get(self) -> Duration {
        self.0
    }
}

/// Checked maximum number of simultaneously admitted records.
#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueCapacity(usize);

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
impl QueueCapacity {
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Checked aggregate serialized-byte budget for admitted records.
#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueByteCapacity(usize);

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
impl QueueByteCapacity {
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// A percentage validated within zero through one hundred.
#[cfg(any(test, feature = "sync-http"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundedPercent(u8);

#[cfg(any(test, feature = "sync-http"))]
impl BoundedPercent {
    pub(crate) const fn get(self) -> u8 {
        self.0
    }
}

/// Positive lifecycle deadlines, both at least the request timeout.
#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
#[derive(Debug)]
pub(crate) struct LifecycleBounds {
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    flush: PositiveDuration,
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    shutdown: PositiveDuration,
}

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
impl LifecycleBounds {
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) const fn flush(&self) -> PositiveDuration {
        self.flush
    }
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) const fn shutdown(&self) -> PositiveDuration {
        self.shutdown
    }
}

/// Checked transport bounds; private fields prohibit unchecked factory construction.
#[derive(Debug)]
pub(crate) struct ValidatedTransportBounds {
    protocol: OtlpProtocol,
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    queue_capacity: QueueCapacity,
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    queue_byte_capacity: QueueByteCapacity,
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    request_timeout: PositiveDuration,
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    lifecycle: LifecycleBounds,
    backend: BackendTransportBounds,
}

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
struct StoredTransportValues {
    queue_capacity: QueueCapacity,
    queue_byte_capacity: QueueByteCapacity,
    request_timeout: PositiveDuration,
    lifecycle: LifecycleBounds,
}

/// Connection values admitted by the same validation path as transport
/// bounds. Backend constructors consume this view instead of consulting
/// ambient `OTEL_*` configuration.
#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
#[derive(Debug, Clone)]
pub(crate) struct ValidatedBackendConnection {
    #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
    endpoint: OtlpEndpoint,
    #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
    auth_header: Option<AuthHeader>,
    #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
    ca_file: Option<PathBuf>,
}

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
impl ValidatedBackendConnection {
    #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) fn endpoint(&self) -> &OtlpEndpoint {
        &self.endpoint
    }

    #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) fn auth_header(&self) -> Option<&AuthHeader> {
        self.auth_header.as_ref()
    }

    #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) fn ca_file(&self) -> Option<&PathBuf> {
        self.ca_file.as_ref()
    }
}

impl ValidatedTransportBounds {
    pub(crate) const fn protocol(&self) -> OtlpProtocol {
        self.protocol
    }
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) const fn queue_capacity(&self) -> QueueCapacity {
        self.queue_capacity
    }
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) const fn queue_byte_capacity(&self) -> QueueByteCapacity {
        self.queue_byte_capacity
    }
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) const fn request_timeout(&self) -> PositiveDuration {
        self.request_timeout
    }
    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    pub(crate) const fn lifecycle(&self) -> &LifecycleBounds {
        &self.lifecycle
    }
    pub(crate) const fn backend(&self) -> &BackendTransportBounds {
        &self.backend
    }
}

/// Backend-specific state; SDK and disabled transports cannot carry retry policy.
#[derive(Debug)]
pub(crate) enum BackendTransportBounds {
    Disabled,
    Sdk,
    #[cfg(any(test, feature = "sync-http"))]
    SyncHttp(RetryPolicy),
    #[cfg(not(any(test, feature = "sync-http")))]
    SyncHttp,
}

/// Checked synchronous HTTP retry policy produced only by ordered config validation.
#[cfg(any(test, feature = "sync-http"))]
#[derive(Debug)]
pub(crate) struct RetryPolicy {
    max_retries: u32,
    initial_backoff: RetryDelay,
    max_backoff: RetryDelay,
    sequence_timeout: PositiveDuration,
    retry_after_cap: PositiveDuration,
    jitter: BoundedPercent,
}

#[cfg(any(test, feature = "sync-http"))]
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
#[cfg(any(test, feature = "sync-http"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryDelay {
    Immediate,
    Positive(PositiveDuration),
}

#[cfg(any(test, feature = "sync-http"))]
impl RetryDelay {
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
#[cfg(any(test, feature = "durable-store"))]
pub(crate) fn validated_transport_bounds(
    config: &OtelConfig,
) -> Result<ValidatedTransportBounds, ConfigFailure> {
    validated_transport_bounds_with_delays(config, false)
}

fn validated_transport_bounds_with_delays(
    config: &OtelConfig,
    immediate: bool,
) -> Result<ValidatedTransportBounds, ConfigFailure> {
    let sync_http_retry_field = first_sync_http_retry_field(config);
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
    validate_positive_duration(&timeout)?;
    validate_positive_duration(&flush)?;
    validate_positive_duration(&shutdown)?;
    let retry = resolve_sync_http_retry_fields(config, immediate)?;
    if shutdown.value < timeout.value {
        return Err(invalid_bound(&timeout, &shutdown));
    }
    if flush.value < timeout.value {
        return Err(invalid_bound(&timeout, &flush));
    }
    validate_queue_bounds(&queue_capacity, &queue_byte_capacity)?;
    #[cfg(any(test, feature = "sync-http"))]
    let sync_http_retry = validated_sync_http_retry(retry, &timeout, immediate)?;
    #[cfg(not(any(test, feature = "sync-http")))]
    validate_sync_http_retry(retry, &timeout)?;

    #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
    let stored = store_transport_values(
        &timeout,
        &flush,
        &shutdown,
        &queue_capacity,
        &queue_byte_capacity,
    );

    let backend = if config.enabled {
        match config.backend {
            ExporterBackend::OpenTelemetrySdk => {
                if let Some(field) = sync_http_retry_field {
                    return Err(not_applicable(
                        field,
                        OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk),
                    ));
                }
                BackendTransportBounds::Sdk
            }
            ExporterBackend::SyncHttp => {
                #[cfg(any(test, feature = "sync-http"))]
                {
                    BackendTransportBounds::SyncHttp(
                        sync_http_retry
                            .expect("synchronous HTTP backend resolves its retry policy"),
                    )
                }
                #[cfg(not(any(test, feature = "sync-http")))]
                {
                    BackendTransportBounds::SyncHttp
                }
            }
        }
    } else {
        if let Some(field) = sync_http_retry_field {
            return Err(not_applicable(field, OtlpConfigTarget::Disabled));
        }
        BackendTransportBounds::Disabled
    };

    if config.insecure_skip_verify && config.enabled {
        return Err(insecure_transport_rejected(config.backend));
    }

    Ok(ValidatedTransportBounds {
        protocol: config.protocol,
        #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
        queue_capacity: stored.queue_capacity,
        #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
        queue_byte_capacity: stored.queue_byte_capacity,
        #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
        request_timeout: stored.request_timeout,
        #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
        lifecycle: LifecycleBounds {
            #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
            flush: stored.lifecycle.flush,
            #[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
            shutdown: stored.lifecycle.shutdown,
        },
        backend,
    })
}

#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
fn store_transport_values(
    timeout: &ResolvedField<u64>,
    flush: &ResolvedField<u64>,
    shutdown: &ResolvedField<u64>,
    queue_capacity: &ResolvedField<usize>,
    queue_byte_capacity: &ResolvedField<usize>,
) -> StoredTransportValues {
    StoredTransportValues {
        queue_capacity: QueueCapacity(queue_capacity.value),
        queue_byte_capacity: QueueByteCapacity(queue_byte_capacity.value),
        request_timeout: PositiveDuration(Duration::from_millis(timeout.value)),
        lifecycle: LifecycleBounds {
            flush: PositiveDuration(Duration::from_millis(flush.value)),
            shutdown: PositiveDuration(Duration::from_millis(shutdown.value)),
        },
    }
}

fn validate_queue_bounds(
    queue_capacity: &ResolvedField<usize>,
    queue_byte_capacity: &ResolvedField<usize>,
) -> Result<(), ConfigFailure> {
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
    Ok(())
}

/// Returns the connection values only after the transport's ordinary ordered
/// validation has succeeded. Enabled factories need an explicit endpoint and
/// must never reconstruct it from environment defaults.
#[cfg(all(test, feature = "otlp-sdk"))]
pub(crate) fn validated_backend_connection(
    config: &OtelConfig,
) -> Result<ValidatedBackendConnection, ConfigFailure> {
    let bounds = validated_transport_bounds(config)?;
    prepared_backend_connection(config, &bounds)
}

/// Consume checked bounds without revalidating their compatibility-only retry delays.
#[cfg(any(test, feature = "sync-http", feature = "otlp-sdk"))]
pub(crate) fn prepared_backend_connection(
    config: &OtelConfig,
    _bounds: &ValidatedTransportBounds,
) -> Result<ValidatedBackendConnection, ConfigFailure> {
    #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
    let endpoint = config.endpoint.clone().ok_or_else(|| {
        invalid_endpoint(
            "enabled telemetry requires an endpoint",
            "set OtelConfig.endpoint before constructing the backend",
        )
    })?;
    #[cfg(not(any(feature = "sync-http", feature = "otlp-sdk")))]
    if config.endpoint.is_none() {
        return Err(invalid_endpoint(
            "enabled telemetry requires an endpoint",
            "set OtelConfig.endpoint before constructing the backend",
        ));
    }
    Ok(ValidatedBackendConnection {
        #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
        endpoint,
        #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
        auth_header: config.auth_header.clone(),
        #[cfg(any(feature = "sync-http", feature = "otlp-sdk"))]
        ca_file: config.ca_file.clone(),
    })
}

/// Returns the first sync-http-only retry setting supplied by the caller.
///
/// The order is part of the deterministic validation contract.
fn first_sync_http_retry_field(config: &OtelConfig) -> Option<OtlpConfigField> {
    let retry = config.sync_http_retry.as_ref();
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
    // inapplicable outside the synchronous HTTP backend; retain the original field.
    config
        .sync_http_retry
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

fn validate_positive_duration(value: &ResolvedField<u64>) -> Result<(), ConfigFailure> {
    if value.value == 0 {
        return Err(config_failure(
            ConfigFailureKind::ZeroDuration,
            error_codes::OTLP_CONFIG_ZERO_DURATION,
            "duration must be greater than zero",
            value.field,
            value.origin,
        ));
    }
    Ok(())
}

fn resolve_sync_http_retry_fields(
    config: &OtelConfig,
    immediate: bool,
) -> Result<Option<ResolvedRetryFields>, ConfigFailure> {
    if config.enabled && matches!(config.backend, ExporterBackend::SyncHttp) {
        resolve_retry_fields(config.sync_http_retry.as_ref(), immediate).map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(any(test, feature = "sync-http"))]
fn validated_sync_http_retry(
    retry: Option<ResolvedRetryFields>,
    timeout: &ResolvedField<u64>,
    immediate: bool,
) -> Result<Option<RetryPolicy>, ConfigFailure> {
    retry
        .map(|retry| resolve_retry(retry, timeout, immediate))
        .transpose()
}

#[cfg(not(any(test, feature = "sync-http")))]
fn validate_sync_http_retry(
    retry: Option<ResolvedRetryFields>,
    timeout: &ResolvedField<u64>,
) -> Result<(), ConfigFailure> {
    if let Some(retry) = retry {
        validate_retry(retry, timeout, |_, _, _, _, _, _| ())?;
    }
    Ok(())
}

/// Resolved retry durations, checked positive before lifecycle ordering.
struct ResolvedRetryFields {
    max_retries: u32,
    initial: ResolvedField<u64>,
    maximum: ResolvedField<u64>,
    sequence: ResolvedField<u64>,
    after_cap: ResolvedField<u64>,
    jitter: ResolvedField<u8>,
}

fn resolve_retry_fields(
    raw: Option<&SyncHttpRetryPolicy>,
    immediate: bool,
) -> Result<ResolvedRetryFields, ConfigFailure> {
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
    let validate_delay = |value: &ResolvedField<u64>| {
        if immediate && value.value == 0 {
            Ok(())
        } else {
            validate_positive_duration(value)
        }
    };
    validate_delay(&initial)?;
    validate_delay(&maximum)?;
    validate_positive_duration(&sequence)?;
    validate_positive_duration(&after_cap)?;
    Ok(ResolvedRetryFields {
        max_retries: raw
            .max_retries
            .unwrap_or(constants::DEFAULT_OTLP_MAX_RETRIES),
        initial,
        maximum,
        sequence,
        after_cap,
        jitter,
    })
}

/// Retry ordering follows lifecycle and queue validation in the public contract.
fn validate_retry<T>(
    retry: ResolvedRetryFields,
    timeout: &ResolvedField<u64>,
    build: impl FnOnce(u32, u64, u64, u64, u64, u8) -> T,
) -> Result<T, ConfigFailure> {
    let ResolvedRetryFields {
        max_retries,
        initial,
        maximum,
        sequence,
        after_cap,
        jitter,
    } = retry;
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
    Ok(build(
        max_retries,
        initial.value,
        maximum.value,
        sequence.value,
        after_cap.value,
        jitter.value,
    ))
}

#[cfg(any(test, feature = "sync-http"))]
fn resolve_retry(
    retry: ResolvedRetryFields,
    timeout: &ResolvedField<u64>,
    immediate: bool,
) -> Result<RetryPolicy, ConfigFailure> {
    validate_retry(
        retry,
        timeout,
        |max_retries, initial_backoff, max_backoff, sequence_timeout, retry_after_cap, jitter| {
            let delay = |value| {
                if immediate && value == 0 {
                    RetryDelay::Immediate
                } else {
                    RetryDelay::positive(PositiveDuration(Duration::from_millis(value)))
                }
            };
            RetryPolicy {
                max_retries,
                initial_backoff: delay(initial_backoff),
                max_backoff: delay(max_backoff),
                sequence_timeout: PositiveDuration(Duration::from_millis(sequence_timeout)),
                retry_after_cap: PositiveDuration(Duration::from_millis(retry_after_cap)),
                jitter: BoundedPercent(jitter),
            }
        },
    )
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

pub(super) fn config_failure_to_init_failure(error: ConfigFailure) -> InitError {
    InitError::Configuration {
        context: error.into_context(),
    }
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
        return host.parse::<std::net::Ipv6Addr>().is_ok()
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
