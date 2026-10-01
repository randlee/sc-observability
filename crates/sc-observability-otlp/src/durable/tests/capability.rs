use super::*;
#[test]
fn every_matrix_row_delivers_and_sdk_is_rejected() {
    for name in [
        "logs",
        "traces",
        "metric_gauge",
        "metric_sum",
        "metric_histogram",
        "metric_exponential_histogram",
        "metric_summary",
        "profiles",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let client = DurableTelemetryClient::open_with_exporter(
            config(dir.path()),
            Arc::new(ScriptedExporter::new(dir.path())),
        )
        .unwrap();
        let receipt = client.emit(fixture(name)).unwrap();
        assert_eq!(
            client
                .flush_submission(&receipt.submission_id, DEADLINE)
                .unwrap()
                .delivered
                .total(),
            1,
            "{name}"
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    config.backend = ExporterBackendId::OpenTelemetrySdk;
    assert!(matches!(
        DurableTelemetryClient::open(config),
        Err(TelemetryClientError::Config(
            TelemetryConfigError::UnsupportedCombination {
                backend: ExporterBackendId::OpenTelemetrySdk,
                ..
            }
        ))
    ));
    assert!(!dir.path().join("telemetry.db").exists());
}
