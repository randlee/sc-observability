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

#[test]
fn exact_request_size_and_depth_boundaries() {
    let mut raw = event();
    raw["message"] = json!("");
    let overhead = serde_json::to_vec(&raw).unwrap().len();
    raw["message"] = json!("x".repeat(65536 - overhead));
    assert_eq!(serde_json::to_vec(&raw).unwrap().len(), 65536);
    let dto = decode_event(raw.clone()).unwrap();
    assert!(to_core_event(dto, stamp()).is_ok());
    let message = raw["message"].as_str().unwrap().to_owned();
    raw["message"] = json!(message + "x");
    assert!(decode_event(raw).is_err());
    let mut value = json!({"kind":"array","value":[]});
    for _ in 0..14 {
        value = json!({"kind":"array","value":[value]});
    }
    let mut raw = event();
    raw["fields"] = json!({"depth":value});
    assert!(decode_event(raw.clone()).is_ok());
    raw["fields"]["depth"] = json!({"kind":"array","value":[raw["fields"]["depth"].clone()]});
    assert!(decode_event(raw).is_err());
}

#[test]
fn all_stored_event_fields_and_trusted_output_survive() {
    let mut native = to_core_event(decode_event(event()).unwrap(), stamp()).unwrap();
    native.identity = core::ProcessIdentity {
        hostname: Some("host".into()),
        pid: Some(u32::MAX),
    };
    native.trace = Some(core::TraceContext {
        trace_id: core::TraceId::new("0123456789abcdef0123456789abcdef").unwrap(),
        span_id: core::SpanId::new("0123456789abcdef").unwrap(),
        parent_span_id: Some(core::SpanId::new("fedcba9876543210").unwrap()),
    });
    native.request_id = Some(core::CorrelationId::new("request").unwrap());
    native.correlation_id = Some(core::CorrelationId::new("correlation").unwrap());
    native.outcome = Some(core::OutcomeLabel::new("success").unwrap());
    native.message = Some("message".into());
    native.diagnostic = Some(core::Diagnostic {
        timestamp: core::Timestamp::UNIX_EPOCH,
        code: core::ErrorCode::new_owned("FUTURE_STORED"),
        message: "stored".into(),
        cause: Some("cause".into()),
        remediation: core::Remediation::Recoverable {
            steps: core::RecoverableSteps::all(["first", "second"]),
        },
        docs: Some("https://example.test".into()),
        details: serde_json::Map::from_iter([("maximum".into(), json!(u64::MAX))]),
    });
    native.state_transition = Some(core::StateTransition {
        entity_kind: core::TargetCategory::new("worker").unwrap(),
        entity_id: Some("worker-1".into()),
        from_state: core::StateName::new("idle").unwrap(),
        to_state: core::StateName::new("active").unwrap(),
        reason: Some("work".into()),
        trigger: Some(core::ActionName::new("start").unwrap()),
    });
    native
        .fields
        .insert("sc_observability.binding.language".into(), json!("rust"));
    let dto = from_core_event(native).unwrap();
    let value = serde_json::to_value(&dto).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 15);
    assert_eq!(value["identity"], json!({"hostname":"host","pid":u32::MAX}));
    assert_eq!(value["trace"]["parent_span_id"], "fedcba9876543210");
    assert_eq!(value["diagnostic"]["timestamp"], "1970-01-01T00:00:00Z");
    assert_eq!(value["diagnostic"]["code"], "FUTURE_STORED");
    assert_eq!(
        value["diagnostic"]["details"]["maximum"]["value"],
        u64::MAX.to_string()
    );
    assert_eq!(
        value["diagnostic"]["remediation"],
        json!({"kind":"recoverable","steps":["first","second"]})
    );
    assert_eq!(
        value["state_transition"],
        json!({"entity_kind":"worker","entity_id":"worker-1","from_state":"idle","to_state":"active","reason":"work","trigger":"start"})
    );
    assert_eq!(
        serde_json::from_value::<StoredEventDto>(value).unwrap(),
        dto
    );
}

#[test]
fn complete_health_projection_and_unsigned_wire_counters() {
    let summary = core::DiagnosticSummary {
        code: None,
        message: "summary".into(),
        at: core::Timestamp::UNIX_EPOCH,
    };
    let native = core::LoggingHealthReport {
        state: core::LoggingHealthState::DegradedDropping,
        dropped_events_total: u64::MAX,
        flush_errors_total: 2,
        active_log_path: "active.jsonl".into(),
        sink_statuses: vec![core::SinkHealth {
            name: core::SinkName::new("file").unwrap(),
            state: core::SinkHealthState::Unavailable,
            last_error: Some(summary.clone()),
        }],
        queue_depth: 3,
        queue_capacity: 4,
        queue_high_water_mark: 5,
        queue_full_drops_total: 6,
        writer_state: core::WriterState::Degraded,
        last_writer_error: Some(summary.clone()),
        query: Some(core::QueryHealthReport {
            state: core::QueryHealthState::Degraded,
            last_error: Some(summary.clone()),
        }),
        maintenance: Some(core::MaintenanceHealthReport {
            state: core::MaintenanceWorkerState::Stopped,
            last_pass_at: Some(core::Timestamp::UNIX_EPOCH),
            rotated_files_total: core::FileCount::from_usize(7),
            pruned_files_total: core::FileCount::from_usize(8),
            last_error: Some(summary.clone()),
        }),
        last_error: Some(summary),
    };
    let dto = from_core_health(
        native,
        core::LevelState {
            configured_level: core::LevelFilter::Info,
            effective_level: core::LevelFilter::Debug,
            revision: u64::MAX,
        },
    )
    .unwrap();
    let mut value = serde_json::to_value(&dto).unwrap();
    assert_eq!(value["logging"].as_object().unwrap().len(), 14);
    assert_eq!(
        value["logging"]["dropped_events_total"],
        u64::MAX.to_string()
    );
    assert_eq!(value["logging"]["maintenance"]["pruned_files_total"], "8");
    assert_eq!(
        value["logging"]["last_error"],
        json!({"at":"1970-01-01T00:00:00Z","code":null,"message":"summary"})
    );
    assert!(value["bridge"].is_null());
    assert_eq!(
        serde_json::from_value::<LogHealthDto>(value.clone()).unwrap(),
        dto
    );
    value["level_state"]["level_revision"] = json!("-1");
    assert!(serde_json::from_value::<LogHealthDto>(value).is_err());
}

#[test]
fn level_change_errors_and_unsuccessful_diagnostics_preserve_payloads() {
    assert!(matches!(
        from_level_error(core::LevelChangeError::Stopping),
        Failure::Closed { .. }
    ));
    assert!(matches!(
        from_level_error(core::LevelChangeError::Stopped),
        Failure::Closed { .. }
    ));
    assert!(matches!(
        from_level_error(core::LevelChangeError::BelowBaseline {
            requested: core::LevelFilter::Error,
            configured: core::LevelFilter::Info
        }),
        Failure::BelowBaseline {
            requested: LevelFilterDto::Error,
            configured: LevelFilterDto::Info,
            ..
        }
    ));
    assert!(matches!(
        from_level_error(core::LevelChangeError::UnsupportedLevel {
            requested: core::LevelFilter::Trace,
            available: core::LevelFilter::Debug
        }),
        Failure::UnsupportedLevel {
            requested: LevelFilterDto::Trace,
            available: LevelFilterDto::Debug,
            ..
        }
    ));
    let previous = core::LevelState {
        configured_level: core::LevelFilter::Info,
        effective_level: core::LevelFilter::Info,
        revision: 0,
    };
    let current = core::LevelState {
        effective_level: core::LevelFilter::Debug,
        revision: 1,
        ..previous
    };
    let original = core::OperationDiagnostic {
        code: core::ErrorCode::new_owned("ORIGINAL"),
        message: "failed admission".into(),
        at: core::Timestamp::UNIX_EPOCH,
        remediation: core::Remediation::not_recoverable("terminal"),
    };
    let dto = from_level_change(core::LevelChange::Changed {
        previous,
        current,
        source: core::LevelChangeSource::UserRequest,
        diagnostic: core::ChangeDiagnostic::NotAccepted {
            diagnostic: original,
        },
    })
    .unwrap();
    let value = serde_json::to_value(dto).unwrap();
    assert_eq!(value["diagnostic"]["kind"], "not_accepted");
    assert_eq!(value["diagnostic"]["diagnostic"]["code"], "ORIGINAL");
    assert_eq!(
        value["diagnostic"]["diagnostic"]["remediation"],
        json!({"kind":"not_recoverable","justification":"terminal"})
    );
}

#[test]
fn nested_output_additions_do_not_weaken_strict_inputs() {
    let mut raw = event();
    raw["fields"] = json!({"x":{"kind":"null","future":true}});
    assert!(decode_event(raw).is_err());
    let output: ValueDto = serde_json::from_value(json!({"kind":"null","future":true})).unwrap();
    assert_eq!(output, ValueDto::Null {});
    let mut raw = event();
    raw["trace"] = json!({"trace_id":"0123456789abcdef0123456789abcdef","span_id":"0123456789abcdef","future":true});
    assert!(decode_event(raw).is_err());
    let trace:TraceContextDto=serde_json::from_value(json!({"trace_id":"0123456789abcdef0123456789abcdef","span_id":"0123456789abcdef","parent_span_id":null,"future":true})).unwrap();
    assert!(trace.parent_span_id.is_none());
}

#[test]
fn diagnostic_string_and_step_bounds_are_exact_and_never_truncate() {
    let mut diagnostic = boundary_diagnostic(
        error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
        "é".repeat(2048),
    );
    diagnostic.remediation = RemediationDto::Recoverable {
        steps: vec!["step".into(); 32],
    };
    validate_diagnostic(&diagnostic, "response.error").unwrap();
    diagnostic.message.push('x');
    let failure = validate_diagnostic(&diagnostic, "response.error").unwrap_err();
    assert!(
        matches!(failure,Failure::Validation{diagnostic,field} if diagnostic.code==error_codes::SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE && field=="response.error")
    );
    diagnostic.message = "bounded".into();
    diagnostic.remediation = RemediationDto::Recoverable {
        steps: vec!["step".into(); 33],
    };
    assert_eq!(
        validate_diagnostic(&diagnostic, "response.error")
            .unwrap_err()
            .diagnostic()
            .code,
        error_codes::SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE
    );
    diagnostic.remediation = RemediationDto::Recoverable { steps: vec![] };
    validate_diagnostic(&diagnostic, "response.error").unwrap();
}
