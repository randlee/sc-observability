use sc_observability_dto::*;
use sc_observability_types as core;
use serde_json::json;
fn main() {
    let input=decode_event(json!({"schema_version":1,"level":"info","target":"external","action":"conversion","fields":{"maximum":{"kind":"integer","value":"18446744073709551615"}}})).expect("checked event");
    let native=to_core_event(input,EventStamp {service:core::ServiceName::new("consumer").unwrap(),timestamp:core::Timestamp::UNIX_EPOCH,identity:core::ProcessIdentity::default()}).expect("core event");
    let snapshot=from_core_snapshot(core::LogSnapshot {events:vec![native],truncated:false}).unwrap();
    assert_eq!(serde_json::to_value(snapshot).unwrap()["events"][0]["fields"]["maximum"]["value"],u64::MAX.to_string());
    assert_eq!(to_core_query(decode_query(json!({"schema_version":1,"since":"1970-01-01T00:00:00Z","until":"1970-01-01T00:00:00Z"})).unwrap()).unwrap().limit,Some(100));
    let state=core::LevelState {configured_level:core::LevelFilter::Info,effective_level:core::LevelFilter::Debug,revision:u64::MAX};
    let health=from_core_health(core::LoggingHealthReport {state:core::LoggingHealthState::Healthy,dropped_events_total:u64::MAX,flush_errors_total:0,active_log_path:"logs.jsonl".into(),sink_statuses:vec![],queue_depth:0,queue_capacity:32,queue_high_water_mark:0,queue_full_drops_total:0,writer_state:core::WriterState::Running,last_writer_error:None,query:None,maintenance:None,last_error:None},state).unwrap();
    assert_eq!(health.level_state.level_revision.as_str(),u64::MAX.to_string());
    let diagnostic=core::OperationDiagnostic {at:core::Timestamp::UNIX_EPOCH,code:core::ErrorCode::new_owned("EXTERNAL_ORIGINAL"),message:"original".into(),remediation:core::Remediation::Recoverable {steps:core::RecoverableSteps::all(["one","two"])}};
    let failure=from_level_error(core::LevelChangeError::Unavailable {diagnostic});assert_eq!(failure.diagnostic().code,"EXTERNAL_ORIGINAL");assert_eq!(failure.diagnostic().at,"1970-01-01T00:00:00Z");
    println!("BINDING_CONSUMER_OK: event query snapshot health diagnostic decimal");
}
