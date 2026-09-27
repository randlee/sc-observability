//! Official SDK/Tokio adapter primitives owned by obs-d-7.
//!
//! The SDK exporters return futures.  D.7 deliberately captures the caller's
//! Tokio handle instead of creating a runtime or blocking the caller.  Each
//! scheduled export owns its D.6 admission permit; completing that permit is
//! the single terminal accounting path for success, failure, cancellation, and
//! shutdown races.

use std::future::Future;

use tokio::runtime::Handle;

use crate::lifecycle::Admitted;
use sc_observability_types::v2::ExportError;

/// Caller-owned runtime used for official SDK export futures.
#[derive(Clone)]
pub(crate) struct CallerRuntime {
    handle: Handle,
}

impl CallerRuntime {
    /// Captures the runtime already entered by the embedding application.
    ///
    /// `None` is an initialization failure for the SDK backend: this adapter
    /// never constructs a hidden runtime and never blocks on the caller thread.
    #[must_use]
    pub(crate) fn try_capture() -> Option<Self> {
        Handle::try_current().ok().map(|handle| Self { handle })
    }

    /// Runs an SDK export future on the captured caller runtime.
    ///
    /// D.6 owns admission and accounting.  The permit is intentionally moved
    /// into the spawned task, so both an SDK result and task cancellation use
    /// `Admitted::complete`/`Drop` rather than creating a second counter path.
    pub(crate) fn spawn_export<T, F>(
        &self,
        admitted: Admitted<T>,
        export: F,
    ) -> tokio::task::JoinHandle<()>
    where
        T: Send + 'static,
        F: Future<Output = Result<(), ExportError>> + Send + 'static,
    {
        self.handle.spawn(async move {
            let _value = admitted.complete(export.await);
        })
    }
}
