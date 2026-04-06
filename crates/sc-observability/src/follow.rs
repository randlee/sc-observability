#![expect(
    clippy::missing_errors_doc,
    reason = "follow-session error behavior is documented at the public facade level, and repeating it here would add low-signal boilerplate"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "lightweight follow accessors are intentionally kept free of repetitive must_use decoration"
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sc_observability_types::{LogQuery, LogSnapshot, QueryError, QueryHealthReport};

use crate::health::QueryHealthTracker;
use crate::query::{self, TrackedFile};

/// Tail-style follow session over the active JSONL log and its rotation set.
#[derive(Debug)]
pub struct LogFollowSession {
    active_log_path: PathBuf,
    query: LogQuery,
    tracked_files: Vec<TrackedFile>,
    health: Arc<QueryHealthTracker>,
    shutdown: Option<Arc<AtomicBool>>,
}

impl LogFollowSession {
    pub(crate) fn with_health(
        active_log_path: PathBuf,
        query: LogQuery,
        health: Arc<QueryHealthTracker>,
        shutdown: Option<Arc<AtomicBool>>,
    ) -> Result<Self, QueryError> {
        query.validate()?;
        let tracked_files = query::start_follow_tracking(&active_log_path)?;
        Ok(Self {
            active_log_path,
            query,
            tracked_files,
            health,
            shutdown,
        })
    }

    /// Polls for newly appended matching log records since the last call.
    pub fn poll(&mut self) -> Result<LogSnapshot, QueryError> {
        if self
            .shutdown
            .as_ref()
            .is_some_and(|shutdown| shutdown.load(Ordering::SeqCst))
        {
            let result = Err(query::shutdown_error());
            self.health.record_result(&result);
            return result;
        }

        let result = query::poll_follow_snapshot(
            &self.active_log_path,
            &self.query,
            &mut self.tracked_files,
        );
        match result {
            Ok(outcome) => {
                if let Some(summary) = outcome.reset_summary {
                    self.health.record_nonfatal_summary(summary);
                } else {
                    self.health.mark_healthy();
                }
                Ok(outcome.snapshot)
            }
            Err(error) => {
                self.health.record_error(&error);
                Err(error)
            }
        }
    }

    /// Returns the current query/follow health snapshot for this session.
    pub fn health(&self) -> QueryHealthReport {
        self.health.snapshot()
    }
}
