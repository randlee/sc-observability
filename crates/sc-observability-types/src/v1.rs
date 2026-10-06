//! Released 1.x compatibility implementations.

mod metric;
mod projection;
pub mod typed;

#[allow(deprecated, reason = "v1 re-exports its deprecated released items")]
pub use metric::{MetricKind, MetricRecord};
#[allow(deprecated, reason = "v1 re-exports its deprecated released items")]
pub use projection::{
    LogProjector, MetricProjector, ObservationSubscriber, ProjectionRegistration, SpanProjector,
    SubscriberRegistration,
};
