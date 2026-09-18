//! The `log::Log` implementation installed by [`init`](crate::init).

use crate::handle;
use crate::{DropCause, mapping};

/// Maps every enabled `log` record to a `LogEvent` and submits it through the
/// guarded submission core (one guard per record, shared with `__private::emit`
/// and `LogControl::try_log`).
#[derive(Debug)]
pub(crate) struct Bridge;

impl log::Log for Bridge {
    /// Retains every compiled facade site through Trace. The staged core applies
    /// the one effective runtime filter during admission.
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        handle::core_enabled(mapping::map_level(metadata.level()))
    }

    /// Never blocks on I/O or queue capacity and never panics.
    ///
    /// Records below the threshold return at once (a lock-free check that runs no
    /// user code). Everything else runs inside one emit guard, entered exactly once
    /// per record: the slot read, `BridgeOptions` lookup, target/action labelling,
    /// rendering `record.args()` and every key-value (user `Display`/`Debug`
    /// code), event assembly, redaction and `try_log`. A panic anywhere in that
    /// sequence is counted once as `DropCause::LoggerPanicked`; a record logged
    /// while the guard is active on this thread (a formatter, panic hook, sink or
    /// redactor that logs) is dropped and counted once as
    /// `DropCause::ReentrantEmit`; a target the sanitizer cannot label is counted
    /// as `DropCause::InvalidEvent`. A key-value whose key is empty or reserved is
    /// omitted (the record still emits) and counted once as `InvalidEvent`.
    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // The facade has no result channel: the result is discarded only after
        // `submit_guarded` has counted a rejection under exactly one DropCause.
        let _ = handle::submit_guarded(|| {
            let installed = handle::current_installed().ok_or(DropCause::NotInstalled)?;
            let mapped = mapping::record_to_parts(record, &installed.options)
                .map_err(|_label_error| DropCause::InvalidEvent)?;
            for _ in 0..mapped.omitted_fields {
                // An omitted key-value is not a dropped event: the record still emits.
                handle::record_drop(DropCause::InvalidEvent);
            }
            handle::submit_to(&installed, mapped.parts)
        });
    }

    /// No-op by design: `log::Log::flush` has no timeout or error channel, and
    /// sc-observability's flush is unbounded (`maintenance.rs:137-164`), so
    /// delegating would let any `log::logger().flush()` caller hang. Use
    /// [`LogControl::flush`](crate::LogControl::flush) (or
    /// [`LogGuard::flush`](crate::LogGuard::flush)) for a bounded flush.
    fn flush(&self) {}
}
