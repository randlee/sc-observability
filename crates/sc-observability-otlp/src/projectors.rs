//! Telemetry projector adapters layered on top of generic observation routing.
//!
//! `V2TelemetryProjectors<T>` wraps canonical `sc-observe` projectors for one
//! `Observable` payload type and forwards their canonical logs, spans, and
//! metrics into a shared `RuntimeTelemetry` without any released-model round
//! trip.
#![allow(
    clippy::must_use_candidate,
    reason = "projection-helper builders are intentionally kept lightweight and explicit without repetitive must_use decoration"
)]
#![expect(
    clippy::return_self_not_must_use,
    reason = "builder-style chaining is explicit from the signatures and intentionally lightweight"
)]
use std::sync::Arc;

use crate::RuntimeTelemetry;
use sc_observability_types::v2::TelemetryError as CanonicalTelemetryError;
use sc_observability_types::v2::{
    LogProjector, MetricProjector, MetricRecord as V2MetricRecord, ObservationFilter,
    ProjectionError, ProjectionRegistration, SpanProjector, SpanSignal as V2SpanSignal,
};
use sc_observability_types::{LogEvent, Observable, Observation};

/// Wrap canonical observation projectors for the canonical OTLP telemetry API.
///
/// Attach log, span, and metric projectors and an optional filter, then call
/// `into_registration` to register them with the observation routing layer.
/// Projected canonical outputs are also forwarded, unchanged, to the supplied
/// v2 telemetry runtime.
#[expect(
    missing_debug_implementations,
    reason = "the helper stores trait-object projectors and filters whose internal state is not part of the public debug contract"
)]
pub struct V2TelemetryProjectors<T>
where
    T: Observable,
{
    telemetry: Arc<RuntimeTelemetry>,
    log_projector: Option<Arc<dyn LogProjector<T>>>,
    span_projector: Option<Arc<dyn SpanProjector<T>>>,
    metric_projector: Option<Arc<dyn MetricProjector<T>>>,
    filter: Option<Arc<dyn ObservationFilter<T>>>,
}

impl<T> V2TelemetryProjectors<T>
where
    T: Observable,
{
    /// Starts a wrapped projector set for one observation payload type.
    pub fn new(telemetry: Arc<RuntimeTelemetry>) -> Self {
        Self {
            telemetry,
            log_projector: None,
            span_projector: None,
            metric_projector: None,
            filter: None,
        }
    }

    /// Attaches a log projector whose output is also forwarded into telemetry.
    pub fn with_log_projector(mut self, projector: Arc<dyn LogProjector<T>>) -> Self {
        self.log_projector = Some(projector);
        self
    }

    /// Attaches a span projector whose output is also forwarded into telemetry.
    pub fn with_span_projector(mut self, projector: Arc<dyn SpanProjector<T>>) -> Self {
        self.span_projector = Some(projector);
        self
    }

    /// Attaches a metric projector whose output is also forwarded into telemetry.
    pub fn with_metric_projector(mut self, projector: Arc<dyn MetricProjector<T>>) -> Self {
        self.metric_projector = Some(projector);
        self
    }

    /// Attaches the filter the wrapped projector registration should honor.
    pub fn with_filter(mut self, filter: Arc<dyn ObservationFilter<T>>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Converts the wrapped helper into ordinary observation registration.
    pub fn into_registration(self) -> ProjectionRegistration<T> {
        let mut registration = ProjectionRegistration::new();
        if let Some(inner) = self.log_projector {
            registration = registration.with_log_projector(Arc::new(CanonicalAttached {
                telemetry: Arc::clone(&self.telemetry),
                inner,
            }));
        }
        if let Some(inner) = self.span_projector {
            registration = registration.with_span_projector(Arc::new(CanonicalAttached {
                telemetry: Arc::clone(&self.telemetry),
                inner,
            }));
        }
        if let Some(inner) = self.metric_projector {
            registration = registration.with_metric_projector(Arc::new(CanonicalAttached {
                telemetry: Arc::clone(&self.telemetry),
                inner,
            }));
        }
        if let Some(filter) = self.filter {
            registration = registration.with_filter(filter);
        }
        registration
    }
}

/// Canonical projector whose output is also admitted by the canonical runtime.
struct CanonicalAttached<P: ?Sized> {
    telemetry: Arc<RuntimeTelemetry>,
    inner: Arc<P>,
}

impl<T: Observable> LogProjector<T> for CanonicalAttached<dyn LogProjector<T>> {
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError> {
        let events = self.inner.project_logs(observation)?;
        for event in &events {
            self.telemetry
                .emit_log(event)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(events)
    }
}

impl<T: Observable> SpanProjector<T> for CanonicalAttached<dyn SpanProjector<T>> {
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<V2SpanSignal>, ProjectionError> {
        let spans = self.inner.project_spans(observation)?;
        for span in &spans {
            self.telemetry
                .emit_span(span)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(spans)
    }
}

impl<T: Observable> MetricProjector<T> for CanonicalAttached<dyn MetricProjector<T>> {
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<V2MetricRecord>, ProjectionError> {
        let metrics = self.inner.project_metrics(observation)?;
        for metric in &metrics {
            self.telemetry
                .emit_metric(metric)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(metrics)
    }
}

fn telemetry_to_projection_error(error: CanonicalTelemetryError) -> ProjectionError {
    ProjectionError::Projection {
        context: error.into_context(),
    }
}
