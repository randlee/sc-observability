use std::path::PathBuf;
use std::sync::Arc;

use sc_observability_types::{LogQuery, LogSnapshot, QueryError, QueryHealthState};

use crate::follow::LogFollowSession;
use crate::health::QueryHealthTracker;
use crate::query;

/// Independent JSONL reader for historical query and follow operations.
#[derive(Debug, Clone)]
pub struct JsonlLogReader {
    active_log_path: PathBuf,
}

impl JsonlLogReader {
    /// Creates a reader over the active JSONL log path and its rotation set.
    pub fn new(active_log_path: PathBuf) -> Self {
        Self { active_log_path }
    }

    /// Queries the current active JSONL log and visible rotation set.
    pub fn query(&self, query: &LogQuery) -> Result<LogSnapshot, QueryError> {
        query::query_snapshot(&self.active_log_path, query)
    }

    /// Starts a tail-style follow session beginning at the end of the current visible log set.
    pub fn follow(&self, query: LogQuery) -> Result<LogFollowSession, QueryError> {
        LogFollowSession::with_health(
            self.active_log_path.clone(),
            query,
            Arc::new(QueryHealthTracker::new(QueryHealthState::Healthy)),
        )
    }
}
