//! Released 1.x compatibility implementations.

mod projection;

pub mod typed;

#[allow(
    deprecated,
    reason = "the v1 compatibility module intentionally re-exports its deprecated released items"
)]
pub use projection::{
    LogProjector, ObservationSubscriber, ProjectionRegistration, SubscriberRegistration,
};
