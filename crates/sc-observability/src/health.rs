use std::sync::Mutex;

use sc_observability_types::{DiagnosticSummary, QueryError, QueryHealthReport, QueryHealthState};

/// Internal tracker for query/follow health reporting.
#[derive(Debug)]
pub(crate) struct QueryHealthTracker {
    report: Mutex<QueryHealthReport>,
}

impl QueryHealthTracker {
    pub(crate) fn new(initial_state: QueryHealthState) -> Self {
        Self {
            report: Mutex::new(QueryHealthReport {
                state: initial_state,
                last_error: None,
            }),
        }
    }

    pub(crate) fn snapshot(&self) -> QueryHealthReport {
        self.report.lock().expect("query health poisoned").clone()
    }

    pub(crate) fn mark_healthy(&self) {
        let mut report = self.report.lock().expect("query health poisoned");
        report.state = QueryHealthState::Healthy;
        report.last_error = None;
    }

    pub(crate) fn mark_unavailable(&self, summary: Option<DiagnosticSummary>) {
        let mut report = self.report.lock().expect("query health poisoned");
        report.state = QueryHealthState::Unavailable;
        report.last_error = summary;
    }

    pub(crate) fn record_error(&self, error: &QueryError) {
        let summary = Some(DiagnosticSummary::from(error.diagnostic()));
        let mut report = self.report.lock().expect("query health poisoned");
        match error {
            QueryError::InvalidQuery(_) => {}
            QueryError::DecodeError(_) => {
                report.state = QueryHealthState::Degraded;
                report.last_error = summary;
            }
            QueryError::IoError(_) | QueryError::Unavailable(_) | QueryError::Shutdown(_) => {
                report.state = QueryHealthState::Unavailable;
                report.last_error = summary;
            }
        }
    }

    pub(crate) fn record_result<T>(&self, result: &Result<T, QueryError>) {
        match result {
            Ok(_) => self.mark_healthy(),
            Err(error) => self.record_error(error),
        }
    }
}
