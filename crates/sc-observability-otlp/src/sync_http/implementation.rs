//! Blocking synchronous HTTP OTLP/HTTP JSON transport.
//!
//! This module retains the immutable synchronous HTTP request and payload behavior while
//! applying only the four authorized safety deltas: retry classification,
//! bounded server pacing/jitter, cancellation-aware retry waits, and an
//! overall retry-sequence deadline. The blocking client is owned exclusively
//! by one private worker thread.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "D.18 wires this staged synchronous HTTP backend into the facade after D.8"
    )
)]

use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::config::{
    AuthHeader, BackendTransportBounds, RetryPolicy, ValidatedBackendConnection,
    ValidatedTransportBounds,
};
#[cfg(test)]
use crate::config::{ExporterBackend, OtlpProtocol};
#[cfg(any(test, feature = "durable-store"))]
use crate::config::{OtelConfig, prepared_backend_connection, validated_transport_bounds};
use crate::constants::PROFILES_EXPORT_PATH;
use crate::contracts::{
    CompleteSpan, ExportRecord, ExporterLifecycle, ExporterSet, LifecycleFuture, LogExporter,
    LogRecord, MetricExporter, TraceExporter,
};
use crate::lifecycle::{LifecycleCore, LifecycleState, Signal};
use sc_observability_types::v2::{ExportError, MetricRecord, TelemetryError};
use sc_observability_types::{ErrorContext, LogEvent, Remediation, error_codes};

macro_rules! observed_request_timeout {
    ($config:expr, $attempt:expr, $timeout:expr, $remaining:expr) => {{
        let timeout = $timeout;
        #[cfg(test)]
        if let Some(observer) = &$config.request_timeout_observer {
            let _ = observer.send(($attempt, timeout, $remaining));
        }
        timeout
    }};
}

// Preserve the existing sibling-test paths while sharing the payload module
// with transport submission; no public or crate-private exporter API changes.
pub(super) use super::payload::{build_logs_payload, log_record};
use super::payload::{build_metrics_payload, build_traces_payload, metric_record, span_record};

/// Caps untrusted server-provided `Retry-After` values before date or integer parsing.
///
/// 128 bytes leaves room for the standard HTTP-date and integer-seconds forms,
/// including long delays, while rejecting oversized input. Raising this limit
/// would allow larger remote-controlled values into the parser.
const RETRY_AFTER_HEADER_LIMIT: usize = 128;

/// Bounds idle worker and cancellation polling while avoiding a tight busy loop.
///
/// Five milliseconds keeps shutdown/control observation responsive without
/// repeatedly waking the worker at CPU speed; changing it shifts that latency/CPU trade-off.
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(5);

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

/// Validated inputs needed by the synchronous HTTP worker.
#[derive(Debug, Clone)]
pub(crate) struct SyncHttpConfig {
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
    retry_delay_observer: Option<Sender<Duration>>,
    #[cfg(test)]
    request_timeout_observer: Option<Sender<(u32, Duration, Duration)>>,
    #[cfg(test)]
    control_submission_observer: Option<Sender<()>>,
    #[cfg(test)]
    startup_hooks: Option<Arc<StartupTestHooks>>,
}

impl SyncHttpConfig {
    /// Builds a worker configuration from D21's already-validated contract.
    #[cfg(any(test, feature = "durable-store"))]
    pub(crate) fn from_otel(
        config: &OtelConfig,
    ) -> Result<(Self, ValidatedTransportBounds), ExportError> {
        let bounds =
            validated_transport_bounds(config).map_err(|error| ExportError::Transport {
                context: error.into_context(),
            })?;
        let connection = prepared_backend_connection(config, &bounds).map_err(|error| {
            ExportError::Transport {
                context: error.into_context(),
            }
        })?;
        Ok((Self::from_prepared(&connection, &bounds)?, bounds))
    }

    pub(super) fn from_prepared(
        connection: &ValidatedBackendConnection,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, ExportError> {
        let BackendTransportBounds::SyncHttp(policy) = bounds.backend() else {
            return Err(transport_error(
                "prepared transport is not the synchronous HTTP backend",
            ));
        };
        Ok(Self {
            endpoint: connection
                .endpoint()
                .as_str()
                .trim_end_matches('/')
                .to_owned(),
            auth_header: connection
                .auth_header()
                .map(AuthHeader::as_str)
                .map(str::to_owned),
            ca_file: connection.ca_file().cloned(),
            insecure_skip_verify: false,
            request_timeout: bounds.request_timeout().get(),
            lifecycle_flush_timeout: bounds.lifecycle().flush().get(),
            lifecycle_shutdown_timeout: bounds.lifecycle().shutdown().get(),
            retry: RetrySettings::from_policy(policy),
            jitter_seed: seed_from_os(),
            #[cfg(test)]
            retry_delay_observer: None,
            #[cfg(test)]
            request_timeout_observer: None,
            #[cfg(test)]
            control_submission_observer: None,
            #[cfg(test)]
            startup_hooks: None,
        })
    }
}

enum DataCommand {
    Export {
        endpoint: String,
        body: Bytes,
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
    /// Serializes producer admission with a control command's first submission.
    ///
    /// The lock is released before waiting for either a control slot or the
    /// worker reply, so producers never inherit a lifecycle wait timeout.
    send_lock: Mutex<()>,
    /// Coordinates only control submitters when the capacity-one slot is full.
    control_submission_lock: Mutex<()>,
    cancel: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    terminated: AtomicBool,
    lifecycle_flush_timeout: Duration,
    lifecycle_shutdown_timeout: Duration,
    /// A submission may spend its full retry sequence in the worker after it
    /// has left the producer queue. Keep the caller alive through that budget
    /// plus the independently bounded lifecycle shutdown interval used as its
    /// queue/dispatch margin.
    export_result_timeout: Duration,
    #[cfg(test)]
    control_submission_observer: Option<Sender<()>>,
}

impl Drop for WorkerInner {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.stop.store(true, Ordering::Release);
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
    fn start(config: SyncHttpConfig) -> Result<Self, ExportError> {
        let (data_tx, data_rx) = mpsc::sync_channel(64);
        let (control_tx, control_rx) = mpsc::sync_channel(1);
        let (ready_tx, ready_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let flush_timeout = config.lifecycle_flush_timeout;
        let shutdown_timeout = config.lifecycle_shutdown_timeout;
        let export_result_timeout = config
            .retry
            .sequence_timeout
            .saturating_add(config.lifecycle_shutdown_timeout);
        let handshake_timeout = config
            .request_timeout
            .min(config.lifecycle_shutdown_timeout);
        #[cfg(test)]
        let control_submission_observer = config.control_submission_observer.clone();
        #[cfg(test)]
        let startup_hooks = config.startup_hooks.clone();
        thread::Builder::new()
            .name("sc-otlp-sync-http".to_owned())
            .spawn(move || {
                #[cfg(test)]
                let hooks = config.startup_hooks.clone();
                worker_main(
                    config,
                    data_rx,
                    control_rx,
                    ready_tx,
                    &worker_cancel,
                    &worker_stop,
                );
                #[cfg(test)]
                if let Some(hooks) = hooks {
                    // Sent only after worker_main returns and its client drops.
                    let _ = hooks.exited.send(worker_cancel.load(Ordering::Acquire));
                }
            })
            .map_err(|error| {
                transport_error_with_source("failed to start synchronous HTTP worker", error)
            })?;
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
                    control_submission_lock: Mutex::new(()),
                    cancel,
                    stop,
                    terminated: AtomicBool::new(false),
                    lifecycle_flush_timeout: flush_timeout,
                    lifecycle_shutdown_timeout: shutdown_timeout,
                    export_result_timeout,
                    #[cfg(test)]
                    control_submission_observer,
                }),
            }),
            Ok(Err(error)) => Err(error),
            Err(RecvTimeoutError::Timeout) => {
                cancel.store(true, Ordering::Release);
                Err(transport_error(
                    "synchronous HTTP worker construction handshake exceeded its finite deadline",
                ))
            }
            Err(RecvTimeoutError::Disconnected) => Err(worker_terminated_error()),
        }
    }

    fn enqueue(
        &self,
        endpoint: String,
        body: Bytes,
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
            .expect("synchronous HTTP worker send lock");
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

    fn export(&self, endpoint: String, body: Bytes) -> Result<(), ExportError> {
        let (result_tx, result_rx) = mpsc::channel();
        self.enqueue(
            endpoint,
            body,
            Box::new(move |result| {
                let _ = result_tx.send(result);
            }),
        )?;
        wait_for_control_result(&result_rx, self.inner.export_result_timeout)
    }

    fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::Release);
    }

    fn stop(&self) {
        self.inner.cancel.store(true, Ordering::Release);
        self.inner.stop.store(true, Ordering::Release);
    }

    fn submit_control(&self, command: ControlCommand) -> Result<(), ExportError> {
        let _control_guard = self
            .inner
            .control_submission_lock
            .lock()
            .expect("synchronous HTTP worker control lock");

        let pending_command = {
            let _send_guard = self
                .inner
                .send_lock
                .lock()
                .expect("synchronous HTTP worker control lock");
            match self.inner.control_tx.try_send(command) {
                Ok(()) => None,
                Err(TrySendError::Full(command)) => Some(command),
                Err(TrySendError::Disconnected(_)) => return Err(worker_terminated_error()),
            }
        };

        // The control channel is deliberately bounded to one pending command.
        // If an older command owns that slot, wait only under the control-only
        // mutex; producer admission keeps using `send_lock` independently.
        if let Some(command) = pending_command {
            self.inner
                .control_tx
                .send(command)
                .map_err(|_| worker_terminated_error())?;
        }
        #[cfg(test)]
        self.notify_control_submission();
        Ok(())
    }

    #[cfg(test)]
    fn notify_control_submission(&self) {
        if let Some(observer) = &self.inner.control_submission_observer {
            let _ = observer.send(());
        }
    }

    fn flush_blocking(&self) -> Result<(), ExportError> {
        let (tx, rx) = mpsc::channel();
        self.submit_control(ControlCommand::Flush { result: tx })?;
        wait_for_control_result(&rx, self.inner.lifecycle_flush_timeout)
    }

    fn shutdown_blocking(&self) -> Result<(), ExportError> {
        self.inner.cancel.store(true, Ordering::Release);
        let (tx, rx) = mpsc::channel();
        self.submit_control(ControlCommand::Shutdown { result: Some(tx) })?;
        wait_for_control_result(&rx, self.inner.lifecycle_shutdown_timeout)
    }
}

impl ExporterLifecycle for Worker {
    fn is_shutdown(&self) -> bool {
        self.inner.terminated.load(Ordering::Acquire) || self.inner.stop.load(Ordering::Acquire)
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn flush_async(&self) -> LifecycleFuture {
        let worker = self.clone();
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            thread::Builder::new()
                .name("sc-otlp-sync-http-flush".to_owned())
                .spawn(move || {
                    let _ = tx.send(worker.flush_blocking());
                })
                .map_err(|error| {
                    transport_error_with_source("failed to start sync HTTP flush helper", error)
                })?;
            rx.await.unwrap_or_else(|_| Err(worker_terminated_error()))
        })
    }

    fn shutdown_async(&self) -> LifecycleFuture {
        let worker = self.clone();
        Box::pin(async move {
            worker.cancel();
            let (tx, rx) = tokio::sync::oneshot::channel();
            thread::Builder::new()
                .name("sc-otlp-sync-http-shutdown".to_owned())
                .spawn(move || {
                    let _ = tx.send(worker.shutdown_blocking());
                })
                .map_err(|error| {
                    transport_error_with_source("failed to start sync HTTP shutdown helper", error)
                })?;
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

#[expect(
    clippy::needless_pass_by_value,
    reason = "the dedicated worker takes ownership of its channels and validated config"
)]
fn worker_main(
    config: SyncHttpConfig,
    data_rx: Receiver<DataCommand>,
    control_rx: Receiver<ControlCommand>,
    ready_tx: mpsc::Sender<Result<(), ExportError>>,
    cancel: &AtomicBool,
    stop: &AtomicBool,
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
        // Cancellation is the persistent stop intent used by final-handle
        // abandonment. Do not rely solely on the capacity-one control slot:
        // it may already contain a flush command.
        if stop.load(Ordering::Acquire) {
            return;
        }
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
    config: &SyncHttpConfig,
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

fn build_client(config: &SyncHttpConfig) -> Result<Client, ExportError> {
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
    config: &SyncHttpConfig,
    endpoint: &str,
    body: &Bytes,
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
        let request_timeout = selected_request_timeout(config.request_timeout, remaining);
        let request_timeout =
            observed_request_timeout!(config, attempt, request_timeout, remaining);
        let response = client
            .post(endpoint)
            .header(CONTENT_TYPE, "application/json")
            .body(body.clone())
            .timeout(request_timeout)
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
                let delay = retry_wait_delay(delay, config, started, cancel, &mut rng)?;
                if !wait_for_retry(config, delay, cancel) {
                    return Err(shutdown_cancelled_error());
                }
            }
            Err(error) => {
                if attempt >= config.retry.max_retries {
                    return Err(retry_attempts_exhausted_with_source(error));
                }
                let delay = retry_wait_delay(
                    fallback.min(config.retry.max_backoff),
                    config,
                    started,
                    cancel,
                    &mut rng,
                )?;
                if !wait_for_retry(config, delay, cancel) {
                    return Err(shutdown_cancelled_error());
                }
            }
        }
        attempt = attempt.saturating_add(1);
        fallback = fallback.saturating_mul(2).min(config.retry.max_backoff);
    }
}

fn retry_wait_delay(
    delay: Duration,
    config: &SyncHttpConfig,
    started: Instant,
    cancel: &AtomicBool,
    rng: &mut u64,
) -> Result<Duration, ExportError> {
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
    Ok(apply_jitter(delay, config.retry.jitter_percent, rng).min(remaining))
}

pub(super) fn selected_request_timeout(request_timeout: Duration, remaining: Duration) -> Duration {
    request_timeout.min(remaining)
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

fn wait_for_retry(config: &SyncHttpConfig, duration: Duration, cancel: &AtomicBool) -> bool {
    #[cfg(test)]
    let observer = config.retry_delay_observer.as_ref();
    #[cfg(not(test))]
    let _ = config;
    #[cfg(not(test))]
    let observer = None;

    wait_cancelable_with_observer(duration, cancel, observer)
}

pub(super) fn wait_cancelable_with_observer(
    duration: Duration,
    cancel: &AtomicBool,
    observer: Option<&Sender<Duration>>,
) -> bool {
    #[cfg(not(test))]
    let _ = observer;
    #[cfg(test)]
    {
        if let Some(sender) = observer {
            // std::sync::mpsc::Sender is unbounded, so send is nonblocking
            // for this test-only notification and cannot lose a pre-wait
            // event to a zero-capacity rendezvous race.
            let _ = sender.send(duration);
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

/// Seeds retry jitter from OS entropy, with a deterministic fallback if it is unavailable.
///
/// The fixed `0xa5a5_5a5a_1234_5678` value is an arbitrary sentinel, not
/// entropy, and has no algorithmic significance: the wrapping LCG adds one
/// each step, so any `u64` seed, including zero, is usable. If several workers
/// take this path, they start with the same seed and lose jitter decorrelation,
/// so their retries may align.
fn seed_from_os() -> u64 {
    let mut seed = [0_u8; 8];
    if getrandom::fill(&mut seed).is_ok() {
        return u64::from_le_bytes(seed);
    }
    0xa5a5_5a5a_1234_5678
}

/// Synchronous HTTP terminal backend shared by all three OTLP signal families.
///
/// `Worker` is the terminal transport and `LifecycleCore` is the only
/// admission/barrier owner. Signal adapters retain this pair through one
/// `Arc`, so the HTTP worker never grows a competing lifecycle state machine.
struct SyncHttpBackend {
    worker: Worker,
    lifecycle: LifecycleCore,
}

impl SyncHttpBackend {
    fn new(
        worker_config: SyncHttpConfig,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Arc<Self>, ExportError> {
        let worker = Worker::start(worker_config)?;
        let terminal: Arc<dyn ExporterLifecycle> = Arc::new(worker.clone());
        let lifecycle = LifecycleCore::from_backend(terminal, bounds)?;
        Ok(Arc::new(Self { worker, lifecycle }))
    }
}

impl Drop for SyncHttpBackend {
    fn drop(&mut self) {
        // The lifecycle core retains a Worker clone, so WorkerInner::drop is
        // not the final-handle boundary. Abandon shared admissions first,
        // then persist cancellation and best-effort wake the worker. The
        // worker loop also observes cancellation while idle, so a saturated
        // control slot cannot lose the stop intent.
        self.lifecycle.abandon();
        self.worker.stop();
        let _ = self
            .worker
            .inner
            .control_tx
            .try_send(ControlCommand::Shutdown { result: None });
    }
}

/// Synchronous HTTP exporter shared by all three OTLP signal families.
pub(crate) struct OtlpHttpExporter {
    backend: Arc<SyncHttpBackend>,
    endpoint: String,
}

/// Crate-private OTLP routes accepted by the synchronous submission path.
///
/// Keeping route derivation here prevents test-only helpers and durable
/// submission from drifting onto different endpoint construction paths.
#[derive(Debug, Clone, Copy)]
pub(super) enum SubmissionRoute {
    Signal(SignalKind),
    Profiles,
}

impl SubmissionRoute {
    fn endpoint(self, endpoint: &str) -> String {
        match self {
            Self::Signal(SignalKind::Logs) => normalize_signal_endpoint(endpoint, "logs"),
            Self::Signal(SignalKind::Traces) => normalize_signal_endpoint(endpoint, "traces"),
            Self::Signal(SignalKind::Metrics) => normalize_signal_endpoint(endpoint, "metrics"),
            Self::Signal(SignalKind::Profiles) | Self::Profiles => {
                format!("{}{PROFILES_EXPORT_PATH}", endpoint.trim_end_matches('/'))
            }
        }
    }
}

impl OtlpHttpExporter {
    pub(super) fn from_prepared(
        worker_config: SyncHttpConfig,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, ExportError> {
        let endpoint = worker_config.endpoint.clone();
        Ok(Self {
            backend: SyncHttpBackend::new(worker_config, bounds)?,
            endpoint,
        })
    }

    #[cfg(test)]
    pub(super) fn for_prepared_test(
        connection: &ValidatedBackendConnection,
        bounds: &ValidatedTransportBounds,
        observer: Sender<Duration>,
    ) -> Result<Self, ExportError> {
        let mut worker_config = SyncHttpConfig::from_prepared(connection, bounds)?;
        worker_config.retry_delay_observer = Some(observer);
        Self::from_prepared(worker_config, bounds)
    }

    #[cfg(test)]
    pub(super) fn for_endpoint(endpoint: String) -> Result<Self, ExportError> {
        Self::for_test_config(endpoint, None, None)
    }

    #[cfg(test)]
    pub(super) fn for_control_ordering_test(
        endpoint: String,
        control_submission_observer: Sender<()>,
    ) -> Result<Self, ExportError> {
        let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
        config.enabled = true;
        config.endpoint = Some(
            crate::config::OtlpEndpoint::new_typed(endpoint.clone())
                .expect("loopback test endpoint is valid"),
        );
        let (mut worker_config, bounds) = SyncHttpConfig::from_otel(&config)?;
        worker_config.control_submission_observer = Some(control_submission_observer);
        Ok(Self {
            backend: SyncHttpBackend::new(worker_config, &bounds)?,
            endpoint,
        })
    }

    /// Exercises real client construction and `Worker::start` with ordered gates.
    #[cfg(test)]
    pub(super) fn for_startup_test(hooks: Arc<StartupTestHooks>) -> Result<Self, ExportError> {
        let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
        config.enabled = true;
        config.endpoint = Some(
            crate::config::OtlpEndpoint::new_typed("http://127.0.0.1:1")
                .expect("test endpoint is valid"),
        );
        // A queued readiness message wins even at this short boundary. The
        // gates keep both cases independent of worker scheduling speed.
        config.timeout_ms = Some(1_u64.into());
        let (mut worker_config, bounds) = SyncHttpConfig::from_otel(&config)?;
        worker_config.startup_hooks = Some(hooks);
        let endpoint = worker_config.endpoint.clone();
        Ok(Self {
            backend: SyncHttpBackend::new(worker_config, &bounds)?,
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
        let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
        config.enabled = true;
        config.endpoint = Some(
            crate::config::OtlpEndpoint::new_typed(endpoint.clone())
                .expect("loopback test endpoint is valid"),
        );
        config.auth_header = auth_header.map(|header| {
            AuthHeader::new_typed(header).expect("test authorization header is valid")
        });
        config.ca_file = ca_file;
        let (worker_config, bounds) = SyncHttpConfig::from_otel(&config)?;
        Ok(Self {
            backend: SyncHttpBackend::new(worker_config, &bounds)?,
            endpoint,
        })
    }

    #[cfg(test)]
    pub(super) fn for_endpoint_with_retry(
        endpoint: String,
        retry: crate::config::SyncHttpRetryPolicy,
        jitter_seed: u64,
        retry_delay_observer: Option<Sender<Duration>>,
    ) -> Result<Self, ExportError> {
        let sequence_timeout_ms = retry.retry_sequence_timeout_ms.map_or(3_000, u64::from);
        let request_timeout_ms = sequence_timeout_ms.saturating_sub(1).clamp(1, 100);
        Self::for_endpoint_with_retry_timeout(
            endpoint,
            retry,
            request_timeout_ms,
            jitter_seed,
            retry_delay_observer,
        )
    }

    #[cfg(test)]
    pub(super) fn for_endpoint_with_retry_timeout(
        endpoint: String,
        retry: crate::config::SyncHttpRetryPolicy,
        request_timeout_ms: u64,
        jitter_seed: u64,
        retry_delay_observer: Option<Sender<Duration>>,
    ) -> Result<Self, ExportError> {
        Self::for_endpoint_with_retry_timeout_observing_request_timeout(
            endpoint,
            retry,
            request_timeout_ms,
            jitter_seed,
            retry_delay_observer,
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn for_endpoint_with_retry_timeout_observing_request_timeout(
        endpoint: String,
        retry: crate::config::SyncHttpRetryPolicy,
        request_timeout_ms: u64,
        jitter_seed: u64,
        retry_delay_observer: Option<Sender<Duration>>,
        request_timeout_observer: Option<Sender<(u32, Duration, Duration)>>,
    ) -> Result<Self, ExportError> {
        let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
        config.enabled = true;
        config.timeout_ms = Some(request_timeout_ms.into());
        config.endpoint = Some(
            crate::config::OtlpEndpoint::new_typed(endpoint.clone())
                .expect("loopback test endpoint is valid"),
        );
        config.sync_http_retry = Some(retry);
        let (mut worker_config, bounds) = SyncHttpConfig::from_otel(&config)?;
        worker_config.jitter_seed = jitter_seed;
        worker_config.retry_delay_observer = retry_delay_observer;
        worker_config.request_timeout_observer = request_timeout_observer;
        Ok(Self {
            backend: SyncHttpBackend::new(worker_config, &bounds)?,
            endpoint,
        })
    }

    fn send_payload(
        &self,
        signal: Signal,
        endpoint_signal: &str,
        batch: impl Send + 'static,
        payload: &Value,
    ) -> Result<(), ExportError> {
        let body = Bytes::from(payload.to_string());
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

    pub(super) fn submit_json_blocking(
        &self,
        route: SubmissionRoute,
        payload: &Value,
    ) -> Result<(), ExportError> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(blocking_in_async_error());
        }
        let endpoint = route.endpoint(&self.endpoint);
        self.backend
            .worker
            .export(endpoint, Bytes::from(payload.to_string()))
    }

    pub(super) fn cancel_submission(&self) {
        self.backend.worker.cancel();
    }

    #[cfg(test)]
    pub(super) fn lifecycle_for_test(&self) -> LifecycleCore {
        self.backend.lifecycle.clone()
    }

    #[cfg(test)]
    pub(super) fn flush_worker_for_test(
        &self,
    ) -> (Receiver<Result<(), ExportError>>, thread::JoinHandle<()>) {
        let worker = self.backend.worker.clone();
        let (result_tx, result_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("sc-otlp-test-flush".to_owned())
            .spawn(move || {
                let _ = result_tx.send(worker.flush_blocking());
            })
            .expect("spawn control ordering test helper");
        (result_rx, thread)
    }
}

impl LogExporter<LogEvent> for OtlpHttpExporter {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError> {
        let records = batch.iter().map(log_record).collect::<Vec<_>>();
        self.send_payload(
            Signal::Logs,
            "logs",
            batch.to_vec(),
            &build_logs_payload(&records),
        )
    }
}

impl LogExporter<ExportRecord<LogRecord>> for OtlpHttpExporter {
    fn export_logs(&self, batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        self.send_payload(
            Signal::Logs,
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
            Signal::Traces,
            "traces",
            batch.to_vec(),
            &build_traces_payload(&records),
        )
    }
}

impl TraceExporter<ExportRecord<CompleteSpan>> for OtlpHttpExporter {
    fn export_spans(&self, batch: &[ExportRecord<CompleteSpan>]) -> Result<(), ExportError> {
        self.send_payload(
            Signal::Traces,
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
            Signal::Metrics,
            "metrics",
            batch.to_vec(),
            &build_metrics_payload(&records),
        )
    }
}

impl MetricExporter<ExportRecord<MetricRecord>> for OtlpHttpExporter {
    fn export_metrics(&self, batch: &[ExportRecord<MetricRecord>]) -> Result<(), ExportError> {
        self.send_payload(
            Signal::Metrics,
            "metrics",
            batch.to_vec(),
            &build_metrics_payload(batch),
        )
    }
}

impl ExporterLifecycle for OtlpHttpExporter {
    fn is_shutdown(&self) -> bool {
        self.backend.lifecycle.health().phase != LifecycleState::Open
    }

    fn lifecycle_health(&self) -> Option<crate::lifecycle::LifecycleHealth> {
        Some(self.backend.lifecycle.health())
    }

    fn blocking_preflight(&self) -> Result<(), ExportError> {
        Ok(())
    }

    fn blocking_lifecycle_preflight(&self) -> Result<(), ExportError> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(blocking_in_async_error());
        }
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
pub(crate) fn build_exporter_set(
    connection: &ValidatedBackendConnection,
    bounds: &ValidatedTransportBounds,
) -> Result<ExporterSet, ExportError> {
    let worker_config = SyncHttpConfig::from_prepared(connection, bounds)?;
    let exporter = Arc::new(OtlpHttpExporter::from_prepared(worker_config, bounds)?);
    Ok(ExporterSet {
        logs: exporter.clone(),
        traces: exporter.clone(),
        metrics: exporter.clone(),
        lifecycle: exporter,
    })
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
                "synchronous HTTP admission returned an unknown telemetry error",
                Remediation::not_recoverable(
                    "inspect the synchronous HTTP admission failure before submitting a new batch",
                ),
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

fn error_with_code(
    code: sc_observability_types::ErrorCode,
    message: &str,
    remediation: Remediation,
) -> ErrorContext {
    ErrorContext::new(code, message, remediation)
}

pub(super) fn worker_terminated_error() -> ExportError {
    ExportError::WorkerTerminated {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_WORKER_TERMINATED,
            "synchronous HTTP worker terminated",
            Remediation::recoverable(
                "restart the synchronous HTTP exporter",
                ["resubmit any batch that was not acknowledged"],
            ),
        )),
    }
}

pub(super) fn wait_for_control_result(
    receiver: &Receiver<Result<(), ExportError>>,
    timeout: Duration,
) -> Result<(), ExportError> {
    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(ExportError::LifecycleTimeout {
            context: Box::new(error_with_code(
                error_codes::otlp::OTLP_LIFECYCLE_TIMEOUT,
                "synchronous HTTP lifecycle control exceeded its finite deadline",
                Remediation::recoverable(
                    "inspect exporter health and keep the runtime alive through lifecycle completion",
                    [] as [&str; 0],
                ),
            )),
        }),
        Err(RecvTimeoutError::Disconnected) => Err(worker_terminated_error()),
    }
}

pub(super) fn queue_full_error() -> ExportError {
    ExportError::QueueFull {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_QUEUE_FULL,
            "synchronous HTTP worker admission is full",
            Remediation::recoverable(
                "wait for synchronous HTTP worker capacity",
                ["retry the export after capacity is available"],
            ),
        )),
    }
}

pub(super) fn shutdown_cancelled_error() -> ExportError {
    ExportError::ShutdownCancelledRetry {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_SHUTDOWN_CANCELLED_RETRY,
            "synchronous HTTP retry was cancelled by shutdown",
            Remediation::not_recoverable(
                "the synchronous HTTP exporter is shutting down and cannot retry this batch",
            ),
        )),
    }
}

pub(super) fn retry_deadline_error() -> ExportError {
    ExportError::RetryDeadlineExhausted {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_RETRY_DEADLINE_EXHAUSTED,
            "synchronous HTTP retry sequence exceeded its deadline",
            Remediation::recoverable(
                "restore collector availability before retrying the export",
                ["increase the retry sequence deadline only when the delivery budget permits"],
            ),
        )),
    }
}

fn retry_attempts_exhausted_error(status: u16) -> ExportError {
    ExportError::RetryAttemptsExhausted {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED,
            &format!(
                "synchronous HTTP collector returned retryable HTTP status {status} until attempts were exhausted"
            ),
            Remediation::recoverable(
                "wait for the collector to recover before retrying the export",
                [] as [&str; 0],
            ),
        )),
    }
}

fn retry_attempts_exhausted_with_source(error: reqwest::Error) -> ExportError {
    ExportError::RetryAttemptsExhausted {
        context: Box::new(
            error_with_code(
                error_codes::otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED,
                "synchronous HTTP transport retries were exhausted",
                Remediation::recoverable(
                    "restore collector connectivity before retrying the export",
                    [] as [&str; 0],
                ),
            )
            .source(Box::new(error)),
        ),
    }
}

pub(super) fn non_retryable_status_error(status: u16) -> ExportError {
    ExportError::NonRetryableHttpStatus {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_HTTP_STATUS_TERMINAL,
            &format!("synchronous HTTP collector returned terminal HTTP status {status}"),
            Remediation::not_recoverable(
                "correct the collector request, credentials, or endpoint before submitting a new batch",
            ),
        )),
    }
}

fn blocking_in_async_error() -> ExportError {
    ExportError::BlockingBackendInAsyncContext {
        context: Box::new(error_with_code(
            error_codes::otlp::OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT,
            "blocking synchronous HTTP lifecycle cannot run from an entered Tokio runtime",
            Remediation::not_recoverable(
                "use the asynchronous SDK lifecycle API from an entered Tokio runtime",
            ),
        )),
    }
}
