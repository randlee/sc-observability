use std::sync::Arc;

use sc_observability::typed::{TypedLogSink, legacy_sink, typed_sink};
use sc_observability::{LogEvent, LogSink, SinkHealth, SinkHealthState};
use sc_observability_types::typed::{
    ClassifiedError, ProjectionFailure, TypedLogProjector, TypedMetricProjector,
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

struct FailingTypedProjector;

impl TypedLogProjector<String> for FailingTypedProjector {
    fn project_logs(
        &self,
        _observation: &Observation<String>,
    ) -> Result<Vec<LogEvent>, ProjectionFailure> {
        Err(ProjectionFailure::from_context(Box::new(ErrorContext::new(
            ErrorCode::new_static("FIXTURE_PROJECTION_FAILURE"),
            "fixture projector failure",
            Remediation::recoverable("inspect the projector", ["repair the projection path"]),
        ))))
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
    fn write(&self, _event: &LogEvent) -> Result<(), sc_observability_types::typed::LogSinkFailure> {
        Ok(())
    }

    fn flush(&self) -> Result<(), sc_observability_types::typed::LogSinkFailure> {
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
    fn write(&self, _event: &LogEvent) -> Result<(), sc_observability_types::LogSinkError> {
        Ok(())
    }

    fn flush(&self) -> Result<(), sc_observability_types::LogSinkError> {
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
    runtime.emit(observation).expect("successful mixed routing");
    runtime.shutdown_typed().expect("mixed routing shutdown");

    let failure_config = ObservabilityConfig::default_for_typed(
        ToolName::new("b1e-adapter-failure").expect("tool"),
        std::env::temp_dir().join("sc-observability-b1e-adapter-failure"),
    )
    .expect("failure config");
    let failing_registration = ProjectionRegistration::<String>::new()
        .with_log_projector(legacy_log_projector(Arc::new(FailingTypedProjector)));
    let failure_runtime = Observability::builder(failure_config)
        .register_projection(failing_registration)
        .build_typed()
        .expect("failing projector runtime");
    assert!(
        failure_runtime
            .emit(Observation::new(
                ServiceName::new("b1e-adapter-failure").expect("service"),
                "payload".to_owned(),
            ))
            .is_err(),
        "projector failure must remain observable at the routing boundary"
    );
    failure_runtime
        .shutdown_typed()
        .expect("failing projector shutdown");

    let custom = ProjectionFailure::from_context(Box::new(
        ErrorContext::new(
            ErrorCode::new_static("CUSTOM_FIXTURE_CODE"),
            "custom adapter failure",
            Remediation::recoverable("inspect the source", ["retry the adapter"]),
        )
        .source(Box::new(std::io::Error::other("fixture source"))),
    ));
    assert_eq!(custom.kind(), sc_observability_types::typed::ProjectionFailureKind::Unclassified);
    assert!(custom.context().diagnostic().message.contains("custom adapter"));
    assert!(std::error::Error::source(&custom).is_some());

    let wrong_family = ProjectionFailure::from_context(Box::new(ErrorContext::new(
        sc_observability::error_codes::LOGGER_INVALID_EVENT,
        "wrong family code",
        Remediation::not_recoverable("fixture"),
    )));
    assert_eq!(wrong_family.kind(), sc_observability_types::typed::ProjectionFailureKind::Unclassified);
}
