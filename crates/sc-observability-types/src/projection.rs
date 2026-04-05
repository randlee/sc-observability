use std::sync::Arc;

use crate::{
    LogEvent, MetricRecord, Observable, Observation, ProjectionError, SpanSignal, SubscriberError,
};

/// Open subscriber contract for typed observations.
pub trait ObservationSubscriber<T>: Send + Sync
where
    T: Observable,
{
    /// Consumes one routed observation.
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
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError>;
}

/// Open projector contract from typed observations into span signals.
pub trait SpanProjector<T>: Send + Sync
where
    T: Observable,
{
    /// Projects one observation into zero or more span lifecycle signals.
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
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError>;
}

/// Construction-time registration for one typed observation subscriber.
#[derive(Clone)]
pub struct SubscriberRegistration<T>
where
    T: Observable,
{
    /// Registered subscriber implementation.
    pub subscriber: Arc<dyn ObservationSubscriber<T>>,
    /// Optional filter evaluated before subscriber execution.
    pub filter: Option<Arc<dyn ObservationFilter<T>>>,
}

/// Construction-time registration for log/span/metric projection of a payload.
#[derive(Clone)]
pub struct ProjectionRegistration<T>
where
    T: Observable,
{
    /// Optional log projector.
    pub log_projector: Option<Arc<dyn LogProjector<T>>>,
    /// Optional span projector.
    pub span_projector: Option<Arc<dyn SpanProjector<T>>>,
    /// Optional metric projector.
    pub metric_projector: Option<Arc<dyn MetricProjector<T>>>,
    /// Optional filter evaluated before projection.
    pub filter: Option<Arc<dyn ObservationFilter<T>>>,
}
