use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sc_observability::typed::{TypedLogSink, legacy_sink, typed_sink};
use sc_observability::{LogEvent, LogSink, SinkHealth, SinkHealthState};
use sc_observability_types::typed::{
    ClassifiedError, ProjectionFailure, ProjectionFailureKind, TypedLogProjector, TypedMetricProjector,
    TypedObservationSubscriber, TypedProcessIdentityResolver, TypedSpanProjector,
    legacy_identity, legacy_log_projector, legacy_metric_projector, legacy_span_projector,
    legacy_subscriber, typed_identity, typed_log_projector, typed_metric_projector,
    typed_span_projector, typed_subscriber,
};
use sc_observability_types::*;
use sc_observe::{Observability, ObservabilityConfig};

struct TypedAdapters;

impl TypedProcessIdentityResolver for TypedAdapters {
    fn resolve(&self) -> Result<ProcessIdentity, sc_observability_types::typed::IdentityFailure> {
        Ok(ProcessIdentity::default())
    }
}

impl TypedObservationSubscriber<String> for TypedAdapters {
    fn observe(
        &self,
        _observation: &Observation<String>,
    ) -> Result<(), sc_observability_types::typed::SubscriberFailure> {
        Ok(())
    }
}

impl TypedLogProjector<String> for TypedAdapters {
    fn project_logs(
        &self,
        _observation: &Observation<String>,
    ) -> Result<Vec<LogEvent>, sc_observability_types::typed::ProjectionFailure> {
        Ok(Vec::new())
    }
}

impl TypedSpanProjector<String> for TypedAdapters {
    fn project_spans(
        &self,
        _observation: &Observation<String>,
    ) -> Result<Vec<SpanSignal>, sc_observability_types::typed::ProjectionFailure> {
        Ok(Vec::new())
    }
}

impl TypedMetricProjector<String> for TypedAdapters {
    fn project_metrics(
        &self,
        _observation: &Observation<String>,
    ) -> Result<Vec<MetricRecord>, sc_observability_types::typed::ProjectionFailure> {
        Ok(Vec::new())
    }
}

struct CountingTypedProjector {
    calls: Arc<AtomicUsize>,
    code: ErrorCode,
}

impl TypedLogProjector<String> for CountingTypedProjector {
    fn project_logs(
        &self,
        _observation: &Observation<String>,
    ) -> Result<Vec<LogEvent>, ProjectionFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(ProjectionFailure::from_context(Box::new(
            ErrorContext::new(
                self.code.clone(),
                "typed adapter failure",
                Remediation::recoverable("inspect the typed projector", ["repair the adapter"]),
            )
            .source(Box::new(std::io::Error::other("typed adapter source"))),
        )))
    }
}

struct CountingLegacyProjector {
    calls: Arc<AtomicUsize>,
    code: ErrorCode,
}

impl LogProjector<String> for CountingLegacyProjector {
    fn project_logs(
        &self,
        _observation: &Observation<String>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(ProjectionError(Box::new(
            ErrorContext::new(
                self.code.clone(),
                "legacy adapter failure",
                Remediation::recoverable("inspect the legacy projector", ["repair the adapter"]),
            )
            .source(Box::new(std::io::Error::other("legacy adapter source"))),
        )))
    }
}

struct LegacyAdapters;

impl ProcessIdentityResolver for LegacyAdapters {
    fn resolve(&self) -> Result<ProcessIdentity, sc_observability_types::IdentityError> {
        Ok(ProcessIdentity::default())
    }
}

impl ObservationSubscriber<String> for LegacyAdapters {
    fn observe(&self, _observation: &Observation<String>) -> Result<(), sc_observability_types::SubscriberError> {
        Ok(())
    }
}

impl LogProjector<String> for LegacyAdapters {
    fn project_logs(&self, _observation: &Observation<String>) -> Result<Vec<LogEvent>, ProjectionError> {
        Ok(Vec::new())
    }
}

impl SpanProjector<String> for LegacyAdapters {
    fn project_spans(&self, _observation: &Observation<String>) -> Result<Vec<SpanSignal>, ProjectionError> {
        Ok(Vec::new())
    }
}

impl MetricProjector<String> for LegacyAdapters {
    fn project_metrics(&self, _observation: &Observation<String>) -> Result<Vec<MetricRecord>, ProjectionError> {
        Ok(Vec::new())
    }
}

struct TypedSink;

impl TypedLogSink for TypedSink {
    fn write(&self, _event: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
        Ok(())
    }

    fn flush(&self) -> Result<(), sc_observability_types::v2::LogSinkError> {
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: sc_observability_types::SinkName::new("typed-fixture").expect("sink"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

struct LegacySink;

impl LogSink for LegacySink {
    fn write(&self, _event: &LogEvent) -> Result<(), sc_observability_types::v2::LogSinkError> {
        Ok(())
    }

    fn flush(&self) -> Result<(), sc_observability_types::v2::LogSinkError> {
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: sc_observability_types::SinkName::new("legacy-fixture").expect("sink"),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

fn event(service: ServiceName) -> LogEvent {
    LogEvent {
        version: sc_observability_types::SchemaVersion::new("v1").expect("schema"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: sc_observability_types::Level::Info,
        service,
        target: TargetCategory::new("fixture").expect("target"),
        action: ActionName::new("fixture.event").expect("action"),
        message: Some("adapter matrix".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}

pub fn run() {
    let observation = Observation::new(
        ServiceName::new("b1e-adapters").expect("service"),
        "payload".to_owned(),
    );
    let typed = Arc::new(TypedAdapters);
    let legacy = Arc::new(LegacyAdapters);

    assert!(legacy_identity(typed.clone()).resolve().is_ok());
    assert!(typed_identity(legacy.clone()).resolve().is_ok());
    assert!(legacy_subscriber::<String>(typed.clone()).observe(&observation).is_ok());
    assert!(typed_subscriber::<String>(legacy.clone()).observe(&observation).is_ok());
    assert!(legacy_log_projector::<String>(typed.clone()).project_logs(&observation).is_ok());
    assert!(typed_log_projector::<String>(legacy.clone()).project_logs(&observation).is_ok());
    assert!(legacy_span_projector::<String>(typed.clone()).project_spans(&observation).is_ok());
    assert!(typed_span_projector::<String>(legacy.clone()).project_spans(&observation).is_ok());
    assert!(legacy_metric_projector::<String>(typed.clone()).project_metrics(&observation).is_ok());
    assert!(typed_metric_projector::<String>(legacy.clone()).project_metrics(&observation).is_ok());

    let log_event = event(observation.service.clone());
    assert!(legacy_sink(Arc::new(TypedSink)).write(&log_event).is_ok());
    assert!(legacy_sink(Arc::new(TypedSink)).flush().is_ok());
    assert!(typed_sink(Arc::new(LegacySink)).write(&log_event).is_ok());
    assert!(typed_sink(Arc::new(LegacySink)).flush().is_ok());

    let config = ObservabilityConfig::default_for_typed(
        ToolName::new("b1e-adapters").expect("tool"),
        std::env::temp_dir().join("sc-observability-b1e-adapters"),
    )
    .expect("config");
    let registration = ProjectionRegistration::<String>::new()
        .with_log_projector(legacy_log_projector(typed.clone()));
    let runtime = Observability::builder(config)
        .register_subscriber(SubscriberRegistration::new(legacy_subscriber(typed.clone())))
        .register_projection(registration)
        .build_typed()
        .expect("mixed legacy registration");
    runtime
        .emit(observation.clone())
        .expect("successful mixed routing");
    runtime.shutdown_typed().expect("mixed routing shutdown");

    let typed_calls = Arc::new(AtomicUsize::new(0));
    let custom_legacy = legacy_log_projector::<String>(Arc::new(CountingTypedProjector {
        calls: typed_calls.clone(),
        code: ErrorCode::new_static("CUSTOM_FIXTURE_CODE"),
    }));
    let custom_legacy_error = custom_legacy
        .project_logs(&observation)
        .expect_err("typed-to-legacy failure must remain observable");
    assert_eq!(typed_calls.load(Ordering::SeqCst), 1);
    assert_eq!(custom_legacy_error.diagnostic().code.as_str(), "CUSTOM_FIXTURE_CODE");
    assert!(custom_legacy_error.diagnostic().message.contains("typed adapter"));
    assert!(std::error::Error::source(&custom_legacy_error).is_some());
    assert!(std::error::Error::source(&custom_legacy_error)
        .and_then(std::error::Error::source)
        .is_some());

    let legacy_calls = Arc::new(AtomicUsize::new(0));
    let wrong_family_typed = typed_log_projector::<String>(Arc::new(CountingLegacyProjector {
        calls: legacy_calls.clone(),
        code: sc_observability::error_codes::LOGGER_INVALID_EVENT,
    }));
    let wrong_family = wrong_family_typed
        .project_logs(&observation)
        .expect_err("legacy-to-typed failure must remain observable");
    assert_eq!(legacy_calls.load(Ordering::SeqCst), 1);
    assert_eq!(wrong_family.kind(), ProjectionFailureKind::Unclassified);
    assert_eq!(wrong_family.context().diagnostic().code.as_str(), "SC_OBSERVABILITY_LOGGER_INVALID_EVENT");
    assert!(wrong_family.context().diagnostic().message.contains("legacy adapter"));
    assert!(std::error::Error::source(&wrong_family).is_some());
    assert!(std::error::Error::source(&wrong_family)
        .and_then(std::error::Error::source)
        .is_some());
}
