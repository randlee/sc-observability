//! The process-global `log` facade and the non-owning host attachment bridge.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

#[cfg(test)]
use std::cell::Cell;

use crate::control::{BridgeEvent, LogControl};
use crate::error::EmitError;
use crate::handle;
use crate::{DropCause, mapping};
use sc_observability_types::v2::FlushError as CoreFlushError;
use sc_observability_types::{
    AdmissionOutcome, ErrorCode, ErrorContext, FailureClassification, LogEvent,
    OperationDiagnostic, ProcessIdentity, Remediation, ServiceName, Timestamp,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttachmentMode {
    Empty,
    Owned,
    Attached,
    Closing,
    Stopped,
    Detached,
}

// One authority for attachment lifecycle, slot membership, and entered calls.
// Never acquire this lock while holding a logger lock or invoking host callbacks.
// INSTALLED only records permanent facade installation; it is never drain state.
struct AttachmentRegistry {
    mode: AttachmentMode,
    state: Option<Arc<AttachmentState>>,
    in_flight: usize,
    abandoned: bool,
}

static ATTACHMENT: Mutex<AttachmentRegistry> = Mutex::new(AttachmentRegistry {
    mode: AttachmentMode::Empty,
    state: None,
    in_flight: 0,
    abandoned: false,
});
static DRAINED: Condvar = Condvar::new();

#[cfg(test)]
thread_local! {
    static ATTACHMENT_ENTRIES: Cell<usize> = const { Cell::new(0) };
}

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

impl PolicyRejection {
    fn message(self) -> &'static str {
        match self {
            Self::Denied => "host bridge policy denied the event",
            Self::PayloadTooLarge => "host bridge policy rejected an oversized payload",
            Self::Invalid => "host bridge policy rejected an invalid event",
        }
    }

    fn remediation(self) -> &'static str {
        match self {
            Self::Denied => "adjust the host allowlist or route the event to an admitted target",
            Self::PayloadTooLarge => "reduce the event payload before submitting it",
            Self::Invalid => "correct the event fields and resubmit the event",
        }
    }
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

    fn foreign_logger_installed() -> Self {
        Self::ForeignLoggerInstalled {
            context: Box::new(context(
                crate::error_codes::SC_LOG_FOREIGN_LOGGER_INSTALLED,
                "another logger attachment or owner occupies the log facade",
                "use the logger owner or detach the active attachment before attaching",
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

/// Shared state retained by an attachment, its controls, and in-flight calls.
pub(crate) struct AttachmentState {
    logger: Arc<sc_observability::v2::Logger>,
    options: crate::BridgeOptions,
    policy: Arc<dyn BridgeEventPolicy>,
    service: ServiceName,
    identity: ProcessIdentity,
    // MUTEX: concurrent admissions and inspection share the latest rejection; both accesses recover poison.
    last_policy_rejection: Mutex<Option<OperationDiagnostic>>,
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

// Rust drops fields in declaration order: release the call's logger ownership
// before decrementing the drain count, including on callback panic and helpers.
struct AttachmentCall {
    state: Arc<AttachmentState>,
    _completion: AttachmentCompletion,
}

struct AttachmentCompletion;

impl Drop for AttachmentCompletion {
    fn drop(&mut self) {
        let mut registry = ATTACHMENT.lock().unwrap_or_else(PoisonError::into_inner);
        registry.in_flight -= 1;
        let retired = if registry.in_flight == 0 {
            let retired = if registry.mode == AttachmentMode::Closing && registry.abandoned {
                registry.mode = AttachmentMode::Detached;
                registry.state.take()
            } else {
                None
            };
            DRAINED.notify_all();
            retired
        } else {
            None
        };
        drop(registry);
        // A policy or logger destructor may invoke host code; never run it
        // under the lifecycle mutex.
        drop(retired);
    }
}

fn enter_attachment(saved: Option<&Weak<AttachmentState>>) -> Option<AttachmentCall> {
    #[cfg(test)]
    ATTACHMENT_ENTRIES.with(|entries| entries.set(entries.get() + 1));
    let mut registry = ATTACHMENT.lock().unwrap_or_else(PoisonError::into_inner);
    if registry.mode != AttachmentMode::Attached {
        return None;
    }
    let state = registry.state.as_ref()?;
    if saved.is_some_and(|saved| !Weak::ptr_eq(saved, &Arc::downgrade(state))) {
        return None;
    }
    let next = registry.in_flight.checked_add(1)?;
    let state = Arc::clone(state);
    registry.in_flight = next;
    Some(AttachmentCall {
        state,
        _completion: AttachmentCompletion,
    })
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

    /// Returns the latest policy rejection from direct, facade, or macro admission.
    #[must_use]
    pub fn last_policy_rejection(&self) -> Option<OperationDiagnostic> {
        self.state.as_ref().and_then(|state| {
            state
                .last_policy_rejection
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        })
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
        let _ = close_attachment(&state, crate::DEFAULT_DROP_SHUTDOWN_TIMEOUT, true);
    }
}

/// Attaches an existing host logger without creating a writer or level owner.
///
/// # Errors
///
/// Returns [`DetachError::ForeignLoggerInstalled`] when the process facade is
/// occupied by an owned bridge, another attachment, or a foreign `log::Log`.
pub fn attach_logger(
    logger: Arc<sc_observability::v2::Logger>,
    options: AttachmentOptions,
) -> Result<LogAttachment, DetachError> {
    let mut registry = ATTACHMENT.lock().unwrap_or_else(PoisonError::into_inner);
    let mode = registry.mode;
    if !matches!(mode, AttachmentMode::Empty | AttachmentMode::Detached) {
        return Err(DetachError::foreign_logger_installed());
    }

    let first_facade = if mode == AttachmentMode::Empty {
        handle::INSTALLED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    } else {
        false
    };
    if mode == AttachmentMode::Empty && !first_facade {
        return Err(DetachError::foreign_logger_installed());
    }
    if first_facade {
        if log::set_boxed_logger(Box::new(Bridge)).is_err() {
            registry.mode = AttachmentMode::Stopped;
            return Err(DetachError::foreign_logger_installed());
        }
        log::set_max_level(log::LevelFilter::Trace);
    }

    let state = Arc::new(AttachmentState {
        service: logger.service_name().clone(),
        identity: ProcessIdentity {
            hostname: None,
            pid: Some(std::process::id()),
        },
        logger,
        options: options.bridge,
        policy: options.policy,
        last_policy_rejection: Mutex::new(None),
    });
    registry.state = Some(Arc::clone(&state));
    registry.mode = AttachmentMode::Attached;
    registry.abandoned = false;
    Ok(LogAttachment { state: Some(state) })
}

fn close_attachment(
    state: &Arc<AttachmentState>,
    timeout: Duration,
    dropping: bool,
) -> Result<(), DetachError> {
    let mut registry = ATTACHMENT.lock().unwrap_or_else(PoisonError::into_inner);
    if registry.mode != AttachmentMode::Attached
        || !registry
            .state
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, state))
    {
        return Err(DetachError::not_installed());
    }
    registry.mode = AttachmentMode::Closing;
    // Overflow means no representable deadline: wait until the entered calls
    // drain rather than panic or truncate the caller's requested duration.
    let deadline = Instant::now().checked_add(timeout);
    while registry.in_flight != 0 {
        if let Some(deadline) = deadline {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let (next, result) = DRAINED
                .wait_timeout(registry, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            registry = next;
            if result.timed_out() {
                break;
            }
        } else {
            registry = DRAINED
                .wait(registry)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
    if registry.in_flight != 0 {
        if dropping {
            // The last call completes the transition after this handle goes away.
            registry.abandoned = true;
        } else {
            registry.mode = AttachmentMode::Attached;
        }
        return Err(DetachError::timeout());
    }
    registry.state.take();
    registry.mode = AttachmentMode::Detached;
    Ok(())
}

fn context(code: ErrorCode, message: &str, remediation: &str) -> ErrorContext {
    ErrorContext::new(
        code,
        message,
        Remediation::recoverable(message, [remediation]),
    )
}

fn policy_diagnostic(reason: PolicyRejection) -> OperationDiagnostic {
    OperationDiagnostic {
        code: crate::error_codes::SC_OBSERVABILITY_LOG_POLICY_REJECTED,
        message: reason.message().to_owned(),
        remediation: Remediation::recoverable(
            reason.remediation(),
            ["resubmit the corrected event explicitly"],
        ),
        at: Timestamp::now_utc(),
    }
}

fn policy_allows(state: &AttachmentState, event: &LogEvent) -> Result<(), EmitError> {
    match state.policy.decide(event) {
        BridgeEventDecision::Admit => Ok(()),
        BridgeEventDecision::Reject(reason) => {
            let diagnostic = policy_diagnostic(reason);
            *state
                .last_policy_rejection
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(diagnostic.clone());
            Err(EmitError::InvalidEvent { diagnostic })
        }
    }
}

pub(crate) fn attached_enabled(level: sc_observability_types::Level) -> Option<bool> {
    let call = enter_attachment(None)?;
    Some(handle::level_enabled(
        level,
        call.state.logger.level_state().effective_level,
    ))
}

pub(crate) fn is_attached() -> bool {
    ATTACHMENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .mode
        == AttachmentMode::Attached
}

pub(crate) fn submit_parts_if_attached(
    parts: crate::__private::EventParts,
) -> Result<Result<AdmissionOutcome, DropCause>, Box<crate::__private::EventParts>> {
    let Some(call) = enter_attachment(None) else {
        return Err(Box::new(parts));
    };
    Ok(submit_parts_to_attachment(call, parts))
}

fn submit_parts_to_attachment(
    call: AttachmentCall,
    parts: crate::__private::EventParts,
) -> Result<AdmissionOutcome, DropCause> {
    let state = &call.state;
    let mut event = mapping::assemble_event(
        parts,
        &state.service,
        &state.identity,
        &state.options.default_action,
    );
    event.trace = crate::context::current_trace();
    if let Err(cause) = policy_allows(state, &event) {
        return Err(handle::Rejection::drop_cause(&cause));
    }
    let outcome = match state.logger.try_log_with_outcome(event) {
        Ok(outcome) => Ok(outcome),
        Err(error) => Err(handle::event_drop_cause(&error)),
    };
    drop(call);
    outcome
}

pub(crate) fn submit_control(
    saved: &Weak<AttachmentState>,
    event: BridgeEvent,
) -> Result<AdmissionOutcome, EmitError> {
    let call = enter_attachment(Some(saved)).ok_or(EmitError::NotInstalled)?;
    submit_attachment_event(&call.state, event)
}

fn submit_attachment_event(
    state: &AttachmentState,
    event: BridgeEvent,
) -> Result<AdmissionOutcome, EmitError> {
    let mut assembled = crate::control::assemble_event(
        event,
        &state.service,
        &state.identity,
        &state.options.default_action,
    )?;
    assembled.trace = assembled.trace.or_else(crate::context::current_trace);
    policy_allows(state, &assembled)?;
    crate::control::submit_event(&state.logger, assembled)
}

pub(crate) fn submit_current_control(event: BridgeEvent) -> Result<AdmissionOutcome, EmitError> {
    let call = enter_attachment(None).ok_or(EmitError::NotInstalled)?;
    submit_attachment_event(&call.state, event)
}

pub(crate) fn flush_attached(
    saved: &Weak<AttachmentState>,
    timeout: Duration,
) -> Result<(), CoreFlushError> {
    let call = enter_attachment(Some(saved))
        .ok_or_else(|| attachment_flush_error("saved attachment is not installed"))?;
    flush_call(call, timeout)
}

fn flush_call(call: AttachmentCall, timeout: Duration) -> Result<(), CoreFlushError> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("sc-observability-log-attachment-flush".to_owned())
        .spawn(move || {
            // Keep the attachment call alive until the helper exits.  A timed-out
            // caller must not be able to detach while this helper still owns the
            // attachment's logger reference.
            let result = call.state.logger.flush();
            drop(call);
            let _ = sender.send(result);
        })
        .map_err(|source| {
            crate::error::flush_drain_as(
                crate::error::operation_context_with_source(
                    crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                    source.to_string(),
                    Remediation::not_recoverable("inspect thread resource availability"),
                    source,
                ),
                FailureClassification::Unavailable,
            )
        })?;
    match receiver.recv_timeout(timeout) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(crate::error::flush_drain(source.into_context())),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(crate::error::flush_drain_as(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT,
                format!("attachment flush did not complete within {timeout:?}"),
                Remediation::recoverable(
                    "retry after inspecting host logger health",
                    ["use a bounded timeout"],
                ),
            ),
            FailureClassification::timeout("flush"),
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(crate::error::flush_drain_as(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                "attachment flush helper ended without a result",
                Remediation::not_recoverable("inspect host logger health before retrying"),
            ),
            FailureClassification::Internal,
        )),
    }
}

pub(crate) fn flush_current_attachment(timeout: Duration) -> Result<(), CoreFlushError> {
    let call = enter_attachment(None)
        .ok_or_else(|| attachment_flush_error("no logger attachment is installed"))?;
    flush_call(call, timeout)
}

fn attachment_flush_error(message: &str) -> CoreFlushError {
    crate::error::flush_drain_as(
        crate::error::operation_context(
            crate::error_codes::SC_LOG_DETACH_NOT_INSTALLED,
            message,
            Remediation::not_recoverable("attach a logger before requesting a flush"),
        ),
        FailureClassification::Closed,
    )
}

pub(crate) fn mark_owned_running() {
    ATTACHMENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .mode = AttachmentMode::Owned;
}

pub(crate) fn mark_owned_stopped() {
    let mut registry = ATTACHMENT.lock().unwrap_or_else(PoisonError::into_inner);
    if registry.mode == AttachmentMode::Owned {
        registry.mode = AttachmentMode::Stopped;
    }
}

/// Maps every enabled `log` record through the shared guarded submission core.
#[derive(Debug)]
pub(crate) struct Bridge;

/// The one logger a facade record is submitted to, selected once per record.
enum Target {
    Attached(AttachmentCall),
    Installed(Arc<handle::Installed>),
}

impl log::Log for Bridge {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        handle::core_enabled(mapping::map_level(metadata.level()))
    }

    fn log(&self, record: &log::Record<'_>) {
        // Select the target once. As in the released facade, a record with no
        // attached or installed logger, or below the target's effective level,
        // is skipped silently: it never enters the guarded core or a drop counter.
        let target = match enter_attachment(None) {
            Some(call) => Target::Attached(call),
            None => match handle::current_installed() {
                Some(installed) => Target::Installed(installed),
                None => return,
            },
        };
        let effective = match &target {
            Target::Attached(call) => call.state.logger.level_state().effective_level,
            Target::Installed(installed) => installed.logger.level_state().effective_level,
        };
        if !handle::level_enabled(mapping::map_level(record.level()), effective) {
            return;
        }
        let _ = handle::submit_guarded(move || -> Result<(), DropCause> {
            match target {
                Target::Attached(call) => {
                    let mapped = mapping::record_to_parts(record, &call.state.options)
                        .map_err(|_label_error| DropCause::InvalidEvent)?;
                    for _ in 0..mapped.omitted_fields {
                        handle::record_drop(DropCause::InvalidEvent);
                    }
                    submit_parts_to_attachment(call, mapped.parts).map(|_| ())
                }
                Target::Installed(installed) => {
                    let mapped = mapping::record_to_parts(record, &installed.options)
                        .map_err(|_label_error| DropCause::InvalidEvent)?;
                    for _ in 0..mapped.omitted_fields {
                        handle::record_drop(DropCause::InvalidEvent);
                    }
                    handle::submit_to(&installed, mapped.parts).map(|_| ())
                }
            }
        });
    }

    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Admit;

    impl BridgeEventPolicy for Admit {
        fn decide(&self, _: &LogEvent) -> BridgeEventDecision {
            BridgeEventDecision::Admit
        }
    }

    /// Runs `test_name` alone in a child process: these tests own the global
    /// attachment registry, installed slot and drop counters.
    fn run_isolated(child_env: &str, test_name: &str) {
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", test_name, "--nocapture", "--test-threads=1"])
            .env(child_env, "1")
            .status()
            .expect("spawn isolated bridge regression");
        assert!(
            status.success(),
            "isolated bridge regression failed: {status}"
        );
    }

    fn attach(
        service: &str,
        root: &std::path::Path,
    ) -> (Arc<sc_observability::v2::Logger>, Arc<AttachmentState>) {
        let service = ServiceName::new(service).expect("service name");
        let logger = Arc::new(
            sc_observability::v2::Logger::new(crate::LoggerConfig::default_for(
                service.clone(),
                root.to_path_buf(),
            ))
            .expect("host logger"),
        );
        let state = Arc::new(AttachmentState {
            logger: Arc::clone(&logger),
            options: options(),
            policy: Arc::new(Admit),
            service,
            identity: ProcessIdentity::default(),
            last_policy_rejection: Mutex::new(None),
        });
        let mut registry = ATTACHMENT.lock().expect("attachment registry");
        assert!(
            registry.state.is_none(),
            "unit test owns attachment registry"
        );
        registry.mode = AttachmentMode::Attached;
        registry.state = Some(Arc::clone(&state));
        registry.in_flight = 0;
        registry.abandoned = false;
        drop(registry);
        (logger, state)
    }

    fn detach(logger: Arc<sc_observability::v2::Logger>, state: Arc<AttachmentState>) {
        close_attachment(&state, Duration::from_secs(5), false)
            .expect("all entered calls drain before releasing the host logger");
        ATTACHMENT.lock().expect("attachment registry").mode = AttachmentMode::Empty;
        drop(state);
        Arc::try_unwrap(logger)
            .unwrap_or_else(|_| panic!("attachment call releases the host logger"))
            .shutdown();
    }

    fn options() -> crate::BridgeOptions {
        crate::BridgeOptions {
            default_action: crate::ActionName::new("log.record").expect("action name"),
            parse_bracket_action: false,
        }
    }

    /// Submits one facade record at `level`, optionally from inside an emit scope.
    fn log_at(level: log::Level, in_emit_scope: bool) {
        let _scope = in_emit_scope.then(|| handle::EmitScope::enter().expect("outer emit scope"));
        log::Log::log(
            &Bridge,
            &log::Record::builder()
                .args(format_args!("facade record"))
                .level(level)
                .target("bridge.skip.test")
                .build(),
        );
    }

    #[test]
    fn facade_record_enters_an_attachment_once() {
        const CHILD_ENV: &str = "SC_OBSERVABILITY_LOG_BRIDGE_ENTRY_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            facade_record_enters_an_attachment_once_in_child();
            return;
        }
        run_isolated(
            CHILD_ENV,
            "bridge::tests::facade_record_enters_an_attachment_once",
        );
    }

    fn facade_record_enters_an_attachment_once_in_child() {
        let root = tempfile::tempdir().expect("temporary log root");
        let (logger, state) = attach("bridge-entry-test", root.path());

        ATTACHMENT_ENTRIES.with(|entries| entries.set(0));
        log_at(log::Level::Info, false);
        assert_eq!(
            ATTACHMENT_ENTRIES.with(Cell::get),
            1,
            "one facade record must retain its first attachment call"
        );

        detach(logger, state);
    }

    #[test]
    fn skipped_facade_records_count_no_drop() {
        const CHILD_ENV: &str = "SC_OBSERVABILITY_LOG_BRIDGE_SKIP_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            skipped_facade_records_count_no_drop_in_child();
            return;
        }
        run_isolated(
            CHILD_ENV,
            "bridge::tests::skipped_facade_records_count_no_drop",
        );
    }

    fn skipped_facade_records_count_no_drop_in_child() {
        // Nothing attached or installed: skipped, also inside an emit scope.
        assert!(handle::current_installed().is_none());
        let before = handle::dropped_events();
        log_at(log::Level::Error, false);
        log_at(log::Level::Error, true);
        assert_eq!(
            handle::dropped_events(),
            before,
            "a record with no logger is skipped without a drop"
        );

        // Attached, below the effective level: one attachment entry, no drop.
        let root = tempfile::tempdir().expect("temporary log root");
        let (logger, state) = attach("bridge-skip-test", root.path());
        let before = handle::dropped_events();
        ATTACHMENT_ENTRIES.with(|entries| entries.set(0));
        log_at(log::Level::Trace, false);
        log_at(log::Level::Trace, true);
        assert_eq!(
            ATTACHMENT_ENTRIES.with(Cell::get),
            2,
            "one entry per record"
        );
        assert_eq!(
            handle::dropped_events(),
            before,
            "a below-level attached record is skipped without a drop"
        );
        // Control: an enabled record inside an emit scope is a reentrant drop.
        let reentrant = handle::drop_count(DropCause::ReentrantEmit);
        log_at(log::Level::Info, true);
        assert_eq!(handle::drop_count(DropCause::ReentrantEmit), reentrant + 1);
        detach(logger, state);

        // Installed, below the effective level.
        let root = tempfile::tempdir().expect("temporary log root");
        let guard = crate::init(
            crate::LoggerConfig::default_for(
                ServiceName::new("bridge-skip-installed").expect("service name"),
                root.path().to_path_buf(),
            ),
            options(),
        )
        .expect("installed logger");
        let before = handle::dropped_events();
        log_at(log::Level::Trace, false);
        log_at(log::Level::Trace, true);
        assert_eq!(
            handle::dropped_events(),
            before,
            "a below-level installed record is skipped without a drop"
        );
        guard
            .shutdown(Duration::from_secs(5))
            .expect("installed logger shuts down");
    }
}
