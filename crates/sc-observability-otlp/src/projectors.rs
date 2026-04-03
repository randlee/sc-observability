use std::sync::Arc;

use crate::{Telemetry, error_codes};
use sc_observability_types::{
    ErrorContext, LogEvent, LogProjector, MetricProjector, MetricRecord, Observable, Observation,
    ObservationFilter, ProjectionError, ProjectionRegistration, Remediation, SpanProjector,
    SpanSignal,
};

/// Public helper for attaching telemetry export to ordinary observation projection registration.
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
        ProjectionRegistration {
            log_projector: self.log_projector.map(|inner| {
                Arc::new(AttachedLogProjector {
                    telemetry: self.telemetry.clone(),
                    inner,
                }) as Arc<dyn LogProjector<T>>
            }),
            span_projector: self.span_projector.map(|inner| {
                Arc::new(AttachedSpanProjector {
                    telemetry: self.telemetry.clone(),
                    inner,
                }) as Arc<dyn SpanProjector<T>>
            }),
            metric_projector: self.metric_projector.map(|inner| {
                Arc::new(AttachedMetricProjector {
                    telemetry: self.telemetry,
                    inner,
                }) as Arc<dyn MetricProjector<T>>
            }),
            filter: self.filter,
        }
    }
}

struct AttachedLogProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn LogProjector<T>>,
}

impl<T> LogProjector<T> for AttachedLogProjector<T>
where
    T: Observable,
{
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

struct AttachedSpanProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn SpanProjector<T>>,
}

impl<T> SpanProjector<T> for AttachedSpanProjector<T>
where
    T: Observable,
{
    fn project_spans(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<SpanSignal>, ProjectionError> {
        let spans = self.inner.project_spans(observation)?;
        for span in &spans {
            self.telemetry
                .emit_span(span)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(spans)
    }
}

struct AttachedMetricProjector<T>
where
    T: Observable,
{
    telemetry: Arc<Telemetry>,
    inner: Arc<dyn MetricProjector<T>>,
}

impl<T> MetricProjector<T> for AttachedMetricProjector<T>
where
    T: Observable,
{
    fn project_metrics(
        &self,
        observation: &Observation<T>,
    ) -> Result<Vec<MetricRecord>, ProjectionError> {
        let metrics = self.inner.project_metrics(observation)?;
        for metric in &metrics {
            self.telemetry
                .emit_metric(metric)
                .map_err(telemetry_to_projection_error)?;
        }
        Ok(metrics)
    }
}

fn telemetry_to_projection_error(error: sc_observability_types::TelemetryError) -> ProjectionError {
    match error {
        sc_observability_types::TelemetryError::Shutdown => {
            ProjectionError(Box::new(ErrorContext::new(
                error_codes::TELEMETRY_EXPORT_FAILED,
                "telemetry runtime is shut down",
                Remediation::not_recoverable("do not project telemetry after shutdown"),
            )))
        }
        sc_observability_types::TelemetryError::ExportFailure(context) => ProjectionError(context),
    }
}
