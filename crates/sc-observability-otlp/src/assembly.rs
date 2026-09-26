//! Span assembly state machine for OTLP export.
//!
//! This module turns ordered `Started`/`Event`/`Ended` span signals into a
//! `CompleteSpan`, reports orphaned or inconsistent lifecycle transitions as
//! explicit errors, and maintains incomplete-span accounting for runtime
//! shutdown handling.
#![expect(
    clippy::missing_errors_doc,
    reason = "span-assembly failure behavior is documented at the telemetry facade level, and repeating it here would add low-signal boilerplate"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "small constructor/accessor methods are intentionally kept free of repetitive must_use decoration"
)]
use std::collections::HashMap;

use crate::error_codes;
use sc_observability_types::typed::EventFailure;
#[allow(
    deprecated,
    reason = "span assembly retains its published EventError adapter boundary"
)]
use sc_observability_types::{
    ErrorContext, EventError, Remediation, SpanEnded, SpanEvent, SpanRecord, SpanSignal,
    SpanStarted,
};

use sc_observability_types::v2::{
    EventError as V2EventError, SpanEnded as V2SpanEnded, SpanEvent as V2SpanEvent,
    SpanRecord as V2SpanRecord, SpanSignal as V2SpanSignal, SpanStarted as V2SpanStarted,
};

/// Completed span assembled from a start/event/end stream.
#[derive(Debug, Clone, PartialEq)]
pub struct CompleteSpan {
    /// Final completed span record.
    pub record: SpanRecord<SpanEnded>,
    /// Ordered span events attached before completion.
    pub events: Vec<SpanEvent>,
}

/// Completed 2.0 span staged for the canonical OTLP exporters.
///
/// The record and events retain the validated neutral model without a
/// transport-shaped intermediate representation. In particular, the ended
/// record owns span kind, links, status, timing, attributes, and trace flags.
#[derive(Debug, Clone, PartialEq)]
#[allow(
    dead_code,
    reason = "D.18 activates this staged 2.0 handoff after replacing the retained facade exports"
)]
pub(crate) struct V2CompleteSpan {
    pub(crate) record: V2SpanRecord<V2SpanEnded>,
    pub(crate) events: Vec<V2SpanEvent>,
}

/// Stateful span assembler used by telemetry export.
#[expect(
    missing_debug_implementations,
    reason = "the assembler is an internal state machine with no stable public debug contract beyond its functional behavior"
)]
pub struct SpanAssembler {
    started: HashMap<String, SpanRecord<SpanStarted>>,
    events: HashMap<String, Vec<SpanEvent>>,
}

/// Stateful assembler for the staged validated 2.0 span contract.
///
/// This is deliberately separate from [`SpanAssembler`]: the public facade
/// still exposes the retained 1.x projection types until D.18 activates the
/// canonical re-exports. Keeping both state machines here prevents a lossy
/// conversion during that handoff.
#[allow(
    dead_code,
    reason = "D.18 composes the staged 2.0 assembler through the final facade"
)]
pub(crate) struct V2SpanAssembler {
    started: HashMap<String, V2SpanRecord<V2SpanStarted>>,
    events: HashMap<String, Vec<V2SpanEvent>>,
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
    #[allow(
        deprecated,
        reason = "retained compatibility assembler method keeps the published EventError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use SpanAssembler::push_typed(); see migrate-error-api.md."
    )]
    pub fn push(&mut self, signal: SpanSignal) -> Result<Option<CompleteSpan>, EventError> {
        self.push_typed(signal).map_err(Into::into)
    }

    /// Pushes one lifecycle signal through the assembler with a neutral failure.
    pub fn push_typed(&mut self, signal: SpanSignal) -> Result<Option<CompleteSpan>, EventFailure> {
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
                    return Err(EventFailure::from_context(Box::new(ErrorContext::new(
                        error_codes::OTLP_EXPORT_TERMINAL,
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
                    return Err(EventFailure::from_context(Box::new(ErrorContext::new(
                        error_codes::OTLP_EXPORT_TERMINAL,
                        "received ended span without a matching started span",
                        Remediation::not_recoverable(
                            "emit started and ended span signals with the same trace context",
                        ),
                    ))));
                }
                let Some(events) = self.events.remove(&key) else {
                    return Err(EventFailure::from_context(Box::new(ErrorContext::new(
                        error_codes::OTLP_EXPORT_TERMINAL,
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

#[allow(
    dead_code,
    reason = "D.18 invokes this staged assembler after the canonical re-export switch"
)]
impl V2SpanAssembler {
    /// Creates an empty validated-signal assembler.
    pub(crate) fn new() -> Self {
        Self {
            started: HashMap::new(),
            events: HashMap::new(),
        }
    }

    /// Assembles one validated neutral signal without rebuilding its fields.
    pub(crate) fn push(
        &mut self,
        signal: V2SpanSignal,
    ) -> Result<Option<V2CompleteSpan>, V2EventError> {
        match signal {
            V2SpanSignal::Started(record) => {
                let key = span_key(
                    record.trace().trace_id.as_str(),
                    record.trace().span_id.as_str(),
                );
                self.events.insert(key.clone(), Vec::new());
                self.started.insert(key, record);
                Ok(None)
            }
            V2SpanSignal::Event(event) => {
                let key = span_key(event.trace.trace_id.as_str(), event.trace.span_id.as_str());
                let Some(started) = self.started.get(&key) else {
                    return Err(v2_lifecycle_error(
                        "received span event without a matching started span",
                        "emit started, event, and ended span signals in order",
                    ));
                };
                if started.trace() != &event.trace {
                    return Err(v2_lifecycle_error(
                        "received span event with mismatched trace context",
                        "preserve trace identifiers, parent, and flags across one span lifecycle",
                    ));
                }
                self.events.entry(key).or_default().push(event);
                Ok(None)
            }
            V2SpanSignal::Ended(record) => {
                let key = span_key(
                    record.trace().trace_id.as_str(),
                    record.trace().span_id.as_str(),
                );
                let Some(started) = self.started.get(&key) else {
                    return Err(v2_lifecycle_error(
                        "received ended span without a matching started span",
                        "emit started and ended span signals with the same trace context",
                    ));
                };
                if started.trace() != record.trace() {
                    return Err(v2_lifecycle_error(
                        "received ended span with mismatched trace context",
                        "preserve trace identifiers, parent, and flags across one span lifecycle",
                    ));
                }
                let started = self
                    .started
                    .remove(&key)
                    .expect("started span was checked before removal");
                let Some(events) = self.events.remove(&key) else {
                    self.started.insert(key, started);
                    return Err(v2_lifecycle_error(
                        "missing span event buffer for a started span",
                        "restart telemetry to restore span assembly state",
                    ));
                };
                Ok(Some(V2CompleteSpan { record, events }))
            }
        }
    }

    /// Drops incomplete spans only when the caller performs its final flush.
    pub(crate) fn flush_incomplete(&mut self) -> usize {
        let dropped = self.started.len();
        self.started.clear();
        self.events.clear();
        dropped
    }
}

impl Default for V2SpanAssembler {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(
    dead_code,
    reason = "only the staged D.18 assembler activation constructs this failure"
)]
fn v2_lifecycle_error(message: &str, recovery: &str) -> V2EventError {
    V2EventError::Routing {
        context: Box::new(ErrorContext::new(
            error_codes::OTLP_EXPORT_TERMINAL,
            message,
            Remediation::not_recoverable(recovery),
        )),
    }
}

impl Default for SpanAssembler {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn span_key(trace_id: &str, span_id: &str) -> String {
    let mut key = String::with_capacity(trace_id.len() + span_id.len() + 1);
    key.push_str(trace_id);
    key.push(':');
    key.push_str(span_id);
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_types::v2::{
        AttributeValue, Attributes, SpanKind, SpanLink, SpanStatus, TraceContext, TraceFlags,
    };
    use sc_observability_types::{ActionName, DurationMs, ServiceName, SpanId, Timestamp, TraceId};

    fn trace(flags: u8) -> TraceContext {
        TraceContext::new(
            TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
            SpanId::new("0123456789abcdef").expect("valid span id"),
            TraceFlags::new(flags),
        )
    }

    fn started(trace: TraceContext) -> V2SpanRecord<V2SpanStarted> {
        let link = SpanLink::new(
            TraceId::new("fedcba9876543210fedcba9876543210").expect("valid linked trace id"),
            SpanId::new("fedcba9876543210").expect("valid linked span id"),
            TraceFlags::new(0x03),
            Attributes::from([(
                "link.source".to_owned(),
                AttributeValue::String("caller".to_owned()),
            )]),
        );
        V2SpanRecord::new(
            Timestamp::UNIX_EPOCH,
            ServiceName::new("test-service").expect("valid service"),
            ActionName::new("test.operation").expect("valid action"),
            trace,
            Attributes::from([(
                "scope.name".to_owned(),
                AttributeValue::String("test".to_owned()),
            )]),
        )
        .with_kind(SpanKind::Client)
        .with_links(vec![link])
    }

    #[test]
    fn v2_assembly_preserves_completed_span_fields_and_events() {
        let trace = trace(0xa5);
        let started_record = started(trace.clone());
        let ended = started_record
            .clone()
            .end(SpanStatus::Ok, DurationMs::from(17));
        let event = V2SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace: trace.clone(),
            name: ActionName::new("test.event").expect("valid event name"),
            attributes: Attributes::from([("event.kind".to_owned(), AttributeValue::Int(7))]),
            diagnostic: None,
        };
        let mut assembler = V2SpanAssembler::new();

        assert!(
            assembler
                .push(V2SpanSignal::Started(started_record))
                .expect("started signal")
                .is_none()
        );
        assert!(
            assembler
                .push(V2SpanSignal::Event(event.clone()))
                .expect("event signal")
                .is_none()
        );
        let complete = assembler
            .push(V2SpanSignal::Ended(ended.clone()))
            .expect("ended signal")
            .expect("completed span");

        assert_eq!(complete.record, ended);
        assert_eq!(complete.record.kind(), SpanKind::Client);
        assert_eq!(complete.record.trace().flags.bits(), 0xa5);
        assert_eq!(complete.record.links().len(), 1);
        assert_eq!(complete.record.duration_ms(), Some(DurationMs::from(17)));
        assert_eq!(complete.events, vec![event]);
    }

    #[test]
    fn v2_assembly_rejects_mismatched_flags_without_dropping_started_span() {
        let started_record = started(trace(0x01));
        let ended = started(trace(0x00)).end(SpanStatus::Error, DurationMs::from(1));
        let mut assembler = V2SpanAssembler::new();

        assembler
            .push(V2SpanSignal::Started(started_record))
            .expect("started signal");
        let error = assembler
            .push(V2SpanSignal::Ended(ended))
            .expect_err("different flags are not the same lifecycle");

        assert_eq!(error.diagnostic().code, error_codes::OTLP_EXPORT_TERMINAL);
        assert_eq!(assembler.flush_incomplete(), 1);
    }

    #[test]
    fn v2_assembly_drops_unfinished_spans_only_at_final_flush() {
        let mut assembler = V2SpanAssembler::new();
        assembler
            .push(V2SpanSignal::Started(started(trace(0x01))))
            .expect("started signal");

        assert_eq!(assembler.flush_incomplete(), 1);
        assert_eq!(assembler.flush_incomplete(), 0);
    }
}
