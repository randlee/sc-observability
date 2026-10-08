//! Canonical observation extension contracts.
//!
//! These traits and registrations report the canonical `v2` errors. The
//! released root traits in `projection` and `process` keep their 1.4.1
//! signatures; both families share [`ObservationFilter`].

use std::sync::Arc;

use crate::errors_v2::{IdentityError, ProjectionError, SubscriberError};
use crate::{LogEvent, Observable, Observation, ProcessIdentity};

type SubscriberRegistrationParts<T> = (
    Arc<dyn ObservationSubscriber<T>>,
    Option<Arc<dyn ObservationFilter<T>>>,
);

type ProjectionRegistrationParts<T> = (
    Option<Arc<dyn LogProjector<T>>>,
    Option<Arc<dyn ObservationFilter<T>>>,
);

/// Open resolver contract for caller-defined process identity lookup.
pub trait ProcessIdentityResolver: Send + Sync {
    /// Resolves the process identity to attach to emitted records.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError`] when the resolver cannot produce a valid
    /// process identity.
    fn resolve(&self) -> Result<ProcessIdentity, IdentityError>;
}

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

/// Construction-time registration for log projection of a payload.
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
            filter: None,
        }
    }

    /// Attaches a log projector.
    #[must_use]
    pub fn with_log_projector(mut self, projector: Arc<dyn LogProjector<T>>) -> Self {
        self.log_projector = Some(projector);
        self
    }

    /// Attaches a filter evaluated before projection.
    #[must_use]
    pub fn with_filter(mut self, filter: Arc<dyn ObservationFilter<T>>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Splits the registration into its log projector and optional filter.
    #[must_use]
    pub fn into_parts(self) -> ProjectionRegistrationParts<T> {
        (self.log_projector, self.filter)
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
