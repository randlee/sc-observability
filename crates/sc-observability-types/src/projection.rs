#![allow(
    deprecated,
    reason = "subscriber and projector registrations retain their published legacy trait signatures"
)]

use std::sync::Arc;

use crate::errors_v2::{
    ProjectionError as CanonicalProjectionError, SubscriberError as CanonicalSubscriberError,
};
use crate::observation_v2 as canonical;
use crate::{
    LogEvent, MetricRecord, Observable, Observation, ProjectionError, SpanSignal, SubscriberError,
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
    ) -> Result<Vec<SpanSignal>, CanonicalProjectionError> {
        self.0
            .project_spans(observation)
            .map_err(|error| CanonicalProjectionError::Projection { context: error.0 })
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
            .map_err(|error| ProjectionError(error.into_context()))
    }
}

struct ReleasedMetricProjector<T: Observable>(Arc<dyn MetricProjector<T>>);

impl<T: Observable> canonical::MetricProjector<T> for ReleasedMetricProjector<T> {
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, CanonicalProjectionError> {
        self.0
            .project_metrics(observation)
            .map_err(|error| CanonicalProjectionError::Projection { context: error.0 })
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
            .map_err(|error| ProjectionError(error.into_context()))
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
