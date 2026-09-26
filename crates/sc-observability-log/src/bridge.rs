//! The process-global `log` facade and the non-owning host attachment bridge.
#![allow(
    deprecated,
    reason = "D2 binds the retained bridge compatibility methods until the D18 migration"
)]

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError, RwLock, Weak};
use std::time::{Duration, Instant};

use crate::control::BridgeEvent;
use crate::handle;
use crate::{DropCause, EmitError, InitError, LogControl, mapping};
use sc_observability_types::{
    AdmissionOutcome, ErrorCode, ErrorContext, LogEvent, OperationDiagnostic, ProcessIdentity,
    Remediation, ServiceName, Timestamp,
};

const MODE_EMPTY: u8 = 0;
const MODE_OWNED: u8 = 1;
const MODE_ATTACHED: u8 = 2;
const MODE_CLOSING: u8 = 3;
const MODE_STOPPED: u8 = 4;
const MODE_DETACHED: u8 = 5;

static BRIDGE_MODE: AtomicU8 = AtomicU8::new(MODE_EMPTY);
static ATTACHMENT_SLOT: RwLock<Option<Arc<AttachmentState>>> = RwLock::new(None);

/// Open host policy evaluated after bridge event assembly and before logger admission.
pub trait BridgeEventPolicy: Send + Sync {
    /// Decide whether the assembled event may reach the host logger.
    fn decide(&self, event: &LogEvent) -> BridgeEventDecision;
}

/// Policy admission result.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeEventDecision {
    /// Admit the event to the host logger.
    Admit,
    /// Reject the event without invoking the host logger or sink.
    Reject(PolicyRejection),
}

/// Typed policy rejection reason.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyRejection {
    /// The host policy denied the event.
    Denied,
    /// The assembled payload exceeded the host bound.
    PayloadTooLarge,
    /// The assembled event was otherwise invalid for the host policy.
    Invalid,
}

/// Options for a non-owning host attachment.
#[non_exhaustive]
pub struct AttachmentOptions {
    /// Facade mapping options; this shape remains identical to owned init.
    pub bridge: crate::BridgeOptions,
    /// Host admission policy. Redaction remains owned by the host logger.
    pub policy: Arc<dyn BridgeEventPolicy>,
}

impl AttachmentOptions {
    /// Creates attachment options with an open host policy.
    #[must_use]
    pub fn new(bridge: crate::BridgeOptions, policy: Arc<dyn BridgeEventPolicy>) -> Self {
        Self { bridge, policy }
    }
}

impl std::fmt::Debug for AttachmentOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AttachmentOptions")
            .field("bridge", &self.bridge)
            .field("policy", &"<dyn BridgeEventPolicy>")
            .finish()
    }
}

/// Canonical attachment failure surface from D13.
#[non_exhaustive]
#[derive(Debug, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum DetachError {
    /// In-flight bridge calls did not drain before the bounded deadline.
    #[error("logger attachment detach timed out: {context}")]
    Timeout {
        /// Structured timeout diagnostic and remediation.
        context: Box<ErrorContext>,
    },
    /// The attachment or its saved control is no longer installed.
    #[error("logger attachment is not installed: {context}")]
    NotInstalled {
        /// Structured stale-control diagnostic and remediation.
        context: Box<ErrorContext>,
    },
    /// A foreign logger owns the process-global facade.
    #[error("a foreign logger owns the log facade: {context}")]
    ForeignLoggerInstalled {
        /// Structured foreign-facade diagnostic and remediation.
        context: Box<ErrorContext>,
    },
}

impl DetachError {
    fn timeout() -> Self {
        Self::Timeout {
            context: Box::new(context(
                crate::error_codes::SC_LOG_DETACH_TIMEOUT,
                "logger attachment calls did not drain before the timeout",
                "retry detach after in-flight bridge calls complete",
            )),
        }
    }

    fn not_installed() -> Self {
        Self::NotInstalled {
            context: Box::new(context(
                crate::error_codes::SC_LOG_DETACH_NOT_INSTALLED,
                "logger attachment is no longer installed",
                "discard stale controls and attach the current host logger",
            )),
        }
    }
}

impl DetachError {
    /// Returns the stable registry code for this failure.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Timeout { context }
            | Self::NotInstalled { context }
            | Self::ForeignLoggerInstalled { context } => context.diagnostic().code.clone(),
        }
    }

    /// Returns the structured diagnostic carried by this failure.
    #[must_use]
    pub fn diagnostic(&self) -> &ErrorContext {
        match self {
            Self::Timeout { context }
            | Self::NotInstalled { context }
            | Self::ForeignLoggerInstalled { context } => context,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachmentPhase {
    Attached,
    Closing,
    Detached,
}

#[derive(Debug)]
struct AttachmentGate {
    phase: AttachmentPhase,
    in_flight: usize,
}

/// Shared state retained by an attachment, its controls, and in-flight calls.
pub(crate) struct AttachmentState {
    pub(crate) logger: Arc<sc_observability::Logger>,
    pub(crate) options: crate::BridgeOptions,
    policy: Arc<dyn BridgeEventPolicy>,
    service: ServiceName,
    identity: ProcessIdentity,
    gate: Mutex<AttachmentGate>,
    drained: Condvar,
}

impl std::fmt::Debug for AttachmentState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AttachmentState")
            .field("logger", &"<Arc<Logger>>")
            .field("options", &self.options)
            .field("policy", &"<dyn BridgeEventPolicy>")
            .finish_non_exhaustive()
    }
}

struct AttachmentCall {
    state: Arc<AttachmentState>,
}

impl Drop for AttachmentCall {
    fn drop(&mut self) {
        let mut gate = self
            .state
            .gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        gate.in_flight = gate.in_flight.saturating_sub(1);
        if gate.in_flight == 0 {
            self.state.drained.notify_all();
        }
    }
}

impl AttachmentState {
    fn enter(self: &Arc<Self>) -> Option<AttachmentCall> {
        let mut gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if gate.phase != AttachmentPhase::Attached {
            return None;
        }
        gate.in_flight += 1;
        Some(AttachmentCall {
            state: Arc::clone(self),
        })
    }
}

/// Non-owning attachment handle. It never owns shutdown or level authority.
#[must_use = "dropping an attachment detaches it on a bounded best effort"]
pub struct LogAttachment {
    state: Option<Arc<AttachmentState>>,
}

impl std::fmt::Debug for LogAttachment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LogAttachment")
            .field("installed", &self.state.is_some())
            .finish()
    }
}

impl LogAttachment {
    /// Returns a cloneable, non-owning control for this attachment.
    #[must_use]
    pub fn control(&self) -> LogControl {
        self.state
            .as_ref()
            .map_or_else(LogControl::stale_attachment, LogControl::for_attachment)
    }

    /// Closes admission, drains entered calls, and releases attachment references.
    ///
    /// # Errors
    ///
    /// Returns [`DetachError::Timeout`] when in-flight calls do not drain before
    /// `timeout`; the attachment remains installed for retry. Returns
    /// [`DetachError::NotInstalled`] for a stale or already-detached handle.
    pub fn detach(&mut self, timeout: Duration) -> Result<(), DetachError> {
        let Some(state) = self.state.as_ref().map(Arc::clone) else {
            return Err(DetachError::not_installed());
        };
        close_attachment(&state, timeout, false)?;
        self.state.take();
        Ok(())
    }
}

impl Drop for LogAttachment {
    fn drop(&mut self) {
        let Some(state) = self.state.take() else {
            return;
        };
        let _ = close_attachment(&state, Duration::from_secs(2), true);
    }
}

/// Attaches an existing host logger without creating a writer or level owner.
///
/// # Errors
///
/// Returns [`InitError::AlreadyInitialized`] when owned initialization or
/// another attachment already occupies the bridge, and
/// [`InitError::ForeignLoggerInstalled`] when another `log::Log` owns the
/// process facade.
pub fn attach_logger(
    logger: Arc<sc_observability::Logger>,
    options: AttachmentOptions,
) -> Result<LogAttachment, InitError> {
    // The attachment is a facade installation, so the existing init errors
    // deliberately remain the public failure surface.
    let mode = BRIDGE_MODE.load(Ordering::SeqCst);
    if !matches!(mode, MODE_EMPTY | MODE_DETACHED) {
        return Err(InitError::AlreadyInitialized);
    }

    let first_facade = if mode == MODE_EMPTY {
        handle::INSTALLED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    } else {
        false
    };
    if mode == MODE_EMPTY && !first_facade {
        return Err(InitError::AlreadyInitialized);
    }
    if first_facade {
        if log::set_boxed_logger(Box::new(Bridge)).is_err() {
            BRIDGE_MODE.store(MODE_STOPPED, Ordering::SeqCst);
            return Err(InitError::ForeignLoggerInstalled);
        }
        log::set_max_level(log::LevelFilter::Trace);
    }

    let state = Arc::new(AttachmentState {
        service: infer_service(&logger),
        identity: ProcessIdentity {
            hostname: None,
            pid: Some(std::process::id()),
        },
        logger,
        options: options.bridge,
        policy: options.policy,
        gate: Mutex::new(AttachmentGate {
            phase: AttachmentPhase::Attached,
            in_flight: 0,
        }),
        drained: Condvar::new(),
    });
    let mut slot = ATTACHMENT_SLOT
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    if slot.is_some() {
        return Err(InitError::AlreadyInitialized);
    }
    *slot = Some(Arc::clone(&state));
    BRIDGE_MODE.store(MODE_ATTACHED, Ordering::SeqCst);
    Ok(LogAttachment { state: Some(state) })
}

fn close_attachment(
    state: &Arc<AttachmentState>,
    timeout: Duration,
    dropping: bool,
) -> Result<(), DetachError> {
    {
        let mut slot = ATTACHMENT_SLOT
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(current) = slot.as_ref() else {
            return Err(DetachError::not_installed());
        };
        if !Arc::ptr_eq(current, state) {
            return Err(DetachError::not_installed());
        }
        let mut gate = state.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if gate.phase != AttachmentPhase::Attached {
            return Err(DetachError::not_installed());
        }
        gate.phase = AttachmentPhase::Closing;
        slot.take();
        BRIDGE_MODE.store(MODE_CLOSING, Ordering::SeqCst);
    }

    let deadline = Instant::now() + timeout;
    let mut gate = state.gate.lock().unwrap_or_else(PoisonError::into_inner);
    while gate.in_flight != 0 {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let (next, result) = state
            .drained
            .wait_timeout(gate, remaining)
            .unwrap_or_else(PoisonError::into_inner);
        gate = next;
        if result.timed_out() {
            break;
        }
    }
    if gate.in_flight != 0 {
        if !dropping {
            gate.phase = AttachmentPhase::Attached;
            drop(gate);
            *ATTACHMENT_SLOT
                .write()
                .unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(state));
            BRIDGE_MODE.store(MODE_ATTACHED, Ordering::SeqCst);
        }
        return Err(DetachError::timeout());
    }
    gate.phase = AttachmentPhase::Detached;
    drop(gate);
    BRIDGE_MODE.store(MODE_DETACHED, Ordering::SeqCst);
    Ok(())
}

fn infer_service(logger: &sc_observability::Logger) -> ServiceName {
    let path = logger.health().active_log_path;
    let candidate = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".log.jsonl"))
        .unwrap_or("attached");
    ServiceName::new(candidate)
        .unwrap_or_else(|_| ServiceName::new("attached").expect("literal service"))
}

fn context(code: ErrorCode, message: &str, remediation: &str) -> ErrorContext {
    ErrorContext::new(
        code,
        message,
        Remediation::recoverable(remediation, std::iter::empty::<String>()),
    )
}

fn policy_diagnostic(reason: PolicyRejection) -> OperationDiagnostic {
    OperationDiagnostic {
        code: crate::error_codes::SC_OBSERVABILITY_LOG_INVALID_FIELD,
        message: format!("host bridge policy rejected event: {reason:?}"),
        remediation: Remediation::recoverable(
            "adjust the host bridge policy or event payload",
            std::iter::empty::<String>(),
        ),
        at: Timestamp::now_utc(),
    }
}

fn policy_allows(state: &AttachmentState, event: &LogEvent) -> Result<(), DropCause> {
    match state.policy.decide(event) {
        BridgeEventDecision::Admit => Ok(()),
        BridgeEventDecision::Reject(_) => Err(DropCause::InvalidEvent),
    }
}

pub(crate) fn attached_enabled(level: sc_observability_types::Level) -> Option<bool> {
    let state = ATTACHMENT_SLOT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()?;
    let effective = state.logger.level_state().effective_level;
    Some(
        effective != sc_observability_types::LevelFilter::Off
            && level_rank(level) >= filter_rank(effective),
    )
}

pub(crate) fn attached_options() -> Option<crate::BridgeOptions> {
    ATTACHMENT_SLOT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|state| state.options.clone())
}

pub(crate) fn is_attached() -> bool {
    ATTACHMENT_SLOT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
}

pub(crate) fn submit_parts_if_attached(
    parts: crate::__private::EventParts,
) -> Result<Result<AdmissionOutcome, DropCause>, Box<crate::__private::EventParts>> {
    let Some(state) = ATTACHMENT_SLOT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    else {
        return Err(Box::new(parts));
    };
    let Some(_call) = state.enter() else {
        return Ok(Err(DropCause::NotInstalled));
    };
    let mut event = mapping::assemble_event(
        parts,
        &state.service,
        &state.identity,
        &state.options.default_action,
    );
    event.trace = crate::context::current_trace();
    if let Err(cause) = policy_allows(&state, &event) {
        return Ok(Err(cause));
    }
    Ok(match state.logger.try_log_with_outcome(event) {
        Ok(outcome) => Ok(outcome),
        Err(error) => Err(match error {
            sc_observability::TryLogError::QueueFull(_) => DropCause::QueueFull,
            sc_observability::TryLogError::InvalidEvent(_) => DropCause::InvalidEvent,
            sc_observability::TryLogError::WriterDegraded(_) => DropCause::WriterDegraded,
            sc_observability::TryLogError::ShutdownTimedOut(_) => DropCause::ShutdownTimedOut,
        }),
    })
}

pub(crate) fn submit_control(
    saved: &Weak<AttachmentState>,
    event: BridgeEvent,
) -> Result<AdmissionOutcome, EmitError> {
    let state = saved.upgrade().ok_or(EmitError::NotRunning {
        phase: crate::LifecyclePhase::Stopped,
    })?;
    let Some(_call) = state.enter() else {
        return Err(EmitError::NotRunning {
            phase: crate::LifecyclePhase::Stopping,
        });
    };
    let mut fields = serde_json::Map::new();
    let mut raw_keys = std::collections::BTreeMap::new();
    for (raw, value) in event.fields {
        let key = mapping::field_key_label(&raw)
            .map_err(|error| EmitError::InvalidField {
                raw_key: raw.clone(),
                reason: match error {
                    mapping::LabelError::Empty { .. } | mapping::LabelError::Rejected { .. } => {
                        crate::FieldKeyError::Empty
                    }
                    mapping::LabelError::ReservedPrefix { .. } => {
                        crate::FieldKeyError::ReservedPrefix
                    }
                },
            })?
            .into_owned();
        if let Some(other_raw_key) = raw_keys.insert(key.clone(), raw.clone()) {
            return Err(EmitError::InvalidField {
                raw_key: raw,
                reason: crate::FieldKeyError::Collision { other_raw_key },
            });
        }
        fields.insert(key, value);
    }
    let observation = sc_observability_types::Observation::new(state.service.clone(), ());
    let assembled = LogEvent {
        version: observation.version,
        timestamp: observation.timestamp,
        level: event.level,
        service: state.service.clone(),
        target: event.target,
        action: event
            .action
            .unwrap_or_else(|| state.options.default_action.clone()),
        message: event.message,
        identity: state.identity.clone(),
        trace: event.trace.or_else(crate::context::current_trace),
        request_id: event.request_id,
        correlation_id: event.correlation_id,
        outcome: event.outcome,
        diagnostic: None,
        state_transition: None,
        fields,
    };
    if let BridgeEventDecision::Reject(reason) = state.policy.decide(&assembled) {
        return Err(EmitError::InvalidEvent {
            diagnostic: policy_diagnostic(reason),
        });
    }
    state
        .logger
        .try_log_with_outcome(assembled)
        .map_err(|error| crate::control::core_emit_error(&error))
}

pub(crate) fn submit_current_control(event: BridgeEvent) -> Result<AdmissionOutcome, EmitError> {
    let state = ATTACHMENT_SLOT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or(EmitError::NotRunning {
            phase: crate::LifecyclePhase::Stopped,
        })?;
    submit_control(&Arc::downgrade(&state), event)
}

pub(crate) fn flush_attached(
    saved: &Weak<AttachmentState>,
    timeout: Duration,
) -> Result<(), crate::FlushError> {
    let state = saved.upgrade().ok_or(crate::FlushError::NotRunning {
        phase: crate::LifecyclePhase::Stopped,
    })?;
    let Some(call) = state.enter() else {
        return Err(crate::FlushError::NotRunning {
            phase: crate::LifecyclePhase::Stopping,
        });
    };
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("sc-observability-log-attachment-flush".to_owned())
        .spawn(move || {
            // Keep the attachment call alive until the helper exits.  A timed-out
            // caller must not be able to detach while this helper still owns the
            // attachment's logger reference.
            let result = call.state.logger.flush();
            let _ = sender.send(result);
            drop(call);
        })
        .map_err(|source| crate::FlushError::HelperSpawn {
            diagnostic: OperationDiagnostic {
                code: crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                message: source.to_string(),
                remediation: Remediation::recoverable(
                    "retry the bounded attachment flush",
                    std::iter::empty::<String>(),
                ),
                at: Timestamp::now_utc(),
            },
        })?;
    match receiver.recv_timeout(timeout) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(crate::FlushError::Logger {
            diagnostic: crate::error::diagnostic_from_info(&source),
        }),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(crate::FlushError::TimedOut { timeout })
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(crate::FlushError::HelperLost {
                diagnostic: OperationDiagnostic {
                    code: crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                    message: "attachment flush helper ended without a result".to_owned(),
                    remediation: Remediation::not_recoverable(
                        "inspect host logger health before retrying",
                    ),
                    at: Timestamp::now_utc(),
                },
            })
        }
    }
}

pub(crate) fn flush_current_attachment(timeout: Duration) -> Result<(), crate::FlushError> {
    let state = ATTACHMENT_SLOT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or(crate::FlushError::NotRunning {
            phase: crate::LifecyclePhase::Stopped,
        })?;
    flush_attached(&Arc::downgrade(&state), timeout)
}

pub(crate) fn mark_owned_running() {
    BRIDGE_MODE.store(MODE_OWNED, Ordering::SeqCst);
}

pub(crate) fn mark_owned_stopped() {
    if BRIDGE_MODE.load(Ordering::SeqCst) == MODE_OWNED {
        BRIDGE_MODE.store(MODE_STOPPED, Ordering::SeqCst);
    }
}

fn level_rank(level: sc_observability_types::Level) -> u8 {
    match level {
        sc_observability_types::Level::Trace => 0,
        sc_observability_types::Level::Debug => 1,
        sc_observability_types::Level::Info => 2,
        sc_observability_types::Level::Warn => 3,
        sc_observability_types::Level::Error => 4,
    }
}

fn filter_rank(level: sc_observability_types::LevelFilter) -> u8 {
    match level {
        sc_observability_types::LevelFilter::Trace => 0,
        sc_observability_types::LevelFilter::Debug => 1,
        sc_observability_types::LevelFilter::Info => 2,
        sc_observability_types::LevelFilter::Warn => 3,
        sc_observability_types::LevelFilter::Error => 4,
        sc_observability_types::LevelFilter::Off => 5,
    }
}

/// Maps every enabled `log` record through the shared guarded submission core.
#[derive(Debug)]
pub(crate) struct Bridge;

impl log::Log for Bridge {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        handle::core_enabled(mapping::map_level(metadata.level()))
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let _ = handle::submit_guarded(|| {
            let options = attached_options()
                .or_else(|| handle::current_installed().map(|installed| installed.options.clone()))
                .ok_or(DropCause::NotInstalled)?;
            let mapped = mapping::record_to_parts(record, &options)
                .map_err(|_label_error| DropCause::InvalidEvent)?;
            for _ in 0..mapped.omitted_fields {
                handle::record_drop(DropCause::InvalidEvent);
            }
            handle::submit_installed(mapped.parts)
        });
    }

    fn flush(&self) {}
}
