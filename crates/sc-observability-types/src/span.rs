use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{ActionName, Diagnostic, DurationMs, ServiceName, Timestamp, TraceContext};

/// Final span status for a completed span record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// The span completed successfully.
    Ok,
    /// The span completed with an error.
    Error,
    /// The span completed without an explicit outcome.
    Unset,
}

/// Typestate marker for a started-but-not-yet-ended span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanStarted;

/// Typestate marker for a completed span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanEnded;

/// Producer-facing span record whose lifecycle is encoded via typestate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpanRecord<S> {
    timestamp: Timestamp,
    service: ServiceName,
    name: ActionName,
    trace: TraceContext,
    status: SpanStatus,
    diagnostic: Option<Diagnostic>,
    attributes: Map<String, Value>,
    duration_ms: Option<DurationMs>,
    marker: PhantomData<S>,
}

impl SpanRecord<SpanStarted> {
    /// Creates a new started span record.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_observability_types::{
    ///     ActionName, ServiceName, SpanId, SpanRecord, SpanStarted, Timestamp, TraceContext, TraceId,
    /// };
    ///
    /// let record = SpanRecord::<SpanStarted>::new(
    ///     Timestamp::UNIX_EPOCH,
    ///     ServiceName::new("demo").expect("valid service"),
    ///     ActionName::new("demo.run").expect("valid action"),
    ///     TraceContext {
    ///         trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace"),
    ///         span_id: SpanId::new("0123456789abcdef").expect("valid span"),
    ///         parent_span_id: None,
    ///     },
    ///     Default::default(),
    /// );
    ///
    /// assert_eq!(record.name().as_str(), "demo.run");
    /// ```
    pub fn new(
        timestamp: Timestamp,
        service: ServiceName,
        name: ActionName,
        trace: TraceContext,
        attributes: Map<String, Value>,
    ) -> Self {
        Self {
            timestamp,
            service,
            name,
            trace,
            status: SpanStatus::Unset,
            diagnostic: None,
            attributes,
            duration_ms: None,
            marker: PhantomData,
        }
    }

    /// Attaches a diagnostic to the started span before completion.
    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Consumes the started span and returns the only valid completed span form.
    pub fn end(self, status: SpanStatus, duration: DurationMs) -> SpanRecord<SpanEnded> {
        SpanRecord {
            timestamp: self.timestamp,
            service: self.service,
            name: self.name,
            trace: self.trace,
            status,
            diagnostic: self.diagnostic,
            attributes: self.attributes,
            duration_ms: Some(duration),
            marker: PhantomData,
        }
    }
}

impl<S> SpanRecord<S> {
    /// Returns the timestamp recorded for the span lifecycle event.
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns the service that emitted the span.
    pub fn service(&self) -> &ServiceName {
        &self.service
    }

    /// Returns the stable action/name associated with the span.
    pub fn name(&self) -> &ActionName {
        &self.name
    }

    /// Returns the trace context for the span.
    pub fn trace(&self) -> &TraceContext {
        &self.trace
    }

    /// Returns the current typestate-derived span status.
    pub fn status(&self) -> SpanStatus {
        self.status
    }

    /// Returns the optional diagnostic attached to the span.
    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }

    /// Returns immutable span attributes.
    pub fn attributes(&self) -> &Map<String, Value> {
        &self.attributes
    }
}

impl SpanRecord<SpanEnded> {
    /// Returns the final duration recorded for the completed span.
    ///
    /// When the record was created through `SpanRecord::end`, this returns
    /// `Some(duration)`. Deserializing malformed external input can still
    /// produce a completed span without a duration, so the accessor remains
    /// fallible by returning `None`.
    pub fn duration_ms(&self) -> Option<DurationMs> {
        self.duration_ms
    }
}

/// Event attached to a span timeline without creating a child span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanEvent {
    /// UTC event timestamp.
    pub timestamp: Timestamp,
    /// Trace/span correlation for the event.
    pub trace: TraceContext,
    /// Stable event name.
    pub name: ActionName,
    /// Structured event attributes.
    pub attributes: Map<String, Value>,
    /// Optional diagnostic attached to the event.
    pub diagnostic: Option<Diagnostic>,
}

/// Generic span lifecycle signal used by projectors and telemetry assembly.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SpanSignal {
    /// Started span record.
    Started(SpanRecord<SpanStarted>),
    /// Point-in-time event on an existing span.
    Event(SpanEvent),
    /// Completed span record.
    Ended(SpanRecord<SpanEnded>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::{Remediation, error_codes};
    use crate::{SpanId, TraceId};

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn action_name() -> ActionName {
        ActionName::new("observation.received").expect("valid action name")
    }

    fn trace_context() -> TraceContext {
        TraceContext {
            trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
            span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
            parent_span_id: Some(SpanId::new("fedcba9876543210").expect("valid parent span id")),
        }
    }

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: error_codes::DIAGNOSTIC_INVALID,
            message: "diagnostic invalid".to_string(),
            cause: Some("invalid example".to_string()),
            remediation: Remediation::recoverable(
                "fix the input",
                ["rerun the command", "review the docs"],
            ),
            docs: Some("https://example.test/docs".to_string()),
            details: Map::from_iter([("key".to_string(), json!("value"))]),
        }
    }

    #[test]
    fn span_signal_round_trips_through_serde() {
        let mut attributes = Map::new();
        attributes.insert("tool".to_string(), json!("rg"));

        let started = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            action_name(),
            trace_context(),
            attributes.clone(),
        )
        .with_diagnostic(diagnostic());

        let ended = started.clone().end(SpanStatus::Ok, DurationMs::from(123));
        let signal = SpanSignal::Ended(ended);
        let encoded = serde_json::to_value(&signal).expect("serialize span signal");

        assert_eq!(encoded["Ended"]["status"], "Ok");
        assert_eq!(encoded["Ended"]["duration_ms"], 123);
    }

    #[test]
    fn span_record_end_transitions_to_span_ended() {
        let span = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            action_name(),
            trace_context(),
            Map::new(),
        );

        let ended = span.end(SpanStatus::Error, DurationMs::from(88));

        assert_eq!(ended.status(), SpanStatus::Error);
        assert_eq!(ended.duration_ms(), Some(DurationMs::from(88)));
        assert_eq!(ended.service().as_str(), "sc-observability");
    }

    #[test]
    fn span_record_accessors_preserve_started_values() {
        let mut attributes = Map::new();
        attributes.insert("count".to_string(), json!(2));
        let span = SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            action_name(),
            trace_context(),
            attributes.clone(),
        )
        .with_diagnostic(diagnostic());

        assert_eq!(span.timestamp(), Timestamp::UNIX_EPOCH);
        assert_eq!(span.name().as_str(), "observation.received");
        assert_eq!(
            span.trace().trace_id.as_str(),
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(span.status(), SpanStatus::Unset);
        assert_eq!(span.diagnostic(), Some(&diagnostic()));
        assert_eq!(span.attributes(), &attributes);
    }
}
