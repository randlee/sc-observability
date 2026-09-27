//! Crate-visible recording exporters shared by contract and lifecycle tests.

#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
use crate::assembly::CompleteSpan as LegacyCompleteSpan;
#[cfg(test)]
use crate::contracts::{
    CompleteSpan, ExportRecord, LogExporter, LogRecord, MetricExporter, TraceExporter,
};
#[cfg(test)]
use crate::error_codes;
#[cfg(test)]
use sc_observability_types::v2::{ExportError, MetricRecord};
#[cfg(test)]
use sc_observability_types::{
    ErrorContext, LogEvent, MetricRecord as LegacyMetricRecord, Remediation,
};

/// D.21 recording exporters shared by lifecycle and facade contract tests.
/// They intentionally record only call counts and preserve the same failure
/// switch for both legacy facade records and canonical export records.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingLogExporter {
    pub(crate) calls: Mutex<Vec<usize>>,
    pub(crate) fail: AtomicBool,
}

#[cfg(test)]
impl RecordingLogExporter {
    fn result(&self, count: usize) -> Result<(), ExportError> {
        self.calls.lock().expect("calls poisoned").push(count);
        if self.fail.load(Ordering::SeqCst) {
            Err(ExportError::Transport {
                context: Box::new(ErrorContext::new(
                    error_codes::OTLP_EXPORT_TERMINAL,
                    "log export failed",
                    Remediation::not_recoverable("test exporter failure"),
                )),
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
impl LogExporter<LogEvent> for RecordingLogExporter {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError> {
        self.result(batch.len())
    }
}

#[cfg(test)]
impl LogExporter for RecordingLogExporter {
    fn export_logs(&self, batch: &[ExportRecord<LogRecord>]) -> Result<(), ExportError> {
        self.result(batch.len())
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingTraceExporter {
    pub(crate) calls: Mutex<Vec<usize>>,
    pub(crate) fail: AtomicBool,
}

#[cfg(test)]
impl RecordingTraceExporter {
    fn result(&self, count: usize) -> Result<(), ExportError> {
        self.calls.lock().expect("calls poisoned").push(count);
        if self.fail.load(Ordering::SeqCst) {
            Err(ExportError::Transport {
                context: Box::new(ErrorContext::new(
                    error_codes::OTLP_EXPORT_TERMINAL,
                    "trace export failed",
                    Remediation::not_recoverable("test exporter failure"),
                )),
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
impl TraceExporter<LegacyCompleteSpan> for RecordingTraceExporter {
    fn export_spans(&self, batch: &[LegacyCompleteSpan]) -> Result<(), ExportError> {
        self.result(batch.len())
    }
}

#[cfg(test)]
impl TraceExporter for RecordingTraceExporter {
    fn export_spans(&self, batch: &[ExportRecord<CompleteSpan>]) -> Result<(), ExportError> {
        self.result(batch.len())
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingMetricExporter {
    pub(crate) calls: Mutex<Vec<usize>>,
    pub(crate) fail: AtomicBool,
}

#[cfg(test)]
impl RecordingMetricExporter {
    fn result(&self, count: usize) -> Result<(), ExportError> {
        self.calls.lock().expect("calls poisoned").push(count);
        if self.fail.load(Ordering::SeqCst) {
            Err(ExportError::Transport {
                context: Box::new(ErrorContext::new(
                    error_codes::OTLP_EXPORT_TERMINAL,
                    "metric export failed",
                    Remediation::not_recoverable("test exporter failure"),
                )),
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
impl MetricExporter<LegacyMetricRecord> for RecordingMetricExporter {
    fn export_metrics(&self, batch: &[LegacyMetricRecord]) -> Result<(), ExportError> {
        self.result(batch.len())
    }
}

#[cfg(test)]
impl MetricExporter for RecordingMetricExporter {
    fn export_metrics(&self, batch: &[ExportRecord<MetricRecord>]) -> Result<(), ExportError> {
        self.result(batch.len())
    }
}
