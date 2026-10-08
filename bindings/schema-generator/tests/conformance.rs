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

fn selected_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../schema")
        .join(filename)
}

struct TemporaryContractDirectory(PathBuf);

impl TemporaryContractDirectory {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "sc-observability-schema-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create temporary contract directory");
        Self(directory)
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryContractDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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
fn temporary_contract_directory_removes_path_after_unwind() {
    let mut temporary_path = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let directory = TemporaryContractDirectory::new("unwind-cleanup");
        temporary_path = Some(directory.path().to_path_buf());
        panic!("temporary directory cleanup witness");
    }));

    assert!(result.is_err(), "the cleanup witness must unwind");
    assert!(
        !temporary_path
            .expect("the temporary path was recorded before unwinding")
            .exists(),
        "drop guard must remove the directory while unwinding"
    );
}

#[test]
fn selected_v2_contracts_match_current_dto_and_error_definitions() {
    let output = generator_check(&selected_path("v2.json"), &selected_path("errors-v2.json"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn selected_v1_contract_history_matches_the_accepted_local_baseline() {
    // This is the accepted Phase E base for the initial binding snapshots.
    // Keep it local and pinned: v1 is history, not an editable current source.
    let accepted_baseline = "51be650a08e2a118039eb62ae0f1af7cdc78789a";
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for (contract_name, filename) in [
        ("binding schema", "v1.json"),
        ("binding error catalogue", "errors-v1.json"),
    ] {
        let repository_path = format!("bindings/schema/{filename}");
        let baseline_spec = format!("{accepted_baseline}:{repository_path}");
        let baseline = Command::new("git")
            .current_dir(&repository_root)
            .args(["show", baseline_spec.as_str()])
            .output()
            .expect("read accepted local schema baseline with git show");
        assert!(
            baseline.status.success(),
            "immutable history check: {contract_name} v1 cannot read accepted baseline \
             {accepted_baseline}:{repository_path}: {}",
            String::from_utf8_lossy(&baseline.stderr),
        );

        let selected = fs::read(selected_path(filename)).unwrap_or_else(|error| {
            panic!(
                "immutable history check: {contract_name} v1 retained file \
                 {repository_path} is missing ({error}); v1 is immutable and an intentional \
                 contract change requires a new v2 snapshot"
            )
        });
        assert!(
            selected == baseline.stdout,
            "immutable history check: {contract_name} v1 differs from accepted baseline \
             {accepted_baseline}:{repository_path}; changed snapshot content (current {} bytes, \
             accepted {} bytes) must be captured in a new v2 snapshot instead of overwriting \
             retained v1",
            selected.len(),
            baseline.stdout.len(),
        );
    }
}

#[test]
fn selected_v1_write_mode_refuses_to_overwrite_accepted_contracts() {
    let output = Command::new(env!("CARGO_BIN_EXE_sc-observability-schema"))
        .args([
            "--output",
            selected_path("v1.json")
                .to_str()
                .expect("schema path is UTF-8"),
            "--errors-output",
            selected_path("errors-v1.json")
                .to_str()
                .expect("error catalogue path is UTF-8"),
        ])
        .output()
        .expect("run schema generator in write mode");
    assert!(
        !output.status.success(),
        "write mode must not overwrite accepted v1 snapshots"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("binding schema v1"));
    assert!(stderr.contains("v2.json"));
    assert!(stderr.contains("errors-v2.json"));
}

#[test]
fn selected_v2_write_mode_refuses_to_overwrite_current_contracts() {
    let output = Command::new(env!("CARGO_BIN_EXE_sc-observability-schema"))
        .args([
            "--output",
            selected_path("v2.json")
                .to_str()
                .expect("schema path is UTF-8"),
            "--errors-output",
            selected_path("errors-v2.json")
                .to_str()
                .expect("error catalogue path is UTF-8"),
        ])
        .output()
        .expect("run schema generator in write mode");
    assert!(
        !output.status.success(),
        "write mode must not overwrite selected v2 snapshots"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("binding schema v2"));
    assert!(stderr.contains("v3.json"));
    assert!(stderr.contains("errors-v3.json"));
}

#[test]
fn versioned_output_pair_selects_schema_id_metadata_and_drift_label() {
    let directory = TemporaryContractDirectory::new("selected-v2-output");
    let schema = directory.join("v2.json");
    let errors = directory.join("errors-v2.json");
    let generator = env!("CARGO_BIN_EXE_sc-observability-schema");

    let generated = Command::new(generator)
        .args([
            "--output",
            schema.to_str().expect("schema path is UTF-8"),
            "--errors-output",
            errors.to_str().expect("error catalogue path is UTF-8"),
        ])
        .output()
        .expect("generate selected v2 contract pair");
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let selected: Value = serde_json::from_slice(&fs::read(&schema).expect("read selected v2"))
        .expect("generated v2 is JSON");
    assert_eq!(
        selected["$id"],
        json!("https://sc-observability.dev/bindings/v2.json")
    );
    assert_eq!(selected["x-sc-bindings"]["schema_version"], json!(2));

    let checked = generator_check(&schema, &errors);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    fs::write(&schema, b"{}\n").expect("introduce selected v2 drift");
    let drift = generator_check(&schema, &errors);
    assert!(!drift.status.success(), "selected v2 drift must fail");
    let stderr = String::from_utf8_lossy(&drift.stderr);
    assert!(stderr.contains("binding schema v2"));
    assert!(stderr.contains(schema.to_str().expect("schema path is UTF-8")));
}

fn set_schema_version_to_v3(contract: &mut Value) {
    contract["x-sc-bindings"]["schema_version"] = json!(3);
}

fn remove_schema_defaults(contract: &mut Value) {
    contract["x-sc-bindings"]
        .as_object_mut()
        .expect("bindings metadata is an object")
        .remove("defaults");
}

fn change_error_catalogue_code(contract: &mut Value) {
    contract[0]["code"] = json!("SC_OBSERVABILITY_BINDING_CHANGED");
}

#[test]
fn selected_v2_contract_mismatches_name_contract_version_and_changed_field() {
    enum ChangedContract {
        Schema,
        Errors,
    }

    struct MismatchCase {
        name: &'static str,
        changed_contract: ChangedContract,
        mutate: fn(&mut Value),
        expected_label: &'static str,
        expected_pointer: &'static str,
    }

    for case in [
        MismatchCase {
            name: "changed schema version",
            changed_contract: ChangedContract::Schema,
            mutate: set_schema_version_to_v3,
            expected_label: "binding schema v2",
            expected_pointer: "/x-sc-bindings/schema_version",
        },
        MismatchCase {
            name: "missing schema field",
            changed_contract: ChangedContract::Schema,
            mutate: remove_schema_defaults,
            expected_label: "binding schema v2",
            expected_pointer: "/x-sc-bindings/defaults",
        },
        MismatchCase {
            name: "changed error catalogue code",
            changed_contract: ChangedContract::Errors,
            mutate: change_error_catalogue_code,
            expected_label: "binding error catalogue v2",
            expected_pointer: "/0/code",
        },
    ] {
        let directory = TemporaryContractDirectory::new(case.name);
        let schema = directory.join("v2.json");
        let errors = directory.join("errors-v2.json");
        fs::copy(selected_path("v2.json"), &schema).expect("copy selected schema");
        fs::copy(selected_path("errors-v2.json"), &errors).expect("copy selected errors");

        let changed_path = match case.changed_contract {
            ChangedContract::Schema => &schema,
            ChangedContract::Errors => &errors,
        };
        let mut changed: Value =
            serde_json::from_slice(&fs::read(changed_path).expect("read selected contract"))
                .expect("selected contract is JSON");
        (case.mutate)(&mut changed);
        let mut bytes = serde_json::to_vec_pretty(&changed).expect("serialize changed contract");
        bytes.push(b'\n');
        fs::write(changed_path, &bytes).expect("write changed contract");

        let output = generator_check(&schema, &errors);
        assert!(!output.status.success(), "{} must fail", case.name);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(case.expected_label),
            "{}: {stderr}",
            case.name
        );
        assert!(
            stderr.contains(case.expected_pointer),
            "{}: {stderr}",
            case.name
        );
        assert!(
            stderr.contains("new versioned snapshot"),
            "{}: {stderr}",
            case.name
        );
        assert_eq!(
            fs::read(changed_path).expect("read changed contract"),
            bytes,
            "{} must not rewrite the changed selected contract",
            case.name
        );
    }
}
#[test]
fn every_registered_type_agrees_with_serde_and_frozen_expectations() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../../conformance/v2/schema-cases.json")).unwrap();
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
        serde_json::from_str(include_str!("../../conformance/v2/conversion-cases.json")).unwrap();
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
