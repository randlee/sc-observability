//! Released 1.x compatibility implementations.

mod metric;
mod projection;
mod span;

pub mod typed;

#[allow(
    deprecated,
    reason = "the v1 compatibility module intentionally re-exports its deprecated released items"
)]
pub use metric::{MetricKind, MetricRecord};
#[allow(
    deprecated,
    reason = "the v1 compatibility module intentionally re-exports its deprecated released items"
)]
pub use projection::{
    LogProjector, MetricProjector, ObservationSubscriber, ProjectionRegistration, SpanProjector,
    SubscriberRegistration,
};
#[allow(
    deprecated,
    reason = "the v1 compatibility module intentionally re-exports its deprecated released items"
)]
pub use span::{SpanEvent, SpanRecord, SpanSignal};
