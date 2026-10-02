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
        let _clock = FrozenClock::new();
        let exporter = Arc::new(ScriptedExporter::new(dir.path()));
        let client = conformance::open_gated(config(dir.path()), &exporter);
        let envelope = fixture(name);
        let signal = envelope.signals().iter().next().unwrap();
        let receipt = client.emit(envelope).unwrap();
        // Capability is independent of Windows fsync/scheduler latency. Drive the
        // real claim/export/commit at a frozen lease clock, then inspect its result.
        assert!(worker::drain_once_for_test(
            &client.owner.shared,
            exporter.as_ref(),
            signal
        ));
        assert_eq!(
            client
                .flush_submission(&receipt.submission_id, Duration::ZERO)
                .unwrap()
                .delivered
                .total(),
            1,
            "{name}"
        );
        let rejected = tempfile::tempdir().unwrap();
        let mut unsupported = config(rejected.path());
        unsupported.backend = ExporterBackendId::OpenTelemetrySdk;
        assert!(
            matches!(
                DurableTelemetryClient::open(unsupported),
                Err(TelemetryClientError::Config(
                    TelemetryConfigError::UnsupportedCombination {
                        backend: ExporterBackendId::OpenTelemetrySdk,
                        signal: Signal::Logs,
                        representation: Representation::Log,
                        ..
                    }
                ))
            ),
            "{name}: open rejects the backend with representative Logs/Log fields before any record exists"
        );
        assert!(!rejected.path().join("telemetry.db").exists());
    }
}

#[test]
fn metric_exemplar_row_is_preserved() {
    use sc_observability_types::otlp::signals::{Exemplar, KeyValues, MetricData, NumberValue};
    let dir = tempfile::tempdir().unwrap();
    let mut envelope = fixture("metric_gauge");
    let MetricData::Gauge { points } = &mut envelope.metrics[0].record.data else {
        panic!("gauge fixture")
    };
    points[0].exemplars.push(Exemplar::new(
        KeyValues::default(),
        sc_observability_types::Timestamp::UNIX_EPOCH,
        NumberValue::Int(7),
        None,
        None,
    ));
    let client = DurableTelemetryClient::open_with_exporter(
        config(dir.path()),
        Arc::new(ScriptedExporter::new(dir.path())),
    )
    .unwrap();
    let receipt = client.emit(envelope.clone()).unwrap();
    let stored: Vec<u8> = client
        .owner
        .shared
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT envelope FROM submissions WHERE submission_id=?1",
            [receipt.submission_id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<SubmissionEnvelope>(&stored).unwrap(),
        envelope
    );
    assert_eq!(
        client
            .flush_submission(&receipt.submission_id, DEADLINE)
            .unwrap()
            .delivered
            .metrics,
        1
    );
}

#[test]
fn production_open_starts_the_sync_http_exporter() {
    let dir = tempfile::tempdir().unwrap();
    let client = DurableTelemetryClient::open(config(dir.path())).unwrap();

    assert_eq!(
        client.owner.shared.config.backend,
        ExporterBackendId::SyncHttp
    );
    assert_eq!(
        client
            .owner
            .shared
            .live_workers
            .load(std::sync::atomic::Ordering::Acquire),
        5,
        "production open should start the lease worker and all four signal workers"
    );
    client.shutdown(DEADLINE).unwrap();
}
