use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, Level, LogEvent, ProcessIdentity, Remediation, ServiceName,
    TargetCategory, Timestamp,
};
use serde_json::json;

fn temp_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sc-observability-integration-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&path);
    path
}

fn service_name() -> ServiceName {
    ServiceName::new("logging-only-app").expect("valid service name")
}

fn event() -> LogEvent {
    LogEvent {
        version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("app.core").expect("valid target"),
        action: ActionName::new("startup").expect("valid action"),
        message: Some("boot complete".to_string()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: Some("ok".to_string()),
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_TEST"),
            message: "integration".to_string(),
            cause: None,
            remediation: Remediation::recoverable("retry", ["inspect log output"]),
            docs: None,
            details: serde_json::Map::new(),
        }),
        state_transition: None,
        fields: serde_json::Map::from_iter([("attempt".to_string(), json!(1))]),
    }
}

#[test]
fn logging_only_consumer_can_emit_without_routing_or_otlp() {
    let root = temp_path("logging-only");
    let logger =
        Logger::new(LoggerConfig::default_for(service_name(), root.clone())).expect("logger");

    logger.emit(event()).expect("emit");
    logger.flush().expect("flush");

    let path = root
        .join("logging-only-app")
        .join("logs")
        .join("logging-only-app.log.jsonl");
    let contents = fs::read_to_string(path).expect("read log output");
    assert!(contents.contains("\"action\":\"startup\""));
    assert!(contents.contains("\"message\":\"boot complete\""));
}
