//! Blocking legacy OTLP/HTTP JSON transport.
//!
//! This module retains the immutable legacy request and payload behavior while
//! applying only the four authorized safety deltas: retry classification,
//! bounded server pacing/jitter, cancellation-aware retry waits, and an
//! overall retry-sequence deadline. The blocking client is owned exclusively
//! by one private worker thread.

#![allow(
    dead_code,
    reason = "D.18 wires this staged legacy backend into the facade after D.8"
)]

use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
#[cfg(test)]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};

use crate::config::{
    AuthHeader, BackendTransportBounds, OtelConfig, RetryPolicy, ValidatedTransportBounds,
    validated_backend_connection, validated_transport_bounds,
};
#[cfg(test)]
use crate::config::{ExporterBackend, OtlpProtocol};
use crate::contracts::{
    CompleteSpan, ExportRecord, ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter,
    LogRecord, MetricExporter, Resource, TraceExporter,
};
use crate::lifecycle::{LifecycleCore, SignalKind};
use sc_observability_types::otlp::{
    OtlpLogRecord, OtlpResource, group_records_by_resource_and_scope,
};
use sc_observability_types::v2::{
    AggregationTemporality, AttributeValue, ExportError, MetricRecord, MetricValue, SpanKind,
    SpanStatus, TelemetryError, TraceFlags,
};
use sc_observability_types::{ErrorContext, LogEvent, Remediation, Timestamp, error_codes};

const RETRY_AFTER_HEADER_LIMIT: usize = 128;
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[cfg(test)]
static RETRY_WAIT_HOOK: OnceLock<Mutex<Option<mpsc::Sender<()>>>> = OnceLock::new();

/// Per-worker gates control ordering, never the readiness result or timeout.
#[cfg(test)]
#[derive(Debug)]
pub(super) struct StartupTestHooks {
    pub(super) initialize: Mutex<Receiver<()>>,
    pub(super) receive: Mutex<Receiver<()>>,
    pub(super) ready: mpsc::Sender<()>,
    pub(super) exited: mpsc::Sender<bool>,
}

#[cfg(test)]
impl StartupTestHooks {
    fn wait(gate: &Mutex<Receiver<()>>) {
        gate.lock()
            .expect("startup gate lock")
            .recv_timeout(Duration::from_secs(10))
            .expect("test releases startup gate");
    }
}

#[derive(Debug, Clone)]
struct RetrySettings {
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
    sequence_timeout: Duration,
    retry_after_cap: Duration,
    jitter_percent: u8,
}

impl RetrySettings {
    fn from_policy(policy: &RetryPolicy) -> Self {
        Self {
            max_retries: policy.max_retries(),
            initial_backoff: policy.initial_backoff().get(),
            max_backoff: policy.max_backoff().get(),
            sequence_timeout: policy.sequence_timeout().get(),
            retry_after_cap: policy.retry_after_cap().get(),
            jitter_percent: policy.jitter().get(),
        }
    }
}

/// Validated inputs needed by the legacy worker.
#[derive(Debug, Clone)]
pub(crate) struct LegacyHttpJsonConfig {
    endpoint: String,
    auth_header: Option<String>,
    ca_file: Option<PathBuf>,
    insecure_skip_verify: bool,
    request_timeout: Duration,
    lifecycle_flush_timeout: Duration,
    lifecycle_shutdown_timeout: Duration,
    retry: RetrySettings,
    jitter_seed: u64,
    #[cfg(test)]
    retry_delay_observer: Option<SyncSender<Duration>>,
    #[cfg(test)]
    startup_hooks: Option<Arc<StartupTestHooks>>,
}

impl LegacyHttpJsonConfig {
    /// Builds a worker configuration from D21's already-validated contract.
    pub(crate) fn from_otel(
        config: &OtelConfig,
    ) -> Result<(Self, ValidatedTransportBounds), ExportError> {
        let bounds =
            validated_transport_bounds(config).map_err(|error| ExportError::Transport {
                context: error.into_context(),
            })?;
        let connection =
            validated_backend_connection(config).map_err(|error| ExportError::Transport {
                context: error.into_context(),
            })?;
        let BackendTransportBounds::Legacy(policy) = bounds.backend() else {
            return Err(transport_error(
                "legacy HTTP/JSON configuration was not validated for the legacy backend",
            ));
        };
        let endpoint = connection.endpoint().as_str().to_owned();
        Ok((
            Self {
                endpoint: endpoint.trim_end_matches('/').to_owned(),
                auth_header: connection
                    .auth_header()
                    .map(AuthHeader::as_str)
                    .map(str::to_owned),
                ca_file: connection.ca_file().cloned(),
                insecure_skip_verify: config.insecure_skip_verify,
                request_timeout: bounds.request_timeout().get(),
                lifecycle_flush_timeout: bounds.lifecycle().flush().get(),
                lifecycle_shutdown_timeout: bounds.lifecycle().shutdown().get(),
                retry: RetrySettings::from_policy(policy),
                jitter_seed: seed_from_os(),
                #[cfg(test)]
                retry_delay_observer: None,
                #[cfg(test)]
                startup_hooks: None,
            },
            bounds,
        ))
    }
}

enum DataCommand {
    Export {
        endpoint: String,
        body: String,
        complete: Box<dyn FnOnce(Result<(), ExportError>) + Send + 'static>,
    },
}

enum ControlCommand {
    Flush {
        result: mpsc::Sender<Result<(), ExportError>>,
    },
    Shutdown {
        result: Option<mpsc::Sender<Result<(), ExportError>>>,
    },
}

struct WorkerInner {
    data_tx: SyncSender<DataCommand>,
    control_tx: SyncSender<ControlCommand>,
    send_lock: Mutex<()>,
    cancel: Arc<AtomicBool>,
    terminated: AtomicBool,
    lifecycle_flush_timeout: Duration,
    lifecycle_shutdown_timeout: Duration,
}

impl Drop for WorkerInner {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        let _ = self
            .control_tx
            .try_send(ControlCommand::Shutdown { result: None });
    }
}

#[derive(Clone)]
struct Worker {
    inner: Arc<WorkerInner>,
}

impl Worker {
    fn start(config: LegacyHttpJsonConfig) -> Result<Self, ExportError> {
        let (data_tx, data_rx) = mpsc::sync_channel(64);
        let (control_tx, control_rx) = mpsc::sync_channel(1);
        let (ready_tx, ready_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let flush_timeout = config.lifecycle_flush_timeout;
        let shutdown_timeout = config.lifecycle_shutdown_timeout;
        let handshake_timeout = config
            .request_timeout
            .min(config.lifecycle_shutdown_timeout);
        #[cfg(test)]
        let startup_hooks = config.startup_hooks.clone();
        thread::Builder::new()
            .name("sc-otlp-legacy-http".to_owned())
            .spawn(move || {
                #[cfg(test)]
                let hooks = config.startup_hooks.clone();
                worker_main(config, data_rx, control_rx, ready_tx, &worker_cancel);
                #[cfg(test)]
                if let Some(hooks) = hooks {
                    // Sent only after worker_main returns and its client drops.
                    let _ = hooks.exited.send(worker_cancel.load(Ordering::Acquire));
                }
            })
            .map_err(|error| transport_error_with_source("failed to start legacy worker", error))?;
        #[cfg(test)]
        if let Some(hooks) = startup_hooks {
            StartupTestHooks::wait(&hooks.receive);
        }
        match ready_rx.recv_timeout(handshake_timeout) {
            Ok(Ok(())) => Ok(Self {
                inner: Arc::new(WorkerInner {
                    data_tx,
                    control_tx,
                    send_lock: Mutex::new(()),
                    cancel,
                    terminated: AtomicBool::new(false),
                    lifecycle_flush_timeout: flush_timeout,
                    lifecycle_shutdown_timeout: shutdown_timeout,
                }),
            }),
            Ok(Err(error)) => Err(error),
            Err(RecvTimeoutError::Timeout) => {
                cancel.store(true, Ordering::Release);
                Err(transport_error(
                    "legacy worker construction handshake exceeded its finite deadline",
                ))
            }
            Err(RecvTimeoutError::Disconnected) => Err(worker_terminated_error()),
        }
    }

    fn enqueue(
        &self,
        endpoint: String,
        body: String,
        complete: Box<dyn FnOnce(Result<(), ExportError>) + Send + 'static>,
    ) -> Result<(), ExportError> {
        if self.inner.terminated.load(Ordering::Acquire) {
            complete(Err(worker_terminated_error()));
            return Err(worker_terminated_error());
        }
        let _guard = self
            .inner
            .send_lock
            .lock()
            .expect("legacy worker send lock");
        match self.inner.data_tx.try_send(DataCommand::Export {
            endpoint,
            body,
            complete,
        }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(DataCommand::Export { complete, .. })) => {
                complete(Err(queue_full_error()));
                Err(queue_full_error())
            }
            Err(TrySendError::Disconnected(DataCommand::Export { complete, .. })) => {
                self.inner.terminated.store(true, Ordering::Release);
                complete(Err(worker_terminated_error()));
                Err(worker_terminated_error())
            }
        }
    }

    fn export(&self, endpoint: String, body: String) -> Result<(), ExportError> {
        let (result_tx, result_rx) = mpsc::channel();
        self.enqueue(
            endpoint,
            body,
            Box::new(move |result| {
                let _ = result_tx.send(result);
            }),
        )?;
        result_rx
            .recv_timeout(self.inner.lifecycle_shutdown_timeout)
            .unwrap_or_else(|_| Err(worker_terminated_error()))
    }

    fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::Release);
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        let _guard = self
            .inner
            .send_lock
            .lock()
            .expect("legacy worker control lock");
        let (tx, rx) = mpsc::channel();
        self.inner
            .control_tx
            .send(ControlCommand::Flush { result: tx })
            .map_err(|_| worker_terminated_error())?;
        rx.recv_timeout(self.inner.lifecycle_flush_timeout)
            .unwrap_or_else(|_| Err(worker_terminated_error()))
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        self.inner.cancel.store(true, Ordering::Release);
        let _guard = self
            .inner
            .send_lock
            .lock()
            .expect("legacy worker control lock");
        let (tx, rx) = mpsc::channel();
        self.inner
            .control_tx
            .send(ControlCommand::Shutdown { result: Some(tx) })
            .map_err(|_| worker_terminated_error())?;
        rx.recv_timeout(self.inner.lifecycle_shutdown_timeout)
            .unwrap_or_else(|_| Err(worker_terminated_error()))
    }
}

impl ExporterLifecycle for Worker {
    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        let worker = self.clone();
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            thread::spawn(move || {
                let _ = tx.send(worker.flush_blocking());
            });
            rx.await.unwrap_or_else(|_| Err(worker_terminated_error()))
        })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        let worker = self.clone();
        Box::pin(async move {
            worker.cancel();
            let (tx, rx) = tokio::sync::oneshot::channel();
            thread::spawn(move || {
                let _ = tx.send(worker.shutdown_blocking());
            });
            rx.await.unwrap_or_else(|_| Err(worker_terminated_error()))
        })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        Worker::flush_blocking(self)
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        Worker::shutdown_blocking(self)
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the dedicated worker takes ownership of its channels and validated config"
)]
fn worker_main(
    config: LegacyHttpJsonConfig,
    data_rx: Receiver<DataCommand>,
    control_rx: Receiver<ControlCommand>,
    ready_tx: mpsc::Sender<Result<(), ExportError>>,
    cancel: &AtomicBool,
) {
    #[cfg(test)]
    if let Some(hooks) = &config.startup_hooks {
        StartupTestHooks::wait(&hooks.initialize);
    }
    let client = match build_client(&config) {
        Ok(client) => {
            // A timed-out constructor never publishes a handle. Dispose of
            // the late client here, on its owning worker, instead of entering
            // the command loop after startup has already been abandoned.
            if cancel.load(Ordering::Acquire) || ready_tx.send(Ok(())).is_err() {
                return;
            }
            #[cfg(test)]
            if let Some(hooks) = &config.startup_hooks {
                let _ = hooks.ready.send(());
            }
            client
        }
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return;
        }
    };
    loop {
        if let Ok(command) = control_rx.try_recv() {
            drain_data(&client, &config, &data_rx, cancel);
            if handle_control(command, cancel) {
                return;
            }
            continue;
        }
        match data_rx.recv_timeout(WORKER_POLL_INTERVAL) {
            Ok(DataCommand::Export {
                endpoint,
                body,
                complete,
            }) => {
                let outcome = send_with_retries(&client, &config, &endpoint, &body, cancel);
                complete(outcome);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn drain_data(
    client: &Client,
    config: &LegacyHttpJsonConfig,
    data_rx: &Receiver<DataCommand>,
    cancel: &AtomicBool,
) {
    while let Ok(DataCommand::Export {
        endpoint,
        body,
        complete,
    }) = data_rx.try_recv()
    {
        let outcome = send_with_retries(client, config, &endpoint, &body, cancel);
        complete(outcome);
    }
}

fn handle_control(command: ControlCommand, cancel: &AtomicBool) -> bool {
    match command {
        ControlCommand::Flush { result } => {
            let _ = result.send(Ok(()));
            false
        }
        ControlCommand::Shutdown { result } => {
            cancel.store(true, Ordering::Release);
            if let Some(result) = result {
                let _ = result.send(Ok(()));
            }
            true
        }
    }
}

fn build_client(config: &LegacyHttpJsonConfig) -> Result<Client, ExportError> {
    let mut builder = ClientBuilder::new().timeout(config.request_timeout);
    if config.insecure_skip_verify {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(path) = &config.ca_file {
        let raw = fs::read(path)
            .map_err(|error| transport_error_with_source("failed to read CA bundle", error))?;
        let cert = reqwest::Certificate::from_pem(&raw)
            .or_else(|_| reqwest::Certificate::from_der(&raw))
            .map_err(|error| transport_error_with_source("failed to parse CA bundle", error))?;
        builder = builder.add_root_certificate(cert);
    }
    if let Some(raw) = &config.auth_header {
        let (name, value) = parse_auth_header(raw)?;
        let mut headers = HeaderMap::new();
        headers.insert(name, value);
        builder = builder.default_headers(headers);
    }
    builder
        .build()
        .map_err(|error| transport_error_with_source("failed to build HTTP client", error))
}

fn parse_auth_header(raw: &str) -> Result<(HeaderName, HeaderValue), ExportError> {
    let Some((name, value)) = raw.split_once(':') else {
        return Err(transport_error(
            "authorization header must use `name: value` syntax",
        ));
    };
    let name = HeaderName::from_bytes(name.trim().as_bytes())
        .map_err(|error| transport_error_with_source("invalid authorization header name", error))?;
    let value = HeaderValue::from_str(value.trim()).map_err(|error| {
        transport_error_with_source("invalid authorization header value", error)
    })?;
    Ok((name, value))
}

fn send_with_retries(
    client: &Client,
    config: &LegacyHttpJsonConfig,
    endpoint: &str,
    body: &str,
    cancel: &AtomicBool,
) -> Result<(), ExportError> {
    let started = Instant::now();
    let mut attempt = 0_u32;
    let mut fallback = config.retry.initial_backoff;
    let mut rng = config.jitter_seed;
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(shutdown_cancelled_error());
        }
        let remaining = config
            .retry
            .sequence_timeout
            .saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(retry_deadline_error());
        }
        let response = client
            .post(endpoint)
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_owned())
            .send();
        match response {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                let status = response.status();
                if !is_retryable_status(status) {
                    return Err(non_retryable_status_error(status.as_u16()));
                }
                if attempt >= config.retry.max_retries {
                    return Err(retry_attempts_exhausted_error(status.as_u16()));
                }
                let server_delay = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| parse_retry_after(value, SystemTime::now()))
                    .map(|delay| delay.min(config.retry.retry_after_cap));
                let delay = server_delay.unwrap_or(fallback.min(config.retry.max_backoff));
                let delay =
                    apply_jitter(delay, config.retry.jitter_percent, &mut rng).min(remaining);
                if !wait_for_retry(config, delay, cancel) {
                    return Err(shutdown_cancelled_error());
                }
            }
            Err(error) => {
                if attempt >= config.retry.max_retries {
                    return Err(retry_attempts_exhausted_with_source(error));
                }
                let delay = apply_jitter(
                    fallback.min(config.retry.max_backoff).min(remaining),
                    config.retry.jitter_percent,
                    &mut rng,
                );
                if !wait_for_retry(config, delay, cancel) {
                    return Err(shutdown_cancelled_error());
                }
            }
        }
        attempt = attempt.saturating_add(1);
        fallback = fallback.saturating_mul(2).min(config.retry.max_backoff);
    }
}

pub(super) fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

pub(super) fn parse_retry_after(value: &str, now: SystemTime) -> Option<Duration> {
    let value = value.trim();
    if value.len() > RETRY_AFTER_HEADER_LIMIT {
        return None;
    }
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = httpdate::parse_http_date(value).ok()?;
    date.duration_since(now).ok()
}

pub(super) fn wait_cancelable(duration: Duration, cancel: &AtomicBool) -> bool {
    wait_cancelable_with_observer(duration, cancel, None)
}

fn wait_for_retry(config: &LegacyHttpJsonConfig, duration: Duration, cancel: &AtomicBool) -> bool {
    #[cfg(test)]
    let observer = config.retry_delay_observer.as_ref();
    #[cfg(not(test))]
    let _ = config;
    #[cfg(not(test))]
    let observer = None;

    wait_cancelable_with_observer(duration, cancel, observer)
}

fn wait_cancelable_with_observer(
    duration: Duration,
    cancel: &AtomicBool,
    observer: Option<&SyncSender<Duration>>,
) -> bool {
    #[cfg(not(test))]
    let _ = observer;
    #[cfg(test)]
    {
        notify_retry_wait_started();
        if let Some(sender) = observer {
            let _ = sender.try_send(duration);
        }
    }

    let deadline = Instant::now() + duration;
    while !cancel.load(Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return true;
        }
        thread::sleep(remaining.min(WORKER_POLL_INTERVAL));
    }
    false
}

#[cfg(test)]
pub(super) fn install_retry_wait_hook(sender: mpsc::Sender<()>) {
    let hook = RETRY_WAIT_HOOK.get_or_init(|| Mutex::new(None));
    *hook.lock().expect("retry wait hook lock") = Some(sender);
}

#[cfg(test)]
pub(super) fn clear_retry_wait_hook() {
    if let Some(hook) = RETRY_WAIT_HOOK.get() {
        *hook.lock().expect("retry wait hook lock") = None;
    }
}

#[cfg(test)]
fn notify_retry_wait_started() {
    let sender = RETRY_WAIT_HOOK
        .get()
        .and_then(|hook| hook.lock().expect("retry wait hook lock").clone());
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
}

fn apply_jitter(delay: Duration, percent: u8, state: &mut u64) -> Duration {
    if percent == 0 || delay.is_zero() {
        return delay;
    }
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    let spread = (delay.as_millis() * u128::from(u32::from(percent))) / 100;
    let span = spread.saturating_mul(2).saturating_add(1);
    let span = u64::try_from(span.min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
    let sample = u128::from(*state % span);
    let millis = if sample >= spread {
        delay.as_millis().saturating_add(sample - spread)
    } else {
        delay.as_millis().saturating_sub(spread - sample)
    }
    .max(1);
    Duration::from_millis(u64::try_from(millis.min(u128::from(u64::MAX))).unwrap_or(u64::MAX))
}

fn seed_from_os() -> u64 {
    let mut seed = [0_u8; 8];
    if getrandom::fill(&mut seed).is_ok() {
        return u64::from_le_bytes(seed);
    }
    0xa5a5_5a5a_1234_5678
}

/// Legacy terminal backend shared by all three OTLP signal families.
///
/// `Worker` is the terminal transport and `LifecycleCore` is the only
/// admission/barrier owner. Signal adapters retain this pair through one
/// `Arc`, so the HTTP worker never grows a competing lifecycle state machine.
struct LegacyBackend {
    worker: Worker,
    lifecycle: LifecycleCore,
}

impl LegacyBackend {
    fn new(
        worker_config: LegacyHttpJsonConfig,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Arc<Self>, ExportError> {
        let worker = Worker::start(worker_config)?;
        let terminal: Arc<dyn ExporterLifecycle> = Arc::new(worker.clone());
        let lifecycle = LifecycleCore::from_backend(terminal, bounds)?;
        Ok(Arc::new(Self { worker, lifecycle }))
    }
}

/// Legacy exporter shared by all three OTLP signal families.
pub(crate) struct OtlpHttpExporter {
    backend: Arc<LegacyBackend>,
    endpoint: String,
}

impl OtlpHttpExporter {
    /// Creates the legacy exporter from D21's validated transport config.
    pub(crate) fn from_config(config: &OtelConfig) -> Result<Self, ExportError> {
        let (worker_config, bounds) = LegacyHttpJsonConfig::from_otel(config)?;
        let endpoint = worker_config.endpoint.clone();
        Ok(Self {
            backend: LegacyBackend::new(worker_config, &bounds)?,
            endpoint,
        })
    }

    #[cfg(test)]
    pub(super) fn for_endpoint(endpoint: String) -> Result<Self, ExportError> {
        Self::for_test_config(endpoint, None, None)
    }

    /// Exercises real client construction and `Worker::start` with ordered gates.
    #[cfg(test)]
    pub(super) fn for_startup_test(hooks: Arc<StartupTestHooks>) -> Result<Self, ExportError> {
        let mut config = OtelConfig::new(ExporterBackend::LegacyHttpJson, OtlpProtocol::HttpJson);
        config.enabled = true;
        config.endpoint = Some(
            crate::config::OtlpEndpoint::new_typed("http://127.0.0.1:1")
                .expect("test endpoint is valid"),
        );
        // A queued readiness message wins even at this short boundary. The
        // gates keep both cases independent of worker scheduling speed.
        config.timeout_ms = Some(1_u64.into());
        let (mut worker_config, bounds) = LegacyHttpJsonConfig::from_otel(&config)?;
        worker_config.startup_hooks = Some(hooks);
        let endpoint = worker_config.endpoint.clone();
        Ok(Self {
            backend: LegacyBackend::new(worker_config, &bounds)?,
            endpoint,
        })
    }

    /// Builds the retained backend with the configuration seams exercised by
    /// the immutable D.8 transport fixtures. This is test-only so production
    /// construction continues to enter solely through D21's validated config.
    #[cfg(test)]
    pub(super) fn for_test_config(
        endpoint: String,
        auth_header: Option<&str>,
        ca_file: Option<PathBuf>,
    ) -> Result<Self, ExportError> {
        let mut config = OtelConfig::new(ExporterBackend::LegacyHttpJson, OtlpProtocol::HttpJson);
        config.enabled = true;
        config.endpoint = Some(
            crate::config::OtlpEndpoint::new_typed(endpoint.clone())
                .expect("loopback test endpoint is valid"),
        );
        config.auth_header = auth_header.map(|header| {
            AuthHeader::new_typed(header).expect("test authorization header is valid")
        });
        config.ca_file = ca_file;
        let (worker_config, bounds) = LegacyHttpJsonConfig::from_otel(&config)?;
        Ok(Self {
            backend: LegacyBackend::new(worker_config, &bounds)?,
            endpoint,
        })
    }

    #[cfg(test)]
    pub(super) fn for_endpoint_with_retry(
        endpoint: String,
        retry: crate::config::LegacyRetryPolicy,
        jitter_seed: u64,
        retry_delay_observer: Option<SyncSender<Duration>>,
    ) -> Result<Self, ExportError> {
        let sequence_timeout_ms = retry.retry_sequence_timeout_ms.map_or(3_000, u64::from);
        let mut config = OtelConfig::new(ExporterBackend::LegacyHttpJson, OtlpProtocol::HttpJson);
        config.enabled = true;
        config.timeout_ms = Some(sequence_timeout_ms.saturating_sub(1).clamp(1, 100).into());
        config.endpoint = Some(
            crate::config::OtlpEndpoint::new_typed(endpoint.clone())
                .expect("loopback test endpoint is valid"),
        );
        config.legacy_retry = Some(retry);
        let (mut worker_config, bounds) = LegacyHttpJsonConfig::from_otel(&config)?;
        worker_config.jitter_seed = jitter_seed;
        worker_config.retry_delay_observer = retry_delay_observer;
        Ok(Self {
            backend: LegacyBackend::new(worker_config, &bounds)?,
            endpoint,
        })
    }

    fn send_payload(
        &self,
        signal: SignalKind,
        endpoint_signal: &str,
        batch: impl Send + 'static,
        payload: &Value,
    ) -> Result<(), ExportError> {
        let body = payload.to_string();
        let admitted = self
            .backend
            .lifecycle
            .admit(signal, batch, body.len())
            .map_err(telemetry_error_to_export_error)?;
        let endpoint = normalize_signal_endpoint(&self.endpoint, endpoint_signal);
        self.backend.worker.enqueue(
            endpoint,
            body,
            Box::new(move |result| {
                let _ = admitted.complete(result);
            }),
        )
    }

    #[cfg(test)]
    pub(super) fn send_payload_sync(
        &self,
        signal: &str,
        payload: &Value,
    ) -> Result<(), ExportError> {
        let endpoint = normalize_signal_endpoint(&self.endpoint, signal);
        self.backend.worker.export(endpoint, payload.to_string())
    }
}

impl LogExporter<LogEvent> for OtlpHttpExporter {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError> {
        let records = batch.iter().map(log_record).collect::<Vec<_>>();
        self.send_payload(
            SignalKind::Logs,
            "logs",
            batch.to_vec(),
            &build_logs_payload(&records),
        )
    }
}

impl LogExporter<ExportRecord<LogRecord>> for OtlpHttpExporter {
    fn export_logs(&self, batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        self.send_payload(
            SignalKind::Logs,
            "logs",
            batch.to_vec(),
            &build_logs_payload(batch),
        )
    }
}

impl TraceExporter<CompleteSpan> for OtlpHttpExporter {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError> {
        let records = batch.iter().map(span_record).collect::<Vec<_>>();
        self.send_payload(
            SignalKind::Traces,
            "traces",
            batch.to_vec(),
            &build_traces_payload(&records),
        )
    }
}

impl TraceExporter<ExportRecord<CompleteSpan>> for OtlpHttpExporter {
    fn export_spans(&self, batch: &[ExportRecord<CompleteSpan>]) -> Result<(), ExportError> {
        self.send_payload(
            SignalKind::Traces,
            "traces",
            batch.to_vec(),
            &build_traces_payload(batch),
        )
    }
}

impl MetricExporter<MetricRecord> for OtlpHttpExporter {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError> {
        let records = batch.iter().map(metric_record).collect::<Vec<_>>();
        self.send_payload(
            SignalKind::Metrics,
            "metrics",
            batch.to_vec(),
            &build_metrics_payload(&records),
        )
    }
}

impl MetricExporter<ExportRecord<MetricRecord>> for OtlpHttpExporter {
    fn export_metrics(&self, batch: &[ExportRecord<MetricRecord>]) -> Result<(), ExportError> {
        self.send_payload(
            SignalKind::Metrics,
            "metrics",
            batch.to_vec(),
            &build_metrics_payload(batch),
        )
    }
}

impl ExporterLifecycle for OtlpHttpExporter {
    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        let lifecycle = self.backend.lifecycle.clone();
        Box::pin(async move { lifecycle.flush_async().await })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        let backend = Arc::clone(&self.backend);
        Box::pin(async move {
            backend.worker.cancel();
            backend.lifecycle.shutdown_async().await
        })
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(blocking_in_async_error());
        }
        block_on_lifecycle(self.backend.lifecycle.flush_async())
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(blocking_in_async_error());
        }
        self.backend.worker.cancel();
        block_on_lifecycle(self.backend.lifecycle.shutdown_async())
    }
}

/// Builds the crate-private exporter set consumed by the D18 facade handoff.
pub(crate) fn build_exporter_set(config: &OtelConfig) -> Result<ExporterSet, ExportError> {
    let exporter = Arc::new(OtlpHttpExporter::from_config(config)?);
    Ok(ExporterSet {
        logs: exporter.clone(),
        traces: exporter.clone(),
        metrics: exporter.clone(),
        lifecycle: exporter,
    })
}

pub(super) fn log_record(event: &LogEvent) -> ExportRecord<LogRecord> {
    ExportRecord {
        resource: Resource {
            attributes: BTreeMap::from_iter([(
                "service.name".to_owned(),
                AttributeValue::String(event.service.as_str().to_owned()),
            )]),
            schema_url: None,
        },
        scope: crate::contracts::InstrumentationScope::default(),
        record: OtlpLogRecord {
            event: event.clone(),
            trace_flags: event
                .trace
                .as_ref()
                .map_or(TraceFlags::default(), |_| TraceFlags::default()),
            attributes: BTreeMap::new(),
        },
    }
}

fn span_record(span: &CompleteSpan) -> ExportRecord<CompleteSpan> {
    ExportRecord {
        resource: Resource {
            attributes: BTreeMap::from_iter([(
                "service.name".to_owned(),
                AttributeValue::String(span.record.service().as_str().to_owned()),
            )]),
            schema_url: None,
        },
        scope: crate::contracts::InstrumentationScope::default(),
        record: span.clone(),
    }
}

fn metric_record(metric: &MetricRecord) -> ExportRecord<MetricRecord> {
    ExportRecord {
        resource: Resource {
            attributes: BTreeMap::from_iter([(
                "service.name".to_owned(),
                AttributeValue::String(metric.service().as_str().to_owned()),
            )]),
            schema_url: None,
        },
        scope: crate::contracts::InstrumentationScope::default(),
        record: metric.clone(),
    }
}

pub(super) fn build_logs_payload(records: &[ExportRecord<LogRecord>]) -> Value {
    let resource_logs = group_records_by_resource_and_scope(records)
        .into_iter()
        .map(|group| {
            json!({
                "resource": resource_json(&group.resource),
                "scopeLogs": group.scopes.into_iter().map(|scope| json!({
                    "scope": scope_json(&scope.scope),
                    "logRecords": scope.records.into_iter().map(log_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({ "resourceLogs": resource_logs })
}

fn build_traces_payload(records: &[ExportRecord<CompleteSpan>]) -> Value {
    let resource_spans = group_records_by_resource_and_scope(records)
        .into_iter()
        .map(|group| {
            json!({
                "resource": resource_json(&group.resource),
                "scopeSpans": group.scopes.into_iter().map(|scope| json!({
                    "scope": scope_json(&scope.scope),
                    "spans": scope.records.into_iter().map(span_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({ "resourceSpans": resource_spans })
}

fn build_metrics_payload(records: &[ExportRecord<MetricRecord>]) -> Value {
    let resource_metrics = group_records_by_resource_and_scope(records)
        .into_iter()
        .map(|group| {
            json!({
                "resource": resource_json(&group.resource),
                "scopeMetrics": group.scopes.into_iter().map(|scope| json!({
                    "scope": scope_json(&scope.scope),
                    "metrics": scope.records.iter().map(metric_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({ "resourceMetrics": resource_metrics })
}

fn resource_json(resource: &OtlpResource) -> Value {
    json!({
        "attributes": resource.attributes.iter().map(|(key, value)| json!({
            "key": key,
            "value": attribute_json(value),
        })).collect::<Vec<_>>(),
        "schemaUrl": resource.schema_url,
    })
}

fn scope_json(scope: &crate::contracts::InstrumentationScope) -> Value {
    json!({
        "name": scope.name,
        "version": scope.version,
        "attributes": scope.attributes.iter().map(|(key, value)| json!({
            "key": key,
            "value": attribute_json(value),
        })).collect::<Vec<_>>(),
        "schemaUrl": scope.schema_url,
    })
}

fn log_json(record: LogRecord) -> Value {
    let event = record.event;
    let mut attributes = record
        .attributes
        .iter()
        .map(|(key, value)| json!({ "key": key, "value": attribute_json(value) }))
        .collect::<Vec<_>>();
    attributes.push(json!({
        "key": "log.target",
        "value": { "stringValue": event.target.as_str() },
    }));
    attributes.push(json!({
        "key": "event.name",
        "value": { "stringValue": event.action.as_str() },
    }));
    for (key, value) in event.fields {
        attributes.push(json!({ "key": key, "value": json_value_to_otlp_any(&value) }));
    }
    if let Some(trace) = event.trace {
        attributes.push(json!({
            "key": "trace_id",
            "value": { "stringValue": trace.trace_id.as_str() },
        }));
        attributes.push(json!({
            "key": "span_id",
            "value": { "stringValue": trace.span_id.as_str() },
        }));
    }
    let (severity_number, severity_text) = severity_fields(event.level);
    json!({
        "timeUnixNano": timestamp_nanos(event.timestamp),
        "body": { "stringValue": event.message.unwrap_or_else(|| event.action.as_str().to_owned()) },
        "severityNumber": severity_number,
        "severityText": severity_text,
        "attributes": attributes,
    })
}

fn span_json(span: CompleteSpan) -> Value {
    let record = span.record;
    let trace = record.trace();
    let mut result = json!({
        "traceId": trace.trace_id.as_str(),
        "spanId": trace.span_id.as_str(),
        "name": record.name().as_str(),
        "kind": span_kind_number(record.kind()),
        "startTimeUnixNano": timestamp_nanos(record.timestamp()),
        "endTimeUnixNano": timestamp_nanos_with_duration(record.timestamp(), record.duration_ms().as_u64()),
        "status": { "code": span_status_number(record.status()) },
        "attributes": record.attributes().iter().map(|(key, value)| json!({
            "key": key,
            "value": attribute_json(value),
        })).collect::<Vec<_>>(),
        "events": span.events.into_iter().map(|event| json!({
            "timeUnixNano": timestamp_nanos(event.timestamp),
            "name": event.name.as_str(),
            "attributes": event.attributes.iter().map(|(key, value)| json!({
                "key": key,
                "value": attribute_json(value),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    if let Some(parent) = &trace.parent_span_id {
        result["parentSpanId"] = json!(parent.as_str());
    }
    result
}

fn metric_json(metric: &MetricRecord) -> Value {
    let attrs = metric
        .attributes()
        .iter()
        .map(|(key, value)| json!({ "key": key, "value": attribute_json(value) }))
        .collect::<Vec<_>>();
    let point_time = timestamp_nanos(metric.timestamp());
    let data_point =
        |value: f64| json!({ "attributes": attrs, "timeUnixNano": point_time, "asDouble": value });
    let mut result = json!({
        "name": metric.name().as_str(),
        "unit": metric.unit().map(ToString::to_string).unwrap_or_default(),
    });
    match metric.value() {
        MetricValue::Gauge(value) => {
            result["gauge"] = json!({ "dataPoints": [data_point(value.get())] });
        }
        MetricValue::Sum {
            value,
            monotonic,
            temporality,
            start_time,
        } => {
            let mut point = data_point(value.get());
            point["startTimeUnixNano"] = json!(timestamp_nanos(*start_time));
            result["sum"] = json!({
                "aggregationTemporality": temporality_number(*temporality),
                "isMonotonic": monotonic,
                "dataPoints": [point],
            });
        }
        MetricValue::Histogram {
            point,
            temporality,
            start_time,
        } => {
            result["histogram"] = json!({
                "aggregationTemporality": temporality_number(*temporality),
                "dataPoints": [{
                    "attributes": attrs,
                    "startTimeUnixNano": timestamp_nanos(*start_time),
                    "timeUnixNano": point_time,
                    "count": point.count(),
                    "sum": point.sum().get(),
                    "bucketCounts": point.bucket_counts(),
                    "explicitBounds": point.explicit_bounds().iter().map(|bound| bound.get()).collect::<Vec<_>>(),
                }],
            });
        }
        _ => {}
    }
    result
}

fn attribute_json(value: &AttributeValue) -> Value {
    match value {
        AttributeValue::Bool(value) => json!({ "boolValue": value }),
        AttributeValue::Int(value) => json!({ "intValue": value.to_string() }),
        AttributeValue::UInt(value) => json!({ "intValue": value.to_string() }),
        AttributeValue::Float(value) => json!({ "doubleValue": value.get() }),
        AttributeValue::String(value) => json!({ "stringValue": value }),
        AttributeValue::Null => json!({ "stringValue": "null" }),
        AttributeValue::Array(values) => json!({
            "arrayValue": { "values": values.iter().map(attribute_json).collect::<Vec<_>>() }
        }),
        AttributeValue::Object(values) => json!({
            "kvlistValue": { "values": values.iter().map(|(key, value)| json!({
                "key": key,
                "value": attribute_json(value),
            })).collect::<Vec<_>>() }
        }),
        _ => json!({ "stringValue": "unsupported" }),
    }
}

fn json_value_to_otlp_any(value: &Value) -> Value {
    match value {
        Value::Null => json!({ "stringValue": "null" }),
        Value::Bool(value) => json!({ "boolValue": value }),
        Value::Number(value) if value.is_i64() => {
            json!({ "intValue": value.as_i64().unwrap_or_default().to_string() })
        }
        Value::Number(value) if value.is_u64() => {
            json!({ "intValue": value.as_u64().unwrap_or_default().to_string() })
        }
        Value::Number(value) => json!({ "doubleValue": value.as_f64().unwrap_or_default() }),
        Value::String(value) => json!({ "stringValue": value }),
        Value::Array(values) => json!({
            "arrayValue": { "values": values.iter().map(json_value_to_otlp_any).collect::<Vec<_>>() }
        }),
        Value::Object(values) => json!({
            "kvlistValue": { "values": values.iter().map(|(key, value)| json!({
                "key": key,
                "value": json_value_to_otlp_any(value),
            })).collect::<Vec<_>>() }
        }),
    }
}

fn timestamp_nanos(timestamp: Timestamp) -> String {
    timestamp.into_inner().unix_timestamp_nanos().to_string()
}

fn timestamp_nanos_with_duration(timestamp: Timestamp, duration_ms: u64) -> String {
    (timestamp.into_inner().unix_timestamp_nanos() + i128::from(duration_ms) * 1_000_000)
        .to_string()
}

fn severity_fields(level: sc_observability_types::Level) -> (u32, &'static str) {
    match level {
        sc_observability_types::Level::Trace => (1, "TRACE"),
        sc_observability_types::Level::Debug => (5, "DEBUG"),
        sc_observability_types::Level::Info => (9, "INFO"),
        sc_observability_types::Level::Warn => (13, "WARN"),
        sc_observability_types::Level::Error => (17, "ERROR"),
    }
}

fn span_kind_number(kind: SpanKind) -> u8 {
    match kind {
        SpanKind::Internal => 1,
        SpanKind::Server => 2,
        SpanKind::Client => 3,
        SpanKind::Producer => 4,
        SpanKind::Consumer => 5,
        _ => 0,
    }
}

fn span_status_number(status: SpanStatus) -> &'static str {
    match status {
        SpanStatus::Unset => "STATUS_CODE_UNSET",
        SpanStatus::Ok => "STATUS_CODE_OK",
        SpanStatus::Error => "STATUS_CODE_ERROR",
    }
}

fn temporality_number(temporality: AggregationTemporality) -> u8 {
    match temporality {
        AggregationTemporality::Delta => 1,
        AggregationTemporality::Cumulative => 2,
        _ => 0,
    }
}

pub(super) fn normalize_logs_endpoint(endpoint: &str) -> String {
    normalize_signal_endpoint(endpoint, "logs")
}

fn normalize_signal_endpoint(endpoint: &str, signal: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let suffix = format!("/v1/{signal}");
    if endpoint.ends_with(&suffix) {
        endpoint.to_owned()
    } else {
        format!("{endpoint}{suffix}")
    }
}

fn telemetry_error_to_export_error(error: TelemetryError) -> ExportError {
    match error {
        TelemetryError::ExportFailure(error) => error,
        TelemetryError::Shutdown { context } => ExportError::TerminalExportFailure { context },
        _ => ExportError::TerminalExportFailure {
            context: Box::new(error_with_code(
                error_codes::otlp::OTLP_EXPORT_TERMINAL,
                "legacy admission returned an unknown telemetry error",
            )),
        },
    }
}

struct ThreadParker(thread::Thread);

impl Wake for ThreadParker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on_lifecycle(mut future: crate::lifecycle::LifecycleWaiter) -> Result<(), ExportError> {
    let waker = Waker::from(Arc::new(ThreadParker(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        match Pin::new(&mut future).poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => thread::park_timeout(WORKER_POLL_INTERVAL),
        }
    }
}

fn transport_error(message: &str) -> ExportError {
    ExportError::Transport {
        context: Box::new(ErrorContext::new(
            error_codes::otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
            message,
            Remediation::recoverable("inspect the OTLP transport configuration", [] as [&str; 0]),
        )),
    }
}

fn transport_error_with_source(
    message: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> ExportError {
    ExportError::Transport {
        context: Box::new(
            ErrorContext::new(
                error_codes::otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
                message,
                Remediation::recoverable(
                    "inspect the OTLP transport configuration",
                    [] as [&str; 0],
                ),
            )
            .source(Box::new(error)),
        ),
    }
}

fn error_with_code(code: sc_observability_types::ErrorCode, message: &str) -> ErrorContext {
    ErrorContext::new(
        code,
        message,
        Remediation::recoverable("inspect telemetry health and retry", [] as [&str; 0]),
    )
}

fn worker_terminated_error() -> ExportError {
    ExportError::WorkerTerminated {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_WORKER_TERMINATED,
            "legacy HTTP worker terminated",
        )),
    }
}

fn queue_full_error() -> ExportError {
    ExportError::QueueFull {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_QUEUE_FULL,
            "legacy HTTP worker admission is full",
        )),
    }
}

fn shutdown_cancelled_error() -> ExportError {
    ExportError::ShutdownCancelledRetry {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_SHUTDOWN_CANCELLED_RETRY,
            "legacy retry was cancelled by shutdown",
        )),
    }
}

fn retry_deadline_error() -> ExportError {
    ExportError::RetryDeadlineExhausted {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_RETRY_DEADLINE_EXHAUSTED,
            "legacy retry sequence exceeded its deadline",
        )),
    }
}

fn retry_attempts_exhausted_error(status: u16) -> ExportError {
    ExportError::RetryAttemptsExhausted {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED,
            &format!(
                "legacy collector returned retryable HTTP status {status} until attempts were exhausted"
            ),
        )),
    }
}

fn retry_attempts_exhausted_with_source(error: reqwest::Error) -> ExportError {
    ExportError::RetryAttemptsExhausted {
        context: Box::new(
            error_with_code(
                error_codes::otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED,
                "legacy transport retries were exhausted",
            )
            .source(Box::new(error)),
        ),
    }
}

fn non_retryable_status_error(status: u16) -> ExportError {
    ExportError::NonRetryableHttpStatus {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_HTTP_STATUS_TERMINAL,
            &format!("legacy collector returned terminal HTTP status {status}"),
        )),
    }
}

fn blocking_in_async_error() -> ExportError {
    ExportError::BlockingBackendInAsyncContext {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT,
            "blocking legacy lifecycle cannot run from an entered Tokio runtime",
        )),
    }
}
