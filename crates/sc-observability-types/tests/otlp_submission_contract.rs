use sc_observability_types::{SpanId, Timestamp, TraceId, otlp::submission::*};
fn stamp() -> Timestamp {
    Timestamp::UNIX_EPOCH
}
struct Ids;
impl IdSource for Ids {
    fn trace_id(&mut self) -> TraceId {
        TraceId::new("0123456789abcdef0123456789abcdef").unwrap()
    }
    fn span_id(&mut self) -> SpanId {
        SpanId::new("0123456789abcdef").unwrap()
    }
    fn now(&mut self) -> Timestamp {
        stamp()
    }
}
#[test]
fn paired_records_share_generated_or_supplied_ids() {
    let json = r#"{"version":1,"logs":[{"body":"result","correlation_id":"run"}],"spans":[{"name":"check","start_time":"1970-01-01T00:00:00Z","duration_nanos":100,"correlation_id":"run"}]}"#;
    let envelope = SubmissionEnvelope::from_json(json, &mut Ids).unwrap();
    assert_eq!(
        envelope.logs[0].record.trace_id.as_ref(),
        Some(&envelope.spans[0].record.trace_id)
    );
    assert_eq!(
        envelope.logs[0].record.span_id.as_ref(),
        Some(&envelope.spans[0].record.span_id)
    );
    assert_eq!(
        envelope.spans[0]
            .record
            .end_time
            .into_inner()
            .unix_timestamp_nanos(),
        100
    );
    assert_eq!(envelope.logs[0].record.time, None);
    let supplied = json.replace(
        "\"body\":\"result\"",
        "\"body\":\"result\",\"span_id\":\"1111111111111111\"",
    );
    let envelope = SubmissionEnvelope::from_json(&supplied, &mut Ids).unwrap();
    assert_eq!(
        envelope.spans[0].record.span_id.as_str(),
        "1111111111111111"
    );
    let both = supplied.replace(
        "\"name\":\"check\"",
        "\"name\":\"check\",\"span_id\":\"2222222222222222\"",
    );
    assert!(matches!(
        SubmissionEnvelope::from_json(&both, &mut Ids),
        Err(SubmissionError::CorrelationConflict { .. })
    ));
}
#[test]
fn canonicalization_reports_precise_failure_classes() {
    let cases = [
        ("{", "SC_OBSERVABILITY_SUBMIT_INVALID_JSON"),
        (
            r#"{"version":1,"logs":[{"attributes":{"a":null}}]}"#,
            "SC_OBSERVABILITY_SUBMIT_VALIDATION",
        ),
        (
            r#"{"version":2,"logs":[{}]}"#,
            "SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION",
        ),
        (r#"{"version":1}"#, "SC_OBSERVABILITY_SUBMIT_EMPTY"),
        (
            r#"{"version":1,"logs":[{"body":{"kind":"uint","data":18446744073709551615}}]}"#,
            "SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE",
        ),
        (
            r#"{"version":1,"logs":[{"attributes":[[1,"value"]]}]}"#,
            "SC_OBSERVABILITY_SUBMIT_VALIDATION",
        ),
        (
            r#"{"version":1,"logs":[{"body":{"kind":"string_index","data":0}}]}"#,
            "SC_OBSERVABILITY_SUBMIT_VALIDATION",
        ),
        (
            r#"{"version":1,"spans":[{"name":"x","start_time":"1970-01-01T00:00:00Z","end_time":"1970-01-01T00:00:01Z","duration_nanos":2}]}"#,
            "SC_OBSERVABILITY_SUBMIT_TIMING_CONFLICT",
        ),
    ];
    for (json, code) in cases {
        assert_eq!(
            SubmissionEnvelope::from_json(json, &mut Ids)
                .unwrap_err()
                .code()
                .as_str(),
            code,
            "{json}"
        );
    }
}
#[test]
fn config_uses_only_allowed_sources_and_redacts_secrets() {
    let mut file:TelemetryFileConfig=serde_json::from_str(r#"{"service":"file-service","otlp":{"endpoint":"http://file:4318","timeout_ms":20},"store":{"path":"store.db","max_bytes":1234,"disk_bound_policy":"evict_oldest","delivered_retention_hours":2},"auth_header":"ignored","backend":"open_telemetry_sdk"}"#).unwrap();
    file.base_dir = "/tmp/config".into();
    let env = |key: &str| match key {
        "OTEL_SERVICE_NAME" => Some("env-service".into()),
        "OTEL_EXPORTER_OTLP_ENDPOINT" => Some("http://env:4318".into()),
        "SC_OTEL_AUTH_HEADER" => Some("secret".into()),
        _ => panic!("unexpected env key {key}"),
    };
    let mut overrides = ConfigOverrides::default();
    let config = resolve_config(ConfigSources::new(&overrides, Some(&file), &env)).unwrap();
    assert_eq!(config.service_name, "file-service");
    assert_eq!(config.endpoint, "http://file:4318");
    assert_eq!(
        config.store_path,
        std::path::PathBuf::from("/tmp/config/store.db")
    );
    assert_eq!(config.request_timeout, std::time::Duration::from_millis(20));
    assert_eq!(config.max_store_bytes, 1234);
    assert_eq!(config.backend, ExporterBackendId::SyncHttp);
    assert_eq!(config.auth_header.as_ref().unwrap().expose(), "secret");
    assert!(!format!("{config:?}").contains("secret"));
    overrides.service_name = Some("explicit".into());
    overrides.store_path = Some("explicit.db".into());
    let config = resolve_config(ConfigSources::new(&overrides, Some(&file), &env)).unwrap();
    assert_eq!(config.service_name, "explicit");
    assert_eq!(config.store_path, std::path::PathBuf::from("explicit.db"));
    let empty = ConfigOverrides::default();
    assert!(matches!(
        resolve_config(ConfigSources::new(&empty, None, &env)),
        Err(TelemetryConfigError::MissingField {
            field: "store_path",
            ..
        })
    ));
}
#[test]
fn flush_terminal_failure_precedes_pending_deadline() {
    let mut report = FlushReport::default();
    report.failed.logs = 1;
    report.still_pending.traces = 1;
    assert!(matches!(
        report.into_result(),
        Err(TelemetryClientError::Delivery(
            DeliveryError::TerminalFailure { .. }
        ))
    ));
    let mut report = FlushReport::default();
    report.still_pending.profiles = 1;
    assert!(matches!(
        report.into_result(),
        Err(TelemetryClientError::Delivery(
            DeliveryError::DeadlineExceeded { .. }
        ))
    ));
    assert!(FlushReport::default().into_result().is_ok());
}
#[test]
fn flattened_metric_input_retains_resource_override() {
    let json = r#"{"version":1,"metrics":[{"name":"requests","description":null,"unit":null,"metadata":{},"data":{"kind":"gauge","data":{"points":[{"attributes":{},"start_time":null,"time":"1970-01-01T00:00:01Z","value":{"kind":"int","data":1},"exemplars":[],"flags":0}]}},"resource":{"attributes":{"service.name":"override"},"dropped_attributes_count":0,"entity_refs":[],"schema_url":null}}]}"#;
    let envelope = SubmissionEnvelope::from_json(json, &mut Ids).unwrap();
    assert_eq!(envelope.metrics.len(), 1);
    assert_eq!(envelope.metrics[0].resource.attributes.entries().len(), 1);
}

#[cfg(feature = "test-double")]
mod double_tests {
    use super::*;
    use sc_observability_types::otlp::submission::testing::{
        DeliveryOutcome, DoubleScript, InMemoryTelemetryClient, ScriptedDelivery,
        conformance::{ConformanceHarness, run_all},
    };
    use std::time::{Duration, Instant};
    fn config() -> TelemetryClientConfig {
        let mut overrides = ConfigOverrides::default();
        overrides.store_path = Some("unused.db".into());
        resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap()
    }
    struct Harness(Option<InMemoryTelemetryClient>);
    impl ConformanceHarness for Harness {
        type Client = InMemoryTelemetryClient;
        fn open(&mut self) -> Self::Client {
            let client = InMemoryTelemetryClient::open(config()).unwrap();
            self.0 = Some(client.clone());
            client
        }
        fn set_outcome(&mut self, signal: Signal, outcome: DeliveryOutcome) {
            let mut script = DoubleScript::default();
            script
                .deliveries
                .push(ScriptedDelivery::new(signal, outcome));
            self.0.as_ref().unwrap().push_script(script);
        }
    }
    fn envelope() -> SubmissionEnvelope {
        SubmissionEnvelope::from_json(r#"{"version":1,"logs":[{}]}"#, &mut Ids).unwrap()
    }
    #[test]
    fn shared_conformance_suite() {
        run_all(&mut Harness(None));
    }
    #[test]
    fn scripted_admission_rejection_each_kind() {
        for (kind, code) in [
            (
                "store_unavailable",
                "SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE",
            ),
            ("disk_bound_exceeded", "SC_OBSERVABILITY_ADMIT_DISK_BOUND"),
            ("persistence", "SC_OBSERVABILITY_ADMIT_PERSISTENCE"),
            ("schema_too_new", "SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW"),
            ("closed", "SC_OBSERVABILITY_ADMIT_CLOSED"),
        ] {
            let script = DoubleScript::from_json(&format!(
                r#"{{"admissions":[{{"outcome":"reject","kind":"{kind}"}}]}}"#
            ))
            .unwrap();
            let client = InMemoryTelemetryClient::with_script(config(), script);
            assert_eq!(client.emit(envelope()).unwrap_err().code().as_str(), code);
            assert!(client.envelopes().is_empty());
            assert!(client.emit(envelope()).is_ok());
        }
    }
    #[test]
    fn double_script_json_round_trip_and_unknown_field_rejection() {
        let script=DoubleScript::from_json(r#"{"admissions":[{"outcome":"admit"}],"deliveries":[{"signal":"logs","outcome":"fail"}],"flush_delay_ms":2}"#).unwrap();
        assert_eq!(
            DoubleScript::from_json(&serde_json::to_string(&script).unwrap()).unwrap(),
            script
        );
        for json in [
            r#"{"unexpected":true}"#,
            r#"{"deliveries":[{"signal":"logs","outcome":"fail","unexpected":true}]}"#,
            r#"{"admissions":[{"outcome":"admit","unexpected":true}]}"#,
        ] {
            assert!(DoubleScript::from_json(json).is_err());
        }
    }
    #[test]
    fn flush_delay_applies_to_flush_and_shutdown() {
        let script = DoubleScript::from_json(r#"{"flush_delay_ms":5}"#).unwrap();
        let client = InMemoryTelemetryClient::with_script(config(), script);
        let receipt = client.emit(envelope()).unwrap();
        let start = Instant::now();
        client.flush(Duration::from_millis(20)).unwrap();
        assert!(start.elapsed() >= Duration::from_millis(5));
        let start = Instant::now();
        client
            .flush_submission(&receipt.submission_id, Duration::from_millis(20))
            .unwrap();
        assert!(start.elapsed() >= Duration::from_millis(5));
        let start = Instant::now();
        client.shutdown(Duration::from_millis(20)).unwrap();
        assert!(start.elapsed() >= Duration::from_millis(5));
    }
    #[test]
    fn shutdown_releases_client_even_on_delivery_failure() {
        let script =
            DoubleScript::from_json(r#"{"deliveries":[{"signal":"logs","outcome":"fail"}]}"#)
                .unwrap();
        let client = InMemoryTelemetryClient::with_script(config(), script);
        client.emit(envelope()).unwrap();
        assert!(client.shutdown(Duration::from_millis(1)).is_err());
        assert_eq!(
            client.emit(envelope()).unwrap_err().code().as_str(),
            "SC_OBSERVABILITY_ADMIT_CLOSED"
        );
        assert!(client.shutdown(Duration::ZERO).is_ok());
    }
}

#[test]
fn shared_golden_submission_fixtures() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/otlp_submission/golden");
    let mut fixtures: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    fixtures.sort();
    assert!(fixtures.len() >= 20);
    for fixture in fixtures {
        let input = std::fs::read_to_string(fixture.join("input.json")).unwrap();
        let actual = SubmissionEnvelope::from_json(&input, &mut Ids);
        if fixture.join("expected.error.json").exists() {
            let expected: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(fixture.join("expected.error.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                actual.unwrap_err().code().as_str(),
                expected["code"].as_str().unwrap(),
                "{}",
                fixture.display()
            );
        } else {
            let actual = actual.unwrap_or_else(|e| panic!("{}: {e}", fixture.display()));
            let expected: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(fixture.join("expected.envelope.json"))
                    .unwrap()
                    .replace("$GENERATED_TRACE_ID", "0123456789abcdef0123456789abcdef")
                    .replace("$GENERATED_SPAN_ID", "0123456789abcdef")
                    .replace("$GENERATED_NOW", "1970-01-01T00:00:00.000000000Z"),
            )
            .unwrap();
            let canonical = actual.to_canonical_json();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&canonical).unwrap(),
                expected,
                "{}",
                fixture.display()
            );
            assert_eq!(
                serde_json::from_str::<SubmissionEnvelope>(&canonical).unwrap(),
                actual
            );
        }
    }
}

#[test]
fn configuration_precedence_covers_every_field() {
    use std::time::Duration;
    let file: TelemetryFileConfig = serde_json::from_str(r#"{
      "service":"file", "backend":"open_telemetry_sdk", "auth_header":"forbidden",
      "otlp":{"endpoint":"http://file:4318", "timeout_ms":3, "auth_header":"forbidden"},
      "store":{"path":"file.db", "max_bytes":4,"disk_bound_policy":"evict_oldest","delivered_retention_hours":5},
      "record_key_retention":1,"emit_flush_deadline":1,"flush_deadline":1,"lease_duration":1,"sync_http_retry":{"max_retries":999}
    }"#).unwrap();
    let env = |key: &str| match key {
        "OTEL_SERVICE_NAME" => Some("env".into()),
        "OTEL_EXPORTER_OTLP_ENDPOINT" => Some("http://env:4318".into()),
        "SC_OTEL_AUTH_HEADER" => Some("env-secret".into()),
        _ => panic!("forbidden environment source: {key}"),
    };
    let mut explicit = ConfigOverrides::default();
    explicit.service_name = Some("explicit".into());
    explicit.endpoint = Some("http://explicit:4318".into());
    explicit.backend = Some(ExporterBackendId::OpenTelemetrySdk);
    explicit.request_timeout = Some(Duration::from_secs(6));
    explicit.auth_header = Some(Secret::new("explicit-secret".into()));
    explicit.store_path = Some("explicit.db".into());
    explicit.max_store_bytes = Some(7);
    explicit.disk_bound_policy = Some(DiskBoundPolicy::RejectNew);
    explicit.delivered_retention = Some(Duration::from_secs(8));
    explicit.record_key_retention = Some(Duration::from_secs(9));
    explicit.emit_flush_deadline = Some(Duration::from_secs(10));
    explicit.flush_deadline = Some(Duration::from_secs(11));
    explicit.lease_duration = Some(Duration::from_secs(12));
    let retry: SyncHttpRetryPolicyDto = serde_json::from_str(r#"{"max_retries":1,"initial_backoff_ms":2,"max_backoff_ms":3,"retry_sequence_timeout_ms":4,"retry_after_cap_ms":5,"retry_jitter_percent":6}"#).unwrap();
    explicit.sync_http_retry = Some(retry.clone());
    let config = resolve_config(ConfigSources::new(&explicit, Some(&file), &env)).unwrap();
    assert_eq!(config.service_name, "explicit");
    assert_eq!(config.endpoint, "http://explicit:4318");
    assert_eq!(config.backend, ExporterBackendId::OpenTelemetrySdk);
    assert_eq!(config.request_timeout, Duration::from_secs(6));
    assert_eq!(config.auth_header.unwrap().expose(), "explicit-secret");
    assert_eq!(config.store_path, std::path::PathBuf::from("explicit.db"));
    assert_eq!(config.max_store_bytes, 7);
    assert_eq!(config.disk_bound_policy, DiskBoundPolicy::RejectNew);
    assert_eq!(config.delivered_retention, Duration::from_secs(8));
    assert_eq!(config.record_key_retention, Duration::from_secs(9));
    assert_eq!(config.emit_flush_deadline, Duration::from_secs(10));
    assert_eq!(config.flush_deadline, Duration::from_secs(11));
    assert_eq!(config.lease_duration, Duration::from_secs(12));
    assert_eq!(config.sync_http_retry, Some(retry));
    let mut empty = ConfigOverrides::default();
    let file_config = resolve_config(ConfigSources::new(&empty, Some(&file), &env)).unwrap();
    assert_eq!(file_config.service_name, "file");
    assert_eq!(file_config.endpoint, "http://file:4318");
    assert_eq!(file_config.request_timeout, Duration::from_millis(3));
    assert_eq!(file_config.store_path, std::path::PathBuf::from("file.db"));
    assert_eq!(file_config.max_store_bytes, 4);
    assert_eq!(file_config.disk_bound_policy, DiskBoundPolicy::EvictOldest);
    assert_eq!(
        file_config.delivered_retention,
        Duration::from_secs(5 * 3600)
    );
    assert_eq!(file_config.backend, ExporterBackendId::SyncHttp);
    assert_eq!(file_config.auth_header.unwrap().expose(), "env-secret");
    assert_eq!(
        file_config.record_key_retention,
        Duration::from_secs(30 * 86400)
    );
    assert_eq!(file_config.emit_flush_deadline, Duration::from_secs(5));
    assert_eq!(file_config.flush_deadline, Duration::from_secs(30));
    assert_eq!(file_config.lease_duration, Duration::from_secs(30));
    assert!(file_config.sync_http_retry.is_none());
    empty.store_path = Some("default.db".into());
    let env_config = resolve_config(ConfigSources::new(&empty, None, &env)).unwrap();
    assert_eq!(env_config.service_name, "env");
    assert_eq!(env_config.endpoint, "http://env:4318");
    let defaults = resolve_config(ConfigSources::new(&empty, None, &|_| None)).unwrap();
    assert_eq!(defaults.service_name, "unknown_service");
    assert_eq!(defaults.endpoint, "http://localhost:4318");
    assert_eq!(defaults.request_timeout, Duration::from_secs(10));
    assert_eq!(defaults.max_store_bytes, 256 * 1024 * 1024);
    assert_eq!(defaults.disk_bound_policy, DiskBoundPolicy::RejectNew);
    assert_eq!(defaults.delivered_retention, Duration::from_secs(24 * 3600));
    assert!(defaults.auth_header.is_none());
}

#[test]
fn error_registry_is_unique_and_deserialization_revalidates_envelopes() {
    let mut codes = std::collections::BTreeSet::new();
    for code in sc_observability_types::error_codes::ALL {
        assert!(codes.insert(code.as_str()), "duplicate {code}");
    }
    let envelope = SubmissionEnvelope::from_json(r#"{"version":1,"logs":[{}]}"#, &mut Ids).unwrap();
    let mut value = serde_json::to_value(envelope).unwrap();
    value["logs"][0]["record"]["body"] = serde_json::json!({"kind":"uint","data":u64::MAX});
    assert!(serde_json::from_value::<SubmissionEnvelope>(value).is_err());
}
