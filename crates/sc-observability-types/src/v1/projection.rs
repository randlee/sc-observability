#![allow(
    deprecated,
    reason = "the v1 compatibility module intentionally refers to its deprecated extension points"
)]

use std::sync::Arc;

use crate::errors_v2::{ProjectionError, SubscriberError};
use crate::v2::ObservationFilter;
use crate::{LogEvent, Observable, Observation, SpanSignal};

use super::MetricRecord;

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

/// Released 1.x subscriber contract.
#[deprecated(note = "use sc_observability_types::v2::ObservationSubscriber")]
pub trait ObservationSubscriber<T>: Send + Sync
where
    T: Observable,
{
    /// Consumes one routed observation.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriberError`] when the subscriber rejects the observation.
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberError>;
}

/// Released 1.x projector contract for log events.
#[deprecated(note = "use sc_observability_types::v2::LogProjector")]
pub trait LogProjector<T>: Send + Sync
where
    T: Observable,
{
    /// Projects one observation into zero or more log events.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionError`] when projection fails.
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError>;
}

/// Released 1.x projector contract for span signals.
#[deprecated(note = "use sc_observability_types::v2::SpanProjector")]
pub trait SpanProjector<T>: Send + Sync
where
    T: Observable,
{
    /// Projects one observation into zero or more span lifecycle signals.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionError`] when projection fails.
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError>;
}

/// Released 1.x projector contract for metric records.
#[deprecated(note = "use sc_observability_types::v2::MetricProjector")]
pub trait MetricProjector<T>: Send + Sync
where
    T: Observable,
{
    /// Projects one observation into zero or more metric records.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionError`] when projection fails.
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError>;
}

/// Released 1.x registration for one typed observation subscriber.
#[derive(Clone)]
#[expect(
    missing_debug_implementations,
    reason = "public registration wrappers intentionally store trait objects that do not have a stable or useful Debug surface"
)]
#[deprecated(note = "use sc_observability_types::v2::SubscriberRegistration")]
pub struct SubscriberRegistration<T>
where
    T: Observable,
{
    subscriber: Arc<dyn ObservationSubscriber<T>>,
    filter: Option<Arc<dyn ObservationFilter<T>>>,
}

#[allow(
    deprecated,
    reason = "v1 registration stores its v1 subscriber contract"
)]
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

    /// Attaches a canonical filter evaluated before subscriber execution.
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

/// Released 1.x registration for log, span, and metric projection.
#[derive(Clone)]
#[expect(
    missing_debug_implementations,
    reason = "public registration wrappers intentionally store trait objects that do not have a stable or useful Debug surface"
)]
#[deprecated(note = "use sc_observability_types::v2::ProjectionRegistration")]
pub struct ProjectionRegistration<T>
where
    T: Observable,
{
    log_projector: Option<Arc<dyn LogProjector<T>>>,
    span_projector: Option<Arc<dyn SpanProjector<T>>>,
    metric_projector: Option<Arc<dyn MetricProjector<T>>>,
    filter: Option<Arc<dyn ObservationFilter<T>>>,
}

#[allow(
    deprecated,
    reason = "v1 registration stores its v1 projector contracts"
)]
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

    /// Attaches a canonical filter evaluated before projection.
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

#[allow(
    deprecated,
    reason = "the default creates the deprecated v1 registration"
)]
impl<T> Default for ProjectionRegistration<T>
where
    T: Observable,
{
    fn default() -> Self {
        Self::new()
    }
}
