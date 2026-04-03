use std::collections::HashMap;

use crate::error_codes;
use sc_observability_types::{
    ErrorContext, EventError, Remediation, SpanEnded, SpanEvent, SpanRecord, SpanSignal,
    SpanStarted, Timestamp,
};

/// Completed span assembled from a start/event/end stream.
#[derive(Debug, Clone, PartialEq)]
pub struct CompleteSpan {
    /// Final completed span record.
    pub record: SpanRecord<SpanEnded>,
    /// Ordered span events attached before completion.
    pub events: Vec<SpanEvent>,
}

/// Stateful span assembler used by telemetry export.
pub struct SpanAssembler {
    started: HashMap<String, SpanRecord<SpanStarted>>,
    events: HashMap<String, Vec<SpanEvent>>,
}

impl SpanAssembler {
    /// Creates an empty assembler.
    pub fn new() -> Self {
        Self {
            started: HashMap::new(),
            events: HashMap::new(),
        }
    }

    pub(crate) fn has_started(&self, trace_id: &str, span_id: &str) -> bool {
        self.started.contains_key(&span_key(trace_id, span_id))
    }

    /// Pushes one lifecycle signal through the assembler.
    pub fn push(&mut self, signal: SpanSignal) -> Result<Option<CompleteSpan>, EventError> {
        match signal {
            SpanSignal::Started(record) => {
                let key = span_key(
                    record.trace().trace_id.as_str(),
                    record.trace().span_id.as_str(),
                );
                self.events.insert(key.clone(), Vec::new());
                self.started.insert(key, record);
                Ok(None)
            }
            SpanSignal::Event(event) => {
                let key = span_key(event.trace.trace_id.as_str(), event.trace.span_id.as_str());
                if !self.started.contains_key(&key) {
                    return Err(EventError(Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED,
                        "received span event without a matching started span",
                        Remediation::not_recoverable(
                            "emit started, event, and ended span signals in order",
                        ),
                    ))));
                }
                self.events.entry(key).or_default().push(event);
                Ok(None)
            }
            SpanSignal::Ended(record) => {
                let key = span_key(
                    record.trace().trace_id.as_str(),
                    record.trace().span_id.as_str(),
                );
                if self.started.remove(&key).is_none() {
                    return Err(EventError(Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED,
                        "received ended span without a matching started span",
                        Remediation::not_recoverable(
                            "emit started and ended span signals with the same trace context",
                        ),
                    ))));
                }
                let Some(events) = self.events.remove(&key) else {
                    return Err(EventError(Box::new(ErrorContext::new(
                        error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED,
                        "missing span event buffer for a started span",
                        Remediation::not_recoverable(
                            "restart telemetry to restore span assembly state",
                        ),
                    ))));
                };
                Ok(Some(CompleteSpan { record, events }))
            }
        }
    }

    /// Drops any incomplete span state and returns the number of dropped spans.
    pub fn flush_incomplete(&mut self) -> usize {
        let dropped = self.started.len();
        self.started.clear();
        self.events.clear();
        dropped
    }

    #[cfg(test)]
    pub(crate) fn remove_event_buffer(&mut self, key: &str) -> Option<Vec<SpanEvent>> {
        self.events.remove(key)
    }
}

impl Default for SpanAssembler {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn span_timestamp(span: &SpanSignal) -> Timestamp {
    match span {
        SpanSignal::Started(record) => record.timestamp(),
        SpanSignal::Event(event) => event.timestamp,
        SpanSignal::Ended(record) => record.timestamp(),
    }
}

pub(crate) fn span_key(trace_id: &str, span_id: &str) -> String {
    format!("{trace_id}:{span_id}")
}
