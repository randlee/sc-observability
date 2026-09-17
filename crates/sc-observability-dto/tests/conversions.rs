use sc_observability_dto::*;
use sc_observability_types as core;
use serde_json::{Value, json};
fn event() -> Value {
    json!({"schema_version":1,"level":"info","target":"test","action":"emit"})
}
fn stamp() -> EventStamp {
    EventStamp {
        service: core::ServiceName::new("test").unwrap(),
        timestamp: core::Timestamp::UNIX_EPOCH,
        identity: core::ProcessIdentity::default(),
    }
}
#[test]
fn decimal_domains_are_canonical() {
    for value in [
        "0",
        "-9223372036854775808",
        "9223372036854775807",
        "18446744073709551615",
    ] {
        assert_eq!(DecimalDto::new(value).unwrap().as_str(), value);
    }
    for value in [
        "",
        "-0",
        "01",
        "-01",
        "+1",
        "1.0",
        " 1",
        "١",
        "18446744073709551616",
        "-9223372036854775809",
    ] {
        assert!(DecimalDto::new(value).is_err(), "{value}");
    }
    assert!(DecimalDto::new("-1").unwrap().as_u64().is_err());
    assert!(serde_json::from_value::<DecimalDto>(json!(1)).is_err());
}
#[test]
fn checked_event_preserves_integer_and_host_stamp() {
    let mut raw = event();
    raw["fields"] = json!({"n":{"kind":"integer","value":"18446744073709551615"}});
    let native = to_core_event(decode_event(raw).unwrap(), stamp()).unwrap();
    assert_eq!(native.timestamp, core::Timestamp::UNIX_EPOCH);
    assert_eq!(native.fields["n"], json!(u64::MAX));
    let stored = from_core_event(native).unwrap();
    assert_eq!(
        stored.fields["n"],
        ValueDto::Integer {
            value: u64::MAX.into()
        }
    );
    assert!(stored.diagnostic.is_none());
    assert_eq!(stored.service, "test");
}
#[test]
fn inputs_reject_missing_unknown_and_invalid_versions() {
    let mut raw = event();
    raw["extra"] = json!(true);
    assert!(matches!(decode_event(raw), Err(Failure::Validation { .. })));
    let mut raw = event();
    raw.as_object_mut().unwrap().remove("level");
    assert!(decode_event(raw).is_err());
    let mut raw = event();
    raw["schema_version"] = json!(2);
    assert!(matches!(
        decode_event(raw),
        Err(Failure::UnsupportedVersion { received: 2, .. })
    ));
    let mut raw = event();
    raw["level"] = json!("fatal");
    assert!(decode_event(raw).is_err());
    assert!(decode_level_request(json!({"kind":"reset","level":"trace"})).is_err());
}
#[test]
fn spoofed_provenance_is_rejected_at_all_depths() {
    for key in [
        "sc_observability.binding.language",
        "sc_observability.binding.any",
        "sc_observability::binding::channel",
        "sc-observability.binding.language",
        "sc observability.binding.channel",
    ] {
        if key == "sc-observability.binding.language" {
            continue;
        }
        for nested in [false, true] {
            let mut fields = json!({key:{"kind":"string","value":"forged"},"safe":{"kind":"null"}});
            if nested {
                fields = json!({"nested":{"kind":"object","value":fields}});
            }
            let mut raw = event();
            raw["fields"] = fields;
            assert!(
                matches!(decode_event(raw), Err(Failure::Validation { .. })),
                "{key}"
            );
        }
    }
}
#[test]
fn query_defaults_and_inclusive_bounds() {
    let query = decode_query(
        json!({"schema_version":1,"since":"1970-01-01T00:00:00Z","until":"1970-01-01T00:00:00Z"}),
    )
    .unwrap();
    assert_eq!(query.limit, 100);
    assert!(query.levels.is_empty());
    let native = to_core_query(query).unwrap();
    assert_eq!(native.since, native.until);
    for raw in [
        json!({"schema_version":1,"limit":0}),
        json!({"schema_version":1,"limit":1001}),
        json!({"schema_version":1,"since":"1970-01-02T00:00:00Z","until":"1970-01-01T00:00:00Z"}),
        json!({"schema_version":1,"since":"1970-01-01T01:00:00+01:00"}),
        json!({"schema_version":1,"field_matches":[{"field":"","value":{"kind":"null"}}]}),
    ] {
        assert!(decode_query(raw).is_err());
    }
    let query=decode_query(json!({"schema_version":1,"limit":1000,"field_matches":[{"field":"sc_observability.binding.language","value":{"kind":"string","value":"python"}}]})).unwrap();
    assert_eq!(query.limit, 1000);
}
#[test]
fn timeouts_are_bounded_integers() {
    for value in [0, 60000] {
        assert_eq!(decode_timeout(json!(value)).unwrap(), value);
    }
    for value in [
        json!(true),
        json!(-1),
        json!(60001),
        json!(1.5),
        json!(null),
    ] {
        assert!(decode_timeout(value).is_err());
    }
}
#[test]
fn nonfinite_typed_values_cannot_bypass_input_checks() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut dto = decode_event(event()).unwrap();
        dto.fields.insert("x".into(), ValueDto::Float { value });
        assert!(to_core_event(dto, stamp()).is_err());
    }
}
#[test]
fn diagnostics_preserve_unknown_codes_order_and_original_time() {
    let original = core::OperationDiagnostic {
        at: core::Timestamp::UNIX_EPOCH,
        code: core::ErrorCode::new_owned("FUTURE_NATIVE_CODE"),
        message: "original".into(),
        remediation: core::Remediation::Recoverable {
            steps: core::RecoverableSteps::all(["second", "first"]),
        },
    };
    let failure = from_level_error(core::LevelChangeError::Unavailable {
        diagnostic: original,
    });
    assert_eq!(failure.diagnostic().at, "1970-01-01T00:00:00Z");
    assert_eq!(failure.diagnostic().code, "FUTURE_NATIVE_CODE");
    assert_eq!(
        failure.diagnostic().remediation,
        RemediationDto::Recoverable {
            steps: vec!["second".into(), "first".into()]
        }
    );
    let state = core::LevelState {
        configured_level: core::LevelFilter::Info,
        effective_level: core::LevelFilter::Trace,
        revision: u64::MAX,
    };
    let change = from_level_change(core::LevelChange::Unchanged { state }).unwrap();
    assert_eq!(
        serde_json::to_value(change).unwrap()["state"]["level_revision"],
        u64::MAX.to_string()
    );
}
#[test]
fn malformed_unknown_and_oversized_remote_errors_differ() {
    let diagnostic = json!({"kind":"future_kind","at":"1970-01-01T00:00:00Z","code":"FUTURE","message":"future","remediation":{"kind":"recoverable","steps":[]}});
    let envelope = json!({"schema_version":1,"kind":"error","error":diagnostic,"new_output":true});
    assert!(
        matches!(decode_envelope::<AdmissionDto>(envelope.clone()).unwrap(),WireEnvelope::Error{error:Failure::UnknownRemote{remote_kind,..},..} if remote_kind=="future_kind")
    );
    let mut malformed = envelope.clone();
    malformed["error"]["kind"] = json!("validation");
    assert!(
        matches!(decode_envelope::<AdmissionDto>(malformed),Err(Failure::Validation{field,..}) if field=="response")
    );
    let mut oversized = envelope;
    oversized["error"]["message"] = json!("x".repeat(4097));
    assert!(
        matches!(decode_envelope::<AdmissionDto>(oversized),Err(Failure::Validation{diagnostic,field}) if field=="response.error" && diagnostic.code==error_codes::SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE)
    );
}
#[test]
fn paths_keep_absence_and_non_unicode() {
    assert_eq!(from_path(None), PathDto::Absent);
    assert!(to_path("", std::path::Path::new("/tmp")).is_err());
    assert!(to_path("a\0b", std::path::Path::new("/tmp")).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![255]));
        assert_eq!(from_path(Some(&path)), PathDto::Unrepresentable);
    }
}
#[test]
fn registry_has_unique_literals_and_exact_remediation() {
    let mut codes = std::collections::BTreeSet::new();
    assert_eq!(error_codes::REGISTRY.len(), 17);
    for entry in error_codes::REGISTRY {
        assert!(codes.insert(entry.code));
        let diagnostic = boundary_diagnostic(entry.code, "test");
        assert_eq!(
            diagnostic.remediation,
            RemediationDto::Recoverable {
                steps: vec![entry.remediation.into()]
            }
        );
        validate_diagnostic(&diagnostic, "test").unwrap();
    }
}
