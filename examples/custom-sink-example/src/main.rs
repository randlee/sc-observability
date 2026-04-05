use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sc_observability::{
    LogFilter, LogSink, Logger, LoggerConfig, SinkHealth, SinkHealthState, SinkRegistration,
};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, ErrorContext, Level, LogEvent, LogSinkError,
    ProcessIdentity, Remediation, ServiceName, TargetCategory, Timestamp,
};
use serde_json::json;

struct AuditSink {
    health: Mutex<SinkHealth>,
}

impl AuditSink {
    fn new() -> Self {
        Self {
            health: Mutex::new(SinkHealth {
                name: "audit-stderr".to_string(),
                state: SinkHealthState::Healthy,
                last_error: None,
            }),
        }
    }

    fn mark_failure<E>(&self, error: E) -> LogSinkError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let message = error.to_string();
        let context = ErrorContext::new(
            sc_observability::error_codes::LOGGER_SINK_WRITE_FAILED,
            "custom sink write failed",
            Remediation::recoverable(
                "inspect stderr output permissions and retry the custom sink write",
                ["retry the write"],
            ),
        )
        .cause(message);

        let mut health = self.health.lock().expect("custom sink health poisoned");
        health.state = SinkHealthState::DegradedDropping;
        health.last_error = Some(sc_observability_types::DiagnosticSummary::from(
            context.diagnostic(),
        ));
        LogSinkError(Box::new(context))
    }
}

impl LogSink for AuditSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        let mut stderr = io::stderr().lock();
        writeln!(
            stderr,
            "AUDIT {} {} {}",
            event.target.as_str(),
            event.action.as_str(),
            event.message.as_deref().unwrap_or("<no-message>")
        )
        .map_err(|err| self.mark_failure(err))?;

        let mut health = self.health.lock().expect("custom sink health poisoned");
        health.state = SinkHealthState::Healthy;
        health.last_error = None;
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        self.health
            .lock()
            .expect("custom sink health poisoned")
            .clone()
    }
}

struct AuditOnly;

impl LogFilter for AuditOnly {
    fn accepts(&self, event: &LogEvent) -> bool {
        event.target.as_str() == "app.audit"
    }
}

fn build_event(service: ServiceName, target: &str, action: &str, message: &str) -> LogEvent {
    LogEvent {
        version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
        timestamp: Timestamp::now_utc(),
        level: Level::Info,
        service,
        target: TargetCategory::new(target).expect("valid target"),
        action: ActionName::new(action).expect("valid action"),
        message: Some(message.to_string()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: Some("ok".to_string()),
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::now_utc(),
            code: ErrorCode::new_static("SC_CUSTOM_SINK_EXAMPLE"),
            message: "custom sink example event".to_string(),
            cause: None,
            remediation: Remediation::recoverable("retry", ["inspect stderr output"]),
            docs: None,
            details: serde_json::Map::from_iter([("example".to_string(), json!(true))]),
        }),
        state_transition: None,
        fields: serde_json::Map::from_iter([("component".to_string(), json!("example"))]),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ServiceName::new("custom-sink-example")?;
    let root = std::env::temp_dir().join("sc-observability-custom-sink-example");
    let mut logger = Logger::new(LoggerConfig::default_for(service.clone(), PathBuf::from(root)))?;

    logger.register_sink(
        SinkRegistration::new(Arc::new(AuditSink::new())).with_filter(Arc::new(AuditOnly)),
    );

    logger.emit(build_event(
        service.clone(),
        "app.audit",
        "startup",
        "accepted by the custom sink",
    ))?;
    logger.emit(build_event(
        service,
        "app.core",
        "heartbeat",
        "written only to the built-in file sink",
    ))?;
    logger.flush()?;

    let health = logger.health();
    println!("logging state: {:?}", health.state);
    println!("active log path: {}", health.active_log_path.display());
    for sink in &health.sink_statuses {
        println!("sink {} => {:?}", sink.name, sink.state);
    }

    Ok(())
}
