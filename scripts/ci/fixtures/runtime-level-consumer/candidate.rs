use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::{
    ActionName, AdmissionOutcome, Level, LevelChange, LevelChangeError, LevelChangeSource,
    LevelFilter, LogEvent, LogFieldMatch, LogQuery, ProcessIdentity, SchemaVersion, ServiceName,
    TargetCategory, Timestamp,
};
use serde_json::json;

fn event(service: ServiceName, level: Level, request: &str) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").unwrap(),
        timestamp: Timestamp::now_utc(),
        level,
        service,
        target: TargetCategory::new("bp2.consumer").unwrap(),
        action: ActionName::new("exercise").unwrap(),
        message: Some(request.into()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::from_iter([("request".into(), json!(request))]),
    }
}

fn main() {
    let _ = std::fs::remove_dir_all("logs");
    let service = ServiceName::new("bp2-candidate").unwrap();
    let config = LoggerConfig::default_for(service.clone(), PathBuf::from("logs"));
    let (logger, mut owner) = Logger::new_with_level_owner(config).unwrap();

    assert_eq!(
        logger
            .try_log_with_outcome(event(service.clone(), Level::Debug, "filtered"))
            .unwrap(),
        AdmissionOutcome::Filtered
    );
    assert!(matches!(
        owner
            .elevate_level(LevelFilter::Debug, LevelChangeSource::Application)
            .unwrap(),
        LevelChange::Changed { .. }
    ));
    assert_eq!(logger.level_state().effective_level, LevelFilter::Debug);

    assert_eq!(
        logger
            .try_log_with_outcome(event(service.clone(), Level::Debug, "accepted"))
            .unwrap(),
        AdmissionOutcome::Accepted
    );
    logger.flush().unwrap();
    let snapshot = logger
        .query(&LogQuery {
            field_matches: vec![LogFieldMatch::equals("request", json!("accepted"))],
            ..LogQuery::default()
        })
        .unwrap();
    assert_eq!(snapshot.events.len(), 1);

    owner.reset_level(LevelChangeSource::Application).unwrap();
    assert_eq!(logger.level_state().effective_level, LevelFilter::Info);
    let stopped = logger.shutdown();
    assert_eq!(stopped.level_state().revision, 2);
    assert!(matches!(
        owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application),
        Err(LevelChangeError::Stopped)
    ));
}
