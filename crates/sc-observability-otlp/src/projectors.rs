//! Telemetry projector adapters layered on top of generic observation routing.
//!
//! `TelemetryProjectors<T>` wraps ordinary `sc-observe` projectors for one
//! `Observable` payload type and forwards projected logs, spans, and metrics
//! into a shared `Telemetry` runtime without changing downstream registration
//! paths.
#![expect(
    clippy::must_use_candidate,
    reason = "projection-helper builders are intentionally kept lightweight and explicit without repetitive must_use decoration"
)]
#![expect(
    clippy::return_self_not_must_use,
    reason = "builder-style chaining is explicit from the signatures and intentionally lightweight"
)]

use std::sync::Arc;

use crate::{Telemetry, error_codes};
use sc_observability_types::typed::{
    ProjectionFailure, TypedLogProjector, TypedMetricProjector, TypedSpanProjector,
    typed_log_projector, typed_metric_projector, typed_span_projector,
};
use sc_observability_types::{
    ErrorContext, LogEvent, LogProjector, MetricProjector, MetricRecord, Observable, Observation,
    ObservationFilter, ProjectionError, ProjectionRegistration, Remediation, SpanProjector,
    SpanSignal,
};

/// Public helper for attaching telemetry export to ordinary observation projection registration.
#[expect(
    missing_debug_implementations,
    reason = "the helper stores trait-object projectors and filters whose internal state is not part of the public debug contract"
)]
pub struct TelemetryProjectors<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    log_projector: Option<Arc<dyn LogProjector<T>>>,
    span_projector: Option<Arc<dyn SpanProjector<T>>>,
    metric_projector: Option<Arc<dyn MetricProjector<T>>>,
    filter: Option<Arc<dyn ObservationFilter<T>>>,
}

impl<T> TelemetryProjectors<T>
where
    T: Observable,
{
    /// Starts a wrapped projector set for one observation payload type.
    pub fn new(telemetry: Arc<Telemetry>) -> Self {
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

    /// Attaches the same observation filter the wrapped projector registration should honor.
    pub fn with_filter(mut self, filter: Arc<dyn ObservationFilter<T>>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Converts the wrapped helper into ordinary sc-observe projection registration.
    pub fn into_registration(self) -> ProjectionRegistration<T> {
        let mut registration = ProjectionRegistration::new();

        if let Some(inner) = self.log_projector {
            registration = registration.with_log_projector(Arc::new(AttachedLogProjector {
                telemetry: self.telemetry.clone(),
                inner: typed_log_projector(inner),
            })
                as Arc<dyn LogProjector<T>>);
        }

        if let Some(inner) = self.span_projector {
            registration = registration.with_span_projector(Arc::new(AttachedSpanProjector {
                telemetry: self.telemetry.clone(),
                inner: typed_span_projector(inner),
            })
                as Arc<dyn SpanProjector<T>>);
        }

        if let Some(inner) = self.metric_projector {
            registration = registration.with_metric_projector(Arc::new(AttachedMetricProjector {
                telemetry: self.telemetry,
                inner: typed_metric_projector(inner),
            })
                as Arc<dyn MetricProjector<T>>);
        }

        if let Some(filter) = self.filter {
            registration = registration.with_filter(filter);
        }

        registration
    }
}

struct AttachedLogProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn TypedLogProjector<T>>,
}

impl<T> TypedLogProjector<T> for AttachedLogProjector<T>
where
    T: Observable,
{
    fn project_logs(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<LogEvent>, ProjectionFailure> {
        let events = self.inner.project_logs(observation)?;
        for event in &events {
            self.telemetry
                .emit_log(event)
                .map_err(telemetry_to_projection_failure)?;
        }
        Ok(events)
    }
}

impl<T> LogProjector<T> for AttachedLogProjector<T>
where
    T: Observable,
{
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionError> {
        <Self as TypedLogProjector<T>>::project_logs(self, observation).map_err(Into::into)
    }
}

struct AttachedSpanProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn TypedSpanProjector<T>>,
}

impl<T> TypedSpanProjector<T> for AttachedSpanProjector<T>
where
    T: Observable,
{
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionFailure> {
        let spans = self.inner.project_spans(observation)?;
        for span in &spans {
            self.telemetry
                .emit_span(span)
                .map_err(telemetry_to_projection_failure)?;
        }
        Ok(spans)
    }
}

impl<T> SpanProjector<T> for AttachedSpanProjector<T>
where
    T: Observable,
{
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        <Self as TypedSpanProjector<T>>::project_spans(self, observation).map_err(Into::into)
    }
}

struct AttachedMetricProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn TypedMetricProjector<T>>,
}

impl<T> TypedMetricProjector<T> for AttachedMetricProjector<T>
where
    T: Observable,
{
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionFailure> {
        let metrics = self.inner.project_metrics(observation)?;
        for metric in &metrics {
            self.telemetry
                .emit_metric(metric)
                .map_err(telemetry_to_projection_failure)?;
        }
        Ok(metrics)
    }
}

impl<T> MetricProjector<T> for AttachedMetricProjector<T>
where
    T: Observable,
{
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError> {
        <Self as TypedMetricProjector<T>>::project_metrics(self, observation).map_err(Into::into)
    }
}

fn telemetry_to_projection_failure(
    error: sc_observability_types::TelemetryError,
) -> ProjectionFailure {
    match error {
        sc_observability_types::TelemetryError::Shutdown => {
            ProjectionFailure::from_context(Box::new(ErrorContext::new(
                error_codes::TELEMETRY_EXPORT_FAILED,
                "telemetry runtime is shut down",
                Remediation::not_recoverable("do not project telemetry after shutdown"),
            )))
        }
        sc_observability_types::TelemetryError::ExportFailure(context) => {
            ProjectionFailure::from_context(context)
        }
    }
}
