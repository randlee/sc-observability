#![cfg(feature = "durable-store")]
#[test]
fn durable_open_persists_records_and_loader_retains_typed_errors() {
    use sc_observability_otlp::durable::{DurableTelemetryClient, load_telemetry_file};
    use sc_observability_types::otlp::submission::*;
    let mut overrides = ConfigOverrides::default();
    let dir = tempfile::tempdir().unwrap();
    overrides.store_path = Some(dir.path().join("telemetry.db"));
    let config = resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap();
    let client = DurableTelemetryClient::open(config.clone()).unwrap();
    let mut input = SubmissionInput::new();
    input.logs.push(LogInput::new());
    let receipt = client
        .emit(SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap())
        .unwrap();
    drop(client);
    let reopened = DurableTelemetryClient::open(config).unwrap();
    let status = reopened
        .status(StatusQuery::Submissions(vec![
            receipt.submission_id.clone(),
        ]))
        .unwrap();
    assert_eq!(status.submissions.len(), 1);
    assert_eq!(status.submissions[0].submission_id, receipt.submission_id);
    assert!(matches!(
        load_telemetry_file(std::path::Path::new("absent.yaml")),
        Err(TelemetryConfigError::ConfigFile { .. })
    ));
}
