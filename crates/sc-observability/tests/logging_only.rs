use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use sc_observability::constants::{DEFAULT_LOG_DIR_NAME, DEFAULT_LOG_FILE_SUFFIX};
use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::{
    ActionName, Diagnostic, ErrorCode, Level, LogEvent, OutcomeLabel, ProcessIdentity, Remediation,
    SchemaVersion, ServiceName, TargetCategory, Timestamp,
};
use serde_json::json;

struct TestRoot(tempfile::TempDir);

impl TestRoot {
    fn path_buf(&self) -> PathBuf {
        self.0.path().to_path_buf()
    }
}

impl Deref for TestRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.path()
    }
}

fn temp_root(name: &str) -> TestRoot {
    TestRoot(
        tempfile::Builder::new()
            .prefix(&format!("sc-observability-integration-{name}-"))
            .tempdir()
            .expect("create temporary test root"),
    )
}

fn service_name() -> ServiceName {
    ServiceName::new("logging-only-app").expect("valid service name")
}

fn event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("valid schema version"),
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
        outcome: Some(OutcomeLabel::new("ok").expect("valid outcome label")),
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
#[expect(
    deprecated,
    reason = "integration coverage intentionally exercises the deprecated emit() compatibility path"
)]
fn logging_only_consumer_can_emit_without_routing_or_otlp() {
    let root = temp_root("logging-only");
    let logger =
        Logger::new(LoggerConfig::default_for(service_name(), root.path_buf())).expect("logger");

    logger.emit(event()).expect("emit");
    logger.flush().expect("flush");

    let path = root
        .join(DEFAULT_LOG_DIR_NAME)
        .join(format!("logging-only-app{DEFAULT_LOG_FILE_SUFFIX}"));
    let contents = fs::read_to_string(path).expect("read log output");
    assert!(contents.contains("\"action\":\"startup\""));
    assert!(contents.contains("\"message\":\"boot complete\""));
}
