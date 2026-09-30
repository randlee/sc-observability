#![allow(
    deprecated,
    reason = "subscriber and projector registrations retain their published legacy trait signatures"
)]

use std::sync::Arc;

use serde_json::{Map, Value};

use crate::errors_v2::{
    ProjectionError as CanonicalProjectionError, SubscriberError as CanonicalSubscriberError,
};
use crate::observation_v2 as canonical;
use crate::signals_v2 as model;
use crate::{
    ErrorContext, LogEvent, MetricKind, MetricRecord, Observable, Observation, ProjectionError,
    Remediation, SpanEvent, SpanRecord, SpanSignal, SpanStarted, SubscriberError, TraceContext,
    error_codes,
};

type SubscriberRegistrationParts<T> = (
    Arc<dyn ObservationSubscriber<T>>,
    Option<Arc<dyn ObservationFilter<T>>>,
);

type ProjectionRegistrationParts<T> = (
    Option<Arc<dyn LogProjector<T>>>,
    Option<Arc<dyn SpanProjector<T>>>,
    Option<Arc<dyn MetricProjector<T>>>,
    Option<Arc<dyn ObservationFilter<T>>>,
);

/// Open subscriber contract for typed observations.
pub trait ObservationSubscriber<T>: Send + Sync
where
    T: Observable,
{
    /// Consumes one routed observation.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriberError`] when the subscriber rejects or cannot
    /// process the observation.
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberError>;
}

/// Open filter contract evaluated before subscriber or projector execution.
pub trait ObservationFilter<T>: Send + Sync
where
    T: Observable,
{
    /// Returns whether the observation should proceed to the subscriber or projector.
    fn accepts(&self, observation: &Observation<T>) -> bool;
}

/// Open projector contract from typed observations into log events.
pub trait LogProjector<T>: Send + Sync
where
    T: Observable,
{
    /// Projects one observation into zero or more log events.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionError`] when the projector cannot derive log output
    /// for the supplied observation.
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError>;
}

/// Open projector contract from typed observations into span signals.
pub trait SpanProjector<T>: Send + Sync
where
    T: Observable,
{
    /// Projects one observation into zero or more span lifecycle signals.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionError`] when the projector cannot derive span
    /// signals for the supplied observation.
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError>;
}

/// Open projector contract from typed observations into metric records.
pub trait MetricProjector<T>: Send + Sync
where
    T: Observable,
{
    /// Projects one observation into zero or more metric records.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionError`] when the projector cannot derive metric
    /// output for the supplied observation.
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError>;
}

/// Construction-time registration for one typed observation subscriber.
#[derive(Clone)]
#[expect(
    missing_debug_implementations,
    reason = "public registration wrappers intentionally store trait objects that do not have a stable or useful Debug surface"
)]
pub struct SubscriberRegistration<T>
where
    T: Observable,
{
    /// Registered subscriber implementation.
    subscriber: Arc<dyn ObservationSubscriber<T>>,
    /// Optional filter evaluated before subscriber execution.
    filter: Option<Arc<dyn ObservationFilter<T>>>,
}

impl<T> SubscriberRegistration<T>
where
    T: Observable,
{
    /// Creates a subscriber registration with no filter.
    #[must_use]
    pub fn new(subscriber: Arc<dyn ObservationSubscriber<T>>) -> Self {
        Self {
            subscriber,
            filter: None,
        }
    }

    /// Attaches a filter evaluated before subscriber execution.
    #[must_use]
    pub fn with_filter(mut self, filter: Arc<dyn ObservationFilter<T>>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Splits the registration into its subscriber and optional filter.
    #[must_use]
    pub fn into_parts(self) -> SubscriberRegistrationParts<T> {
        (self.subscriber, self.filter)
    }
}

/// Construction-time registration for log/span/metric projection of a payload.
#[derive(Clone)]
#[expect(
    missing_debug_implementations,
    reason = "public registration wrappers intentionally store trait objects that do not have a stable or useful Debug surface"
)]
pub struct ProjectionRegistration<T>
where
    T: Observable,
{
    /// Optional log projector.
    log_projector: Option<Arc<dyn LogProjector<T>>>,
    /// Optional span projector.
    span_projector: Option<Arc<dyn SpanProjector<T>>>,
    /// Optional metric projector.
    metric_projector: Option<Arc<dyn MetricProjector<T>>>,
    /// Optional filter evaluated before projection.
    filter: Option<Arc<dyn ObservationFilter<T>>>,
}

impl<T> ProjectionRegistration<T>
where
    T: Observable,
{
    /// Creates an empty projection registration ready for projector attachment.
    #[must_use]
    pub fn new() -> Self {
        Self {
            log_projector: None,
            span_projector: None,
            metric_projector: None,
            filter: None,
        }
    }

    /// Attaches a log projector.
    #[must_use]
    pub fn with_log_projector(mut self, projector: Arc<dyn LogProjector<T>>) -> Self {
        self.log_projector = Some(projector);
        self
    }

    /// Attaches a span projector.
    #[must_use]
    pub fn with_span_projector(mut self, projector: Arc<dyn SpanProjector<T>>) -> Self {
        self.span_projector = Some(projector);
        self
    }

    /// Attaches a metric projector.
    #[must_use]
    pub fn with_metric_projector(mut self, projector: Arc<dyn MetricProjector<T>>) -> Self {
        self.metric_projector = Some(projector);
        self
    }

    /// Attaches a filter evaluated before projection.
    #[must_use]
    pub fn with_filter(mut self, filter: Arc<dyn ObservationFilter<T>>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Splits the registration into its projector components and optional filter.
    #[must_use]
    pub fn into_parts(self) -> ProjectionRegistrationParts<T> {
        (
            self.log_projector,
            self.span_projector,
            self.metric_projector,
            self.filter,
        )
    }
}

impl<T> Default for ProjectionRegistration<T>
where
    T: Observable,
{
    fn default() -> Self {
        Self::new()
    }
}

// Conversions between the released root family and the canonical `v2` family.
// Each adapter moves the original boxed context between the two error shapes,
// so code, message, remediation, source and backtrace are never rebuilt.
// Span and metric models convert field by field; a value the target family
// cannot represent fails with a projection error instead of being dropped or
// approximated.

struct ReleasedSubscriber<T: Observable>(Arc<dyn ObservationSubscriber<T>>);

impl<T: Observable> canonical::ObservationSubscriber<T> for ReleasedSubscriber<T> {
    fn observe(&self, observation: &Observation<T>) -> Result<(), CanonicalSubscriberError> {
        self.0
            .observe(observation)
            .map_err(|error| CanonicalSubscriberError::Subscriber { context: error.0 })
    }
}

struct CanonicalSubscriber<T: Observable>(Arc<dyn canonical::ObservationSubscriber<T>>);

impl<T: Observable> ObservationSubscriber<T> for CanonicalSubscriber<T> {
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberError> {
        self.0
            .observe(observation)
            .map_err(|error| SubscriberError(error.into_context()))
    }
}

struct ReleasedLogProjector<T: Observable>(Arc<dyn LogProjector<T>>);

impl<T: Observable> canonical::LogProjector<T> for ReleasedLogProjector<T> {
    fn project_logs(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<LogEvent>, CanonicalProjectionError> {
        self.0
            .project_logs(observation)
            .map_err(|error| CanonicalProjectionError::Projection { context: error.0 })
    }
}

struct CanonicalLogProjector<T: Observable>(Arc<dyn canonical::LogProjector<T>>);

impl<T: Observable> LogProjector<T> for CanonicalLogProjector<T> {
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError> {
        self.0
            .project_logs(observation)
            .map_err(|error| ProjectionError(error.into_context()))
    }
}

struct ReleasedSpanProjector<T: Observable>(Arc<dyn SpanProjector<T>>);

impl<T: Observable> canonical::SpanProjector<T> for ReleasedSpanProjector<T> {
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<model::SpanSignal>, CanonicalProjectionError> {
        self.0
            .project_spans(observation)
            .map_err(|error| CanonicalProjectionError::Projection { context: error.0 })?
            .into_iter()
            .map(canonical_span)
            .collect()
    }
}

struct CanonicalSpanProjector<T: Observable>(Arc<dyn canonical::SpanProjector<T>>);

impl<T: Observable> SpanProjector<T> for CanonicalSpanProjector<T> {
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        self.0
            .project_spans(observation)
            .map_err(|error| ProjectionError(error.into_context()))?
            .into_iter()
            .map(released_span)
            .collect()
    }
}

struct ReleasedMetricProjector<T: Observable>(Arc<dyn MetricProjector<T>>);

impl<T: Observable> canonical::MetricProjector<T> for ReleasedMetricProjector<T> {
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<model::MetricRecord>, CanonicalProjectionError> {
        self.0
            .project_metrics(observation)
            .map_err(|error| CanonicalProjectionError::Projection { context: error.0 })?
            .into_iter()
            .map(canonical_metric)
            .collect()
    }
}

struct CanonicalMetricProjector<T: Observable>(Arc<dyn canonical::MetricProjector<T>>);

impl<T: Observable> MetricProjector<T> for CanonicalMetricProjector<T> {
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError> {
        self.0
            .project_metrics(observation)
            .map_err(|error| ProjectionError(error.into_context()))?
            .into_iter()
            .map(|metric| released_metric(&metric))
            .collect()
    }
}

impl<T: Observable> From<SubscriberRegistration<T>> for canonical::SubscriberRegistration<T> {
    fn from(registration: SubscriberRegistration<T>) -> Self {
        let (subscriber, filter) = registration.into_parts();
        let converted = Self::new(Arc::new(ReleasedSubscriber(subscriber)));
        match filter {
            Some(filter) => converted.with_filter(filter),
            None => converted,
        }
    }
}

impl<T: Observable> From<canonical::SubscriberRegistration<T>> for SubscriberRegistration<T> {
    fn from(registration: canonical::SubscriberRegistration<T>) -> Self {
        let (subscriber, filter) = registration.into_parts();
        let converted = Self::new(Arc::new(CanonicalSubscriber(subscriber)));
        match filter {
            Some(filter) => converted.with_filter(filter),
            None => converted,
        }
    }
}

impl<T: Observable> From<ProjectionRegistration<T>> for canonical::ProjectionRegistration<T> {
    fn from(registration: ProjectionRegistration<T>) -> Self {
        let (log, span, metric, filter) = registration.into_parts();
        let mut converted = Self::new();
        if let Some(projector) = log {
            converted = converted.with_log_projector(Arc::new(ReleasedLogProjector(projector)));
        }
        if let Some(projector) = span {
            converted = converted.with_span_projector(Arc::new(ReleasedSpanProjector(projector)));
        }
        if let Some(projector) = metric {
            converted =
                converted.with_metric_projector(Arc::new(ReleasedMetricProjector(projector)));
        }
        if let Some(filter) = filter {
            converted = converted.with_filter(filter);
        }
        converted
    }
}

impl<T: Observable> From<canonical::ProjectionRegistration<T>> for ProjectionRegistration<T> {
    fn from(registration: canonical::ProjectionRegistration<T>) -> Self {
        let (log, span, metric, filter) = registration.into_parts();
        let mut converted = Self::new();
        if let Some(projector) = log {
            converted = converted.with_log_projector(Arc::new(CanonicalLogProjector(projector)));
        }
        if let Some(projector) = span {
            converted = converted.with_span_projector(Arc::new(CanonicalSpanProjector(projector)));
        }
        if let Some(projector) = metric {
            converted =
                converted.with_metric_projector(Arc::new(CanonicalMetricProjector(projector)));
        }
        if let Some(filter) = filter {
            converted = converted.with_filter(filter);
        }
        converted
    }
}

// Released root models into canonical models. Root spans carry no flags,
// kind or links, so they map to the canonical defaults for those fields.

fn canonical_span(signal: SpanSignal) -> Result<model::SpanSignal, CanonicalProjectionError> {
    Ok(match signal {
        SpanSignal::Started(record) => model::SpanSignal::Started(canonical_started(&record)?),
        SpanSignal::Event(event) => model::SpanSignal::Event(model::SpanEvent {
            timestamp: event.timestamp,
            trace: canonical_trace(&event.trace),
            attributes: canonical_attributes(&event.attributes)?,
            name: event.name,
            diagnostic: event.diagnostic,
        }),
        SpanSignal::Ended(record) => {
            let duration = record.duration_ms().ok_or_else(|| {
                canonical_unrepresentable(conversion_context(
                    "released completed span has no duration",
                    "end released spans through SpanRecord::end",
                ))
            })?;
            model::SpanSignal::Ended(canonical_started(&record)?.end(record.status(), duration))
        }
    })
}

fn canonical_started<S>(
    record: &SpanRecord<S>,
) -> Result<model::SpanRecord<SpanStarted>, CanonicalProjectionError> {
    let started = model::SpanRecord::new(
        record.timestamp(),
        record.service().clone(),
        record.name().clone(),
        canonical_trace(record.trace()),
        canonical_attributes(record.attributes())?,
    );
    Ok(match record.diagnostic() {
        Some(diagnostic) => started.with_diagnostic(diagnostic.clone()),
        None => started,
    })
}

fn canonical_trace(trace: &TraceContext) -> model::TraceContext {
    let context = model::TraceContext::new(
        trace.trace_id.clone(),
        trace.span_id.clone(),
        model::TraceFlags::default(),
    );
    match trace.parent_span_id.clone() {
        Some(parent) => context.with_parent(parent),
        None => context,
    }
}

fn canonical_metric(metric: MetricRecord) -> Result<model::MetricRecord, CanonicalProjectionError> {
    let finite = |value: f64| {
        model::FiniteF64::new(value).map_err(|error| {
            canonical_unrepresentable(
                conversion_context(
                    "released metric value is not finite",
                    "project finite metric values",
                )
                .source(Box::new(error)),
            )
        })
    };
    let value = match metric.kind {
        MetricKind::Gauge => model::MetricValue::Gauge(finite(metric.value)?),
        MetricKind::Counter => model::MetricValue::Sum {
            value: finite(metric.value)?,
            monotonic: true,
            temporality: model::AggregationTemporality::Cumulative,
            start_time: metric.timestamp,
        },
        MetricKind::Histogram => {
            return Err(canonical_unrepresentable(conversion_context(
                "released scalar histogram has no canonical bucket distribution",
                "project a canonical HistogramPoint through the v2 MetricProjector",
            )));
        }
    };
    let attributes = canonical_attributes(&metric.attributes)?;
    model::MetricRecord::try_new(metric.timestamp, metric.service, metric.name, value)
        .map(|record| record.with_unit(metric.unit).with_attributes(attributes))
        .map_err(|error| {
            canonical_unrepresentable(
                conversion_context(
                    "released metric violates the canonical metric contract",
                    "project a metric value valid for its canonical aggregation",
                )
                .source(Box::new(error)),
            )
        })
}

fn canonical_attributes(
    values: &Map<String, Value>,
) -> Result<model::Attributes, CanonicalProjectionError> {
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), canonical_attribute(value)?)))
        .collect()
}

/// Converts one released attribute value; a number with no integer or finite
/// float representation, possible when a consumer enables `serde_json`
/// `arbitrary_precision`, is rejected rather than replaced.
fn canonical_attribute(value: &Value) -> Result<model::AttributeValue, CanonicalProjectionError> {
    Ok(match value {
        Value::Null => model::AttributeValue::Null,
        Value::Bool(value) => model::AttributeValue::Bool(*value),
        Value::Number(value) => value
            .as_i64()
            .map(model::AttributeValue::Int)
            .or_else(|| value.as_u64().map(model::AttributeValue::UInt))
            .or_else(|| {
                value
                    .as_f64()
                    .and_then(|number| model::FiniteF64::new(number).ok())
                    .map(model::AttributeValue::Float)
            })
            .ok_or_else(|| {
                canonical_unrepresentable(conversion_context(
                    "released attribute number has no canonical representation",
                    "project attribute numbers that fit an integer or a finite float",
                ))
            })?,
        Value::String(value) => model::AttributeValue::String(value.clone()),
        Value::Array(values) => model::AttributeValue::Array(
            values
                .iter()
                .map(canonical_attribute)
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(values) => model::AttributeValue::Object(canonical_attributes(values)?),
    })
}

// Canonical models into released root models. Released spans have no flags,
// kind or links and released metrics have no aggregation interval or bucket
// distribution; any such value is rejected.

fn released_span(signal: model::SpanSignal) -> Result<SpanSignal, ProjectionError> {
    Ok(match signal {
        model::SpanSignal::Started(record) => SpanSignal::Started(released_started(&record)?),
        model::SpanSignal::Event(event) => SpanSignal::Event(SpanEvent {
            timestamp: event.timestamp,
            trace: released_trace(&event.trace)?,
            name: event.name,
            attributes: released_attributes(&event.attributes),
            diagnostic: event.diagnostic,
        }),
        model::SpanSignal::Ended(record) => {
            SpanSignal::Ended(released_started(&record)?.end(record.status(), record.duration_ms()))
        }
    })
}

fn released_started<S: model::SpanState>(
    record: &model::SpanRecord<S>,
) -> Result<SpanRecord<SpanStarted>, ProjectionError> {
    if record.kind() != model::SpanKind::Internal {
        return Err(released_unrepresentable(conversion_context(
            "canonical span kind has no released representation",
            "register a v2 SpanProjector to keep the span kind",
        )));
    }
    if !record.links().is_empty() {
        return Err(released_unrepresentable(conversion_context(
            "canonical span links have no released representation",
            "register a v2 SpanProjector to keep span links",
        )));
    }
    let started = SpanRecord::new(
        record.timestamp(),
        record.service().clone(),
        record.name().clone(),
        released_trace(record.trace())?,
        released_attributes(record.attributes()),
    );
    Ok(match record.diagnostic() {
        Some(diagnostic) => started.with_diagnostic(diagnostic.clone()),
        None => started,
    })
}

fn released_trace(trace: &model::TraceContext) -> Result<TraceContext, ProjectionError> {
    if trace.flags != model::TraceFlags::default() {
        return Err(released_unrepresentable(conversion_context(
            "canonical trace flags have no released representation",
            "register a v2 SpanProjector to keep trace flags",
        )));
    }
    Ok(TraceContext {
        trace_id: trace.trace_id.clone(),
        span_id: trace.span_id.clone(),
        parent_span_id: trace.parent_span_id.clone(),
    })
}

fn released_metric(metric: &model::MetricRecord) -> Result<MetricRecord, ProjectionError> {
    let (kind, value) = match metric.value() {
        model::MetricValue::Gauge(value) => (MetricKind::Gauge, value.get()),
        model::MetricValue::Sum {
            value,
            monotonic: true,
            temporality: model::AggregationTemporality::Cumulative,
            start_time,
        } if *start_time == metric.timestamp() => (MetricKind::Counter, value.get()),
        model::MetricValue::Sum { .. } => {
            return Err(released_unrepresentable(conversion_context(
                "canonical sum interval has no released representation",
                "register a v2 MetricProjector to keep the sum interval and temporality",
            )));
        }
        model::MetricValue::Histogram { .. } => {
            return Err(released_unrepresentable(conversion_context(
                "canonical histogram buckets have no released representation",
                "register a v2 MetricProjector to keep the histogram distribution",
            )));
        }
    };
    Ok(MetricRecord {
        timestamp: metric.timestamp(),
        service: metric.service().clone(),
        name: metric.name().clone(),
        kind,
        value,
        unit: metric.unit().cloned(),
        attributes: released_attributes(metric.attributes()),
    })
}

fn released_attributes(values: &model::Attributes) -> Map<String, Value> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), released_attribute(value)))
        .collect()
}

fn released_attribute(value: &model::AttributeValue) -> Value {
    match value {
        model::AttributeValue::Bool(value) => Value::Bool(*value),
        model::AttributeValue::Int(value) => Value::from(*value),
        model::AttributeValue::UInt(value) => Value::from(*value),
        model::AttributeValue::Float(value) => Value::from(value.get()),
        model::AttributeValue::String(value) => Value::String(value.clone()),
        model::AttributeValue::Array(values) => {
            Value::Array(values.iter().map(released_attribute).collect())
        }
        model::AttributeValue::Object(values) => Value::Object(released_attributes(values)),
        model::AttributeValue::Null => Value::Null,
    }
}

fn conversion_context(message: &str, recovery: &str) -> ErrorContext {
    ErrorContext::new(
        error_codes::VALUE_VALIDATION_FAILED,
        message,
        Remediation::not_recoverable(recovery),
    )
}

fn canonical_unrepresentable(context: ErrorContext) -> CanonicalProjectionError {
    CanonicalProjectionError::Projection {
        context: Box::new(context),
    }
}

fn released_unrepresentable(context: ErrorContext) -> ProjectionError {
    ProjectionError(Box::new(context))
}
