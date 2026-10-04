use sc_observability_dto::*;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn roundtrip<T: serde::de::DeserializeOwned + serde::Serialize>(v: Value) -> Value {
    serde_json::to_value(serde_json::from_value::<T>(v).expect("Serde accepts schema fixture"))
        .unwrap()
}

fn selected_v1_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../schema")
        .join(filename)
}

fn temporary_contract_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "sc-observability-schema-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("create temporary contract directory");
    directory
}

fn generator_check(schema: &Path, errors: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sc-observability-schema"))
        .args([
            "--output",
            schema.to_str().expect("schema path is UTF-8"),
            "--errors-output",
            errors.to_str().expect("error catalogue path is UTF-8"),
            "--check",
        ])
        .output()
        .expect("run schema generator")
}

#[test]
fn selected_v1_contracts_match_current_dto_and_error_definitions() {
    let output = generator_check(
        &selected_v1_path("v1.json"),
        &selected_v1_path("errors-v1.json"),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn selected_v1_contract_mismatches_name_contract_version_and_changed_field() {
    let directory = temporary_contract_directory("selected-v1-mismatch");
    let schema = directory.join("v1.json");
    let errors = directory.join("errors-v1.json");
    fs::copy(selected_v1_path("v1.json"), &schema).expect("copy selected schema");
    fs::copy(selected_v1_path("errors-v1.json"), &errors).expect("copy selected errors");

    let mut changed: Value = serde_json::from_slice(&fs::read(&schema).expect("read schema"))
        .expect("selected schema is JSON");
    changed["x-sc-bindings"]["schema_version"] = json!(2);
    let mut bytes = serde_json::to_vec_pretty(&changed).expect("serialize changed schema");
    bytes.push(b'\n');
    fs::write(&schema, &bytes).expect("write changed schema");

    let output = generator_check(&schema, &errors);
    assert!(
        !output.status.success(),
        "changed selected contract must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("binding schema v1"));
    assert!(stderr.contains("/x-sc-bindings/schema_version"));
    assert!(stderr.contains("new versioned snapshot"));
    assert_eq!(fs::read(&schema).expect("read changed schema"), bytes);

    fs::remove_dir_all(directory).expect("remove temporary contract directory");
}

#[test]
fn selected_v1_schema_missing_field_names_contract_version_and_field() {
    let directory = temporary_contract_directory("selected-v1-missing-field");
    let schema = directory.join("v1.json");
    let errors = directory.join("errors-v1.json");
    fs::copy(selected_v1_path("v1.json"), &schema).expect("copy selected schema");
    fs::copy(selected_v1_path("errors-v1.json"), &errors).expect("copy selected errors");

    let mut changed: Value = serde_json::from_slice(&fs::read(&schema).expect("read schema"))
        .expect("selected schema is JSON");
    changed["x-sc-bindings"]
        .as_object_mut()
        .expect("bindings metadata is an object")
        .remove("defaults");
    let mut bytes = serde_json::to_vec_pretty(&changed).expect("serialize changed schema");
    bytes.push(b'\n');
    fs::write(&schema, &bytes).expect("write changed schema");

    let output = generator_check(&schema, &errors);
    assert!(!output.status.success(), "missing selected field must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("binding schema v1"));
    assert!(stderr.contains("/x-sc-bindings/defaults"));
    assert!(stderr.contains("new versioned snapshot"));
    assert_eq!(fs::read(&schema).expect("read changed schema"), bytes);

    fs::remove_dir_all(directory).expect("remove temporary contract directory");
}

#[test]
fn selected_v1_error_catalogue_mismatch_names_contract_version_and_changed_field() {
    let directory = temporary_contract_directory("selected-v1-error-mismatch");
    let schema = directory.join("v1.json");
    let errors = directory.join("errors-v1.json");
    fs::copy(selected_v1_path("v1.json"), &schema).expect("copy selected schema");
    fs::copy(selected_v1_path("errors-v1.json"), &errors).expect("copy selected errors");

    let mut changed: Value = serde_json::from_slice(&fs::read(&errors).expect("read errors"))
        .expect("selected error catalogue is JSON");
    changed[0]["code"] = json!("SC_OBSERVABILITY_BINDING_CHANGED");
    let mut bytes = serde_json::to_vec_pretty(&changed).expect("serialize changed errors");
    bytes.push(b'\n');
    fs::write(&errors, &bytes).expect("write changed errors");

    let output = generator_check(&schema, &errors);
    assert!(
        !output.status.success(),
        "changed selected errors must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("binding error catalogue v1"));
    assert!(stderr.contains("/0/code"));
    assert!(stderr.contains("new versioned snapshot"));
    assert_eq!(fs::read(&errors).expect("read changed errors"), bytes);

    fs::remove_dir_all(directory).expect("remove temporary contract directory");
}
#[test]
fn every_registered_type_agrees_with_serde_and_frozen_expectations() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../../conformance/v1/schema-cases.json")).unwrap();
    for case in cases {
        if case["valid"] != true {
            continue;
        }
        let entry = case["entrypoint"].as_str().unwrap();
        let name = entry
            .strip_prefix("Input")
            .or_else(|| entry.strip_prefix("Output"))
            .unwrap();
        let value = case["value"].clone();
        let actual = match name {
            "AdmissionDto" => roundtrip::<AdmissionDto>(value),
            "AvailabilityDto" => roundtrip::<AvailabilityDto>(value),
            "BridgeHealthDto" => roundtrip::<BridgeHealthDto>(value),
            "ChangeDiagnosticDto" => roundtrip::<ChangeDiagnosticDto>(value),
            "CompletionDto" => roundtrip::<CompletionDto>(value),
            "DecimalDto" => roundtrip::<DecimalDto>(value),
            "Diagnostic" => roundtrip::<Diagnostic>(value),
            "DiagnosticSummaryDto" => roundtrip::<DiagnosticSummaryDto>(value),
            "DispatchDto" => roundtrip::<DispatchDto>(value),
            "DropCountsDto" => roundtrip::<DropCountsDto>(value),
            "Failure" => roundtrip::<Failure>(value),
            "FieldMatchDto" => roundtrip::<FieldMatchDto>(value),
            "FlushRequest" => roundtrip::<FlushRequest>(value),
            "HealthRequest" => roundtrip::<HealthRequest>(value),
            "LevelChangeDto" => roundtrip::<LevelChangeDto>(value),
            "LevelChangeRequest" => roundtrip::<LevelChangeRequest>(value),
            "LevelChangeSourceDto" => roundtrip::<LevelChangeSourceDto>(value),
            "LevelDto" => roundtrip::<LevelDto>(value),
            "LevelFilterDto" => roundtrip::<LevelFilterDto>(value),
            "LevelRequestDto" => roundtrip::<LevelRequestDto>(value),
            "LevelStateDto" => roundtrip::<LevelStateDto>(value),
            "LifecycleDto" => roundtrip::<LifecycleDto>(value),
            "LogEventDto" => roundtrip::<LogEventDto>(value),
            "LogHealthDto" => roundtrip::<LogHealthDto>(value),
            "LogOrderDto" => roundtrip::<LogOrderDto>(value),
            "LogQueryDto" => roundtrip::<LogQueryDto>(value),
            "LogSnapshotDto" => roundtrip::<LogSnapshotDto>(value),
            "LoggingHealthDto" => roundtrip::<LoggingHealthDto>(value),
            "MaintenanceHealthDto" => roundtrip::<MaintenanceHealthDto>(value),
            "PathDto" => roundtrip::<PathDto>(value),
            "ProcessIdentityDto" => roundtrip::<ProcessIdentityDto>(value),
            "QueryHealthDto" => roundtrip::<QueryHealthDto>(value),
            "QueryRequest" => roundtrip::<QueryRequest>(value),
            "QueryStateDto" => roundtrip::<QueryStateDto>(value),
            "RemediationDto" => roundtrip::<RemediationDto>(value),
            "ResultDtoAdmissionDto" => roundtrip::<ResultDto<AdmissionDto>>(value),
            "ResultDtoCompletionDto" => roundtrip::<ResultDto<CompletionDto>>(value),
            "ResultDtoDispatchDto" => roundtrip::<ResultDto<DispatchDto>>(value),
            "ResultDtoLevelChangeDto" => roundtrip::<ResultDto<LevelChangeDto>>(value),
            "ResultDtoLogHealthDto" => roundtrip::<ResultDto<LogHealthDto>>(value),
            "ResultDtoLogSnapshotDto" => roundtrip::<ResultDto<LogSnapshotDto>>(value),
            "SinkHealthDto" => roundtrip::<SinkHealthDto>(value),
            "StateTransitionDto" => roundtrip::<StateTransitionDto>(value),
            "StoredDiagnosticDto" => roundtrip::<StoredDiagnosticDto>(value),
            "StoredEventDto" => roundtrip::<StoredEventDto>(value),
            "TraceContextDto" => roundtrip::<TraceContextDto>(value),
            "TryLogRequest" => roundtrip::<TryLogRequest>(value),
            "ValueDto" => roundtrip::<ValueDto>(value),
            "WireEnvelopeAdmissionDto" => roundtrip::<WireEnvelope<AdmissionDto>>(value),
            "WireEnvelopeCompletionDto" => roundtrip::<WireEnvelope<CompletionDto>>(value),
            "WireEnvelopeDispatchDto" => roundtrip::<WireEnvelope<DispatchDto>>(value),
            "WireEnvelopeLevelChangeDto" => roundtrip::<WireEnvelope<LevelChangeDto>>(value),
            "WireEnvelopeLogHealthDto" => roundtrip::<WireEnvelope<LogHealthDto>>(value),
            "WireEnvelopeLogSnapshotDto" => roundtrip::<WireEnvelope<LogSnapshotDto>>(value),
            "WorkerStateDto" => roundtrip::<WorkerStateDto>(value),
            "OperationDiagnosticDto" => roundtrip::<OperationDiagnosticDto>(value),
            "ClientOutcome" => roundtrip::<ClientOutcome>(value),
            "ClientStatus" => roundtrip::<ClientStatus>(value),
            "FailureCountsDto" => roundtrip::<FailureCountsDto>(value),
            "LogOperationDto" => roundtrip::<LogOperationDto>(value),
            "AdmissionOperationDto" => roundtrip::<AdmissionOperationDto>(value),
            "CompletionOperationDto" => roundtrip::<CompletionOperationDto>(value),
            "ResultDtoClientOutcome" => roundtrip::<ResultDto<ClientOutcome>>(value),
            "ResultDtoClientStatus" => roundtrip::<ResultDto<ClientStatus>>(value),
            "WireEnvelopeClientOutcome" => roundtrip::<WireEnvelope<ClientOutcome>>(value),
            "WireEnvelopeClientStatus" => roundtrip::<WireEnvelope<ClientStatus>>(value),
            "CanonicalDiagnosticDto" => roundtrip::<CanonicalDiagnosticDto>(value),
            "CanonicalFailureDto" => roundtrip::<CanonicalFailureDto>(value),
            "TraceContextV2Dto" => roundtrip::<TraceContextV2Dto>(value),
            "SpanLinkDto" => roundtrip::<SpanLinkDto>(value),
            "SpanKindDto" => roundtrip::<SpanKindDto>(value),
            "AggregationTemporalityDto" => roundtrip::<AggregationTemporalityDto>(value),
            "HistogramPointDto" => roundtrip::<HistogramPointDto>(value),
            "MetricValueDto" => roundtrip::<MetricValueDto>(value),
            "MetricRecordDto" => roundtrip::<MetricRecordDto>(value),
            "SpanStatusDto" => roundtrip::<SpanStatusDto>(value),
            "SpanRecordDto" => roundtrip::<SpanRecordDto>(value),
            "SpanEventDto" => roundtrip::<SpanEventDto>(value),
            "SpanSignalDto" => roundtrip::<SpanSignalDto>(value),
            "CanonicalWireEnvelopeAdmissionDto" => {
                roundtrip::<CanonicalWireEnvelope<AdmissionDto>>(value)
            }
            _ => panic!("unregistered fixture type: {name}"),
        };
        assert_eq!(actual, case["serde_output"], "{}", case["id"]);
    }
}

#[test]
fn semantic_negatives_have_exact_failure_kinds_and_codes() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../../conformance/v1/conversion-cases.json")).unwrap();
    for case in cases {
        let value = case["value"].clone();
        if case["operation"] == "canonical_envelope" {
            match case["result"].as_str().unwrap() {
                "decoded" => {
                    let decoded = decode_canonical_envelope::<AdmissionDto>(value)
                        .unwrap_or_else(|error| panic!("{}: {error:?}", case["id"]));
                    assert_eq!(
                        serde_json::to_value(decoded).unwrap(),
                        case["expected"],
                        "{}",
                        case["id"]
                    );
                }
                "rejected" => {
                    let error = decode_canonical_envelope::<AdmissionDto>(value)
                        .expect_err("malformed canonical envelope must be rejected");
                    assert_eq!(error.diagnostic().code, case["expected_error"]["code"]);
                    assert_eq!(
                        serde_json::to_value(error).unwrap()["kind"],
                        case["expected_error"]["kind"]
                    );
                }
                result => panic!("unknown canonical-envelope result: {result}"),
            }
            continue;
        }
        let error = match case["operation"].as_str().unwrap() {
            "metric" => decode_metric(value).unwrap_err(),
            "span" => decode_span(value).unwrap_err(),
            "event" => decode_event(value)
                .and_then(|event| {
                    to_core_event(
                        event,
                        EventStamp {
                            service: serde_json::from_value(serde_json::json!("conformance"))
                                .unwrap(),
                            timestamp: serde_json::from_value(serde_json::json!(
                                "1970-01-01T00:00:00Z"
                            ))
                            .unwrap(),
                            identity: Default::default(),
                        },
                    )
                })
                .unwrap_err(),
            "query" => decode_query(value).and_then(to_core_query).unwrap_err(),
            "level" => decode_level_request(value).unwrap_err(),
            "timeout" => decode_timeout(value).unwrap_err(),
            "envelope" => decode_envelope::<AdmissionDto>(value).unwrap_err(),
            _ => panic!("unknown operation"),
        };
        let wire = serde_json::to_value(error).unwrap();
        assert_eq!(wire["kind"], case["kind"], "{}", case["id"]);
        assert_eq!(wire["code"], case["code"], "{}", case["id"]);
    }
}
