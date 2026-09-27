//! Caller-runtime scheduling primitives for the future OTLP wire adapter.
//!
//! The pinned official SDK cannot publicly construct lossless span events,
//! links, or pre-aggregated metric requests. A proposed replacement projects
//! all signals to OTLP protobuf requests. These primitives deliberately contain
//! no SDK-specific record construction, so either transport retains the
//! caller-runtime, admission, and resource-grouping invariants.

use std::future::Future;

use tokio::runtime::Handle;

use crate::contracts::{ExportRecord, Resource};
use crate::lifecycle::Admitted;
use sc_observability_types::v2::ExportError;

/// Caller-owned runtime used for asynchronous OTLP export futures.
#[derive(Clone)]
pub(crate) struct CallerRuntime {
    handle: Handle,
}

impl CallerRuntime {
    /// Captures the runtime already entered by the embedding application.
    ///
    /// A missing runtime is a typed construction failure for an enabled async
    /// backend: this module never creates one or blocks the caller thread.
    #[must_use]
    pub(crate) fn try_capture() -> Option<Self> {
        Handle::try_current().ok().map(|handle| Self { handle })
    }

    /// Runs a previously projected export future on the caller's runtime.
    ///
    /// The D.6 admission permit moves into the task. Its `complete` method is
    /// the success/failure terminal path; dropping it records cancellation or
    /// runtime teardown without a separate accounting counter.
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

/// One resource-homogeneous export group.
///
/// A collector request may carry only one resource. Grouping therefore occurs
/// before admission/scheduling; record order remains stable inside each group
/// and group order is the resource's first appearance in the input batch.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResourceGroup<T> {
    /// Shared resource for every record in this group.
    pub(crate) resource: Resource,
    /// Records belonging to that resource, preserving input order.
    pub(crate) records: Vec<T>,
}

/// Groups neutral records by their exact resource without reordering within a
/// resource. This is intentionally transport-neutral so it applies to logs,
/// spans, and metrics alike.
pub(crate) fn group_by_resource<T: Clone>(batch: &[ExportRecord<T>]) -> Vec<ResourceGroup<T>> {
    let mut groups = Vec::new();
    for item in batch {
        if let Some(group) = groups
            .iter_mut()
            .find(|group: &&mut ResourceGroup<T>| group.resource == item.resource)
        {
            group.records.push(item.record.clone());
        } else {
            groups.push(ResourceGroup {
                resource: item.resource.clone(),
                records: vec![item.record.clone()],
            });
        }
    }
    groups
}
