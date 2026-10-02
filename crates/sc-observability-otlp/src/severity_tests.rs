use crate::sdk::implementation::project_logs;
use crate::severity::fields;
use crate::sync_http::payload::{build_logs_payload, log_record};
use sc_observability_types::otlp::signals::LogPoint;
use sc_observability_types::{
    ActionName, Level, LogEvent, ProcessIdentity, SchemaVersion, ServiceName, TargetCategory,
    Timestamp,
};

fn event(level: Level) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("valid schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level,
        service: ServiceName::new("severity-conformance").expect("valid service"),
        target: TargetCategory::new("otlp.severity").expect("valid target"),
        action: ActionName::new("project").expect("valid action"),
        message: Some("canonical severity".to_owned()),
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

#[test]
fn log_severity_projection_matches_the_canonical_log_point_for_every_level() {
    for level in [
        Level::Trace,
        Level::Debug,
        Level::Info,
        Level::Warn,
        Level::Error,
    ] {
        let event = event(level);
        let canonical = LogPoint::try_from(event.clone()).expect("valid log event converts");
        let (number, text) = fields(level);

        assert_eq!(canonical.severity_number.get(), number);
        assert_eq!(canonical.severity_text.as_deref(), Some(text));

        let record = log_record(&event);
        let payload = build_logs_payload(std::slice::from_ref(&record));
        let payload_log = &payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        assert_eq!(payload_log["severityNumber"], number);
        assert_eq!(payload_log["severityText"], text);

        let sdk_log = &project_logs(&[record])[0].scope_logs[0].log_records[0];
        assert_eq!(sdk_log.severity_number, i32::from(number));
        assert_eq!(sdk_log.severity_text, text);
    }
}
