use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sc_observability::{
    ActionName, Diagnostic, DiagnosticSummary, ErrorCode, ErrorContext, Level, LogEvent,
    LogFilter, LogSink, LogSinkError, LoggerBuilder, LoggerConfig, OutcomeLabel,
    ProcessIdentity, Remediation, SchemaVersion, ServiceName, SinkHealth, SinkHealthState,
    SinkName, SinkRegistration, TargetCategory, Timestamp, WriterState,
    OBSERVATION_ENVELOPE_VERSION,
};
use serde_json::json;

const TARGET_AUDIT: &str = "app.audit";
const TARGET_CORE: &str = "app.core";
const TARGET_HEALTH: &str = "app.health";
const ACTION_STARTUP: &str = "startup";
const ACTION_HEARTBEAT: &str = "heartbeat";
const ACTION_LOGGER_HEALTH: &str = "logger-health";
const ACTION_SINK_HEALTH: &str = "sink-health";
const ACTION_WRITER_WARNING: &str = "writer-warning";
const DIAGNOSTIC_CODE: &str = "SC_CUSTOM_SINK_EXAMPLE";
const DIAGNOSTIC_CAUSE: &str = "example startup event emitted for custom sink demonstration";
const MESSAGE_AUDIT_ACCEPTED: &str = "accepted by the custom sink";
const MESSAGE_FILE_ONLY: &str = "written only to the built-in file sink";
const MESSAGE_LOGGER_HEALTH: &str = "logger health snapshot";
const MESSAGE_QUEUE_FULL: &str = "non-blocking log admission failed";
const MESSAGE_QUEUE_DROPS: &str = "non-blocking log events were dropped";
const MESSAGE_WRITER_STATE: &str = "writer runtime is not healthy";
const MESSAGE_WRITER_ERROR: &str = "writer runtime reported an error";
const MESSAGE_SINK_HEALTH: &str = "sink health snapshot";
const OUTCOME_OK: &str = "ok";
const FIELD_COMPONENT: &str = "component";
const FIELD_ERROR: &str = "error";
const FIELD_EXAMPLE: &str = "example";
const FIELD_QUEUE_CAPACITY: &str = "queue_capacity";
const FIELD_QUEUE_DEPTH: &str = "queue_depth";
const FIELD_QUEUE_FULL_DROPS: &str = "queue_full_drops_total";
const FIELD_QUEUE_HIGH_WATER: &str = "queue_high_water_mark";
const FIELD_SINK_NAME: &str = "sink_name";
const FIELD_SINK_STATE: &str = "sink_state";
const FIELD_STATE: &str = "state";
const FIELD_WRITER_ERROR_CODE: &str = "writer_error_code";
const FIELD_WRITER_ERROR_MESSAGE: &str = "writer_error_message";
const FIELD_WRITER_STATE: &str = "writer_state";

struct AuditSink {
    health: Mutex<SinkHealth>,
}

impl AuditSink {
    fn new() -> Self {
        Self {
            health: Mutex::new(SinkHealth {
                name: SinkName::new("audit-stderr").expect("valid sink name"),
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
        health.last_error = Some(DiagnosticSummary::from(context.diagnostic()));
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
        event.target.as_str() == TARGET_AUDIT
    }
}

fn build_event(service: ServiceName, target: &str, action: &str, message: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("valid schema version"),
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
        outcome: Some(OutcomeLabel::new(OUTCOME_OK).expect("valid outcome label")),
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::now_utc(),
            code: ErrorCode::new_static(DIAGNOSTIC_CODE),
            message: "custom sink example event".to_string(),
            cause: Some(DIAGNOSTIC_CAUSE.to_string()),
            remediation: Remediation::recoverable("retry", ["inspect stderr output"]),
            docs: None,
            details: serde_json::Map::from_iter([(FIELD_EXAMPLE.to_string(), json!(true))]),
        }),
        state_transition: None,
        fields: serde_json::Map::from_iter([(FIELD_COMPONENT.to_string(), json!("example"))]),
    }
}

fn build_health_event(
    service: ServiceName,
    level: Level,
    action: &str,
    message: &str,
    fields: serde_json::Map<String, serde_json::Value>,
) -> LogEvent {
    let mut event = build_event(service, TARGET_HEALTH, action, message);
    event.level = level;
    event.diagnostic = None;
    event.fields = fields;
    event
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ServiceName::new("custom-sink-example")?;
    let root = std::env::temp_dir().join("sc-observability-custom-sink-example");
    let mut builder =
        LoggerBuilder::new(LoggerConfig::default_for(service.clone(), PathBuf::from(root)))?;

    builder.register_sink(
        SinkRegistration::new(Arc::new(AuditSink::new())).with_filter(Arc::new(AuditOnly)),
    );
    let logger = builder.build();

    logger.log(build_event(
        service.clone(),
        TARGET_AUDIT,
        ACTION_STARTUP,
        MESSAGE_AUDIT_ACCEPTED,
    ))?;
    if let Err(err) = logger.try_log(build_event(
        service.clone(),
        TARGET_CORE,
        ACTION_HEARTBEAT,
        MESSAGE_FILE_ONLY,
    )) {
        logger.log(build_health_event(
            service.clone(),
            Level::Warn,
            ACTION_WRITER_WARNING,
            MESSAGE_QUEUE_FULL,
            serde_json::Map::from_iter([(FIELD_ERROR.to_string(), json!(err.to_string()))]),
        ))?;
    }
    logger.flush()?;

    let health = logger.health();
    logger.log(build_health_event(
        service.clone(),
        Level::Info,
        ACTION_LOGGER_HEALTH,
        MESSAGE_LOGGER_HEALTH,
        serde_json::Map::from_iter([
            (
                "active_log_path".to_string(),
                json!(health.active_log_path.display().to_string()),
            ),
            (FIELD_QUEUE_DEPTH.to_string(), json!(health.queue_depth)),
            (FIELD_QUEUE_CAPACITY.to_string(), json!(health.queue_capacity)),
            (
                FIELD_QUEUE_HIGH_WATER.to_string(),
                json!(health.queue_high_water_mark),
            ),
            (
                FIELD_QUEUE_FULL_DROPS.to_string(),
                json!(health.queue_full_drops_total),
            ),
            (FIELD_STATE.to_string(), json!(format!("{:?}", health.state))),
            (
                FIELD_WRITER_STATE.to_string(),
                json!(format!("{:?}", health.writer_state)),
            ),
        ]),
    ))?;

    if health.queue_full_drops_total != 0 {
        logger.log(build_health_event(
            service.clone(),
            Level::Warn,
            ACTION_WRITER_WARNING,
            MESSAGE_QUEUE_DROPS,
            serde_json::Map::from_iter([(
                FIELD_QUEUE_FULL_DROPS.to_string(),
                json!(health.queue_full_drops_total),
            )]),
        ))?;
    }

    if health.writer_state != WriterState::Running {
        logger.log(build_health_event(
            service.clone(),
            Level::Warn,
            ACTION_WRITER_WARNING,
            MESSAGE_WRITER_STATE,
            serde_json::Map::from_iter([(
                FIELD_WRITER_STATE.to_string(),
                json!(format!("{:?}", health.writer_state)),
            )]),
        ))?;
    }

    if let Some(error) = &health.last_writer_error {
        logger.log(build_health_event(
            service.clone(),
            Level::Warn,
            ACTION_WRITER_WARNING,
            MESSAGE_WRITER_ERROR,
            serde_json::Map::from_iter([
                (
                    FIELD_WRITER_ERROR_CODE.to_string(),
                    json!(error.code.as_ref().map(|code| code.as_str()).unwrap_or("<no-code>")),
                ),
                (
                    FIELD_WRITER_ERROR_MESSAGE.to_string(),
                    json!(error.message.clone()),
                ),
            ]),
        ))?;
    }

    for sink in &health.sink_statuses {
        let level = if sink.state == SinkHealthState::Healthy {
            Level::Info
        } else {
            Level::Warn
        };
        logger.log(build_health_event(
            service.clone(),
            level,
            ACTION_SINK_HEALTH,
            MESSAGE_SINK_HEALTH,
            serde_json::Map::from_iter([
                (FIELD_SINK_NAME.to_string(), json!(sink.name.as_str())),
                (FIELD_SINK_STATE.to_string(), json!(format!("{:?}", sink.state))),
            ]),
        ))?;
    }

    Ok(())
}
