//! Deterministic canonical schema compiler. Rust DTOs are the sole shape authority.
use sc_observability_dto::*;
use schemars::{
    JsonSchema,
    generate::{SchemaGenerator, SchemaSettings},
};
use serde_json::{Map, Value, json};
use std::{error::Error, path::Path};
fn register<T: JsonSchema>(
    g: &mut SchemaGenerator,
    entries: &mut Map<String, Value>,
    name: &str,
) -> Result<(), Box<dyn Error>> {
    if entries
        .insert(name.into(), serde_json::to_value(g.subschema_for::<T>())?)
        .is_some()
    {
        return Err(format!("duplicate public name: {name}").into());
    }
    Ok(())
}
fn definitions(output: bool) -> Result<(Map<String, Value>, Map<String, Value>), Box<dyn Error>> {
    let settings = SchemaSettings::draft2020_12();
    let mut generator = (if output {
        settings.for_serialize()
    } else {
        settings.for_deserialize()
    })
    .into_generator();
    let mut entries = Map::new();
    register::<DecimalDto>(&mut generator, &mut entries, "DecimalDto")?;
    register::<LevelDto>(&mut generator, &mut entries, "LevelDto")?;
    register::<LevelFilterDto>(&mut generator, &mut entries, "LevelFilterDto")?;
    register::<LevelChangeSourceDto>(&mut generator, &mut entries, "LevelChangeSourceDto")?;
    register::<AvailabilityDto>(&mut generator, &mut entries, "AvailabilityDto")?;
    register::<WorkerStateDto>(&mut generator, &mut entries, "WorkerStateDto")?;
    register::<QueryStateDto>(&mut generator, &mut entries, "QueryStateDto")?;
    register::<LifecycleDto>(&mut generator, &mut entries, "LifecycleDto")?;
    register::<LogOrderDto>(&mut generator, &mut entries, "LogOrderDto")?;
    register::<PathDto>(&mut generator, &mut entries, "PathDto")?;
    register::<ValueDto>(&mut generator, &mut entries, "ValueDto")?;
    register::<RemediationDto>(&mut generator, &mut entries, "RemediationDto")?;
    register::<TraceContextDto>(&mut generator, &mut entries, "TraceContextDto")?;
    register::<ProcessIdentityDto>(&mut generator, &mut entries, "ProcessIdentityDto")?;
    register::<StateTransitionDto>(&mut generator, &mut entries, "StateTransitionDto")?;
    register::<StoredDiagnosticDto>(&mut generator, &mut entries, "StoredDiagnosticDto")?;
    register::<LogEventDto>(&mut generator, &mut entries, "LogEventDto")?;
    register::<StoredEventDto>(&mut generator, &mut entries, "StoredEventDto")?;
    register::<FieldMatchDto>(&mut generator, &mut entries, "FieldMatchDto")?;
    register::<LogQueryDto>(&mut generator, &mut entries, "LogQueryDto")?;
    register::<LogSnapshotDto>(&mut generator, &mut entries, "LogSnapshotDto")?;
    register::<DiagnosticSummaryDto>(&mut generator, &mut entries, "DiagnosticSummaryDto")?;
    register::<Diagnostic>(&mut generator, &mut entries, "Diagnostic")?;
    register::<SinkHealthDto>(&mut generator, &mut entries, "SinkHealthDto")?;
    register::<QueryHealthDto>(&mut generator, &mut entries, "QueryHealthDto")?;
    register::<MaintenanceHealthDto>(&mut generator, &mut entries, "MaintenanceHealthDto")?;
    register::<LoggingHealthDto>(&mut generator, &mut entries, "LoggingHealthDto")?;
    register::<DropCountsDto>(&mut generator, &mut entries, "DropCountsDto")?;
    register::<LevelStateDto>(&mut generator, &mut entries, "LevelStateDto")?;
    register::<BridgeHealthDto>(&mut generator, &mut entries, "BridgeHealthDto")?;
    register::<LogHealthDto>(&mut generator, &mut entries, "LogHealthDto")?;
    register::<DispatchDto>(&mut generator, &mut entries, "DispatchDto")?;
    register::<AdmissionDto>(&mut generator, &mut entries, "AdmissionDto")?;
    register::<CompletionDto>(&mut generator, &mut entries, "CompletionDto")?;
    register::<ChangeDiagnosticDto>(&mut generator, &mut entries, "ChangeDiagnosticDto")?;
    register::<LevelChangeDto>(&mut generator, &mut entries, "LevelChangeDto")?;
    register::<LevelRequestDto>(&mut generator, &mut entries, "LevelRequestDto")?;
    register::<Failure>(&mut generator, &mut entries, "Failure")?;
    register::<TryLogRequest>(&mut generator, &mut entries, "TryLogRequest")?;
    register::<QueryRequest>(&mut generator, &mut entries, "QueryRequest")?;
    register::<HealthRequest>(&mut generator, &mut entries, "HealthRequest")?;
    register::<FlushRequest>(&mut generator, &mut entries, "FlushRequest")?;
    register::<LevelChangeRequest>(&mut generator, &mut entries, "LevelChangeRequest")?;
    register::<ResultDto<AdmissionDto>>(&mut generator, &mut entries, "ResultDtoAdmissionDto")?;
    register::<ResultDto<CompletionDto>>(&mut generator, &mut entries, "ResultDtoCompletionDto")?;
    register::<ResultDto<DispatchDto>>(&mut generator, &mut entries, "ResultDtoDispatchDto")?;
    register::<ResultDto<LogSnapshotDto>>(&mut generator, &mut entries, "ResultDtoLogSnapshotDto")?;
    register::<ResultDto<LogHealthDto>>(&mut generator, &mut entries, "ResultDtoLogHealthDto")?;
    register::<ResultDto<LevelChangeDto>>(&mut generator, &mut entries, "ResultDtoLevelChangeDto")?;
    register::<WireEnvelope<AdmissionDto>>(
        &mut generator,
        &mut entries,
        "WireEnvelopeAdmissionDto",
    )?;
    register::<WireEnvelope<CompletionDto>>(
        &mut generator,
        &mut entries,
        "WireEnvelopeCompletionDto",
    )?;
    register::<WireEnvelope<DispatchDto>>(&mut generator, &mut entries, "WireEnvelopeDispatchDto")?;
    register::<WireEnvelope<LogSnapshotDto>>(
        &mut generator,
        &mut entries,
        "WireEnvelopeLogSnapshotDto",
    )?;
    register::<WireEnvelope<LogHealthDto>>(
        &mut generator,
        &mut entries,
        "WireEnvelopeLogHealthDto",
    )?;
    register::<WireEnvelope<LevelChangeDto>>(
        &mut generator,
        &mut entries,
        "WireEnvelopeLevelChangeDto",
    )?;
    register::<OperationDiagnosticDto>(&mut generator, &mut entries, "OperationDiagnosticDto")?;
    register::<ClientOutcome>(&mut generator, &mut entries, "ClientOutcome")?;
    register::<ClientStatus>(&mut generator, &mut entries, "ClientStatus")?;
    register::<FailureCountsDto>(&mut generator, &mut entries, "FailureCountsDto")?;
    register::<LogOperationDto>(&mut generator, &mut entries, "LogOperationDto")?;
    register::<AdmissionOperationDto>(&mut generator, &mut entries, "AdmissionOperationDto")?;
    register::<CompletionOperationDto>(&mut generator, &mut entries, "CompletionOperationDto")?;
    register::<ResultDto<ClientOutcome>>(&mut generator, &mut entries, "ResultDtoClientOutcome")?;
    register::<ResultDto<ClientStatus>>(&mut generator, &mut entries, "ResultDtoClientStatus")?;
    register::<WireEnvelope<ClientOutcome>>(
        &mut generator,
        &mut entries,
        "WireEnvelopeClientOutcome",
    )?;
    register::<WireEnvelope<ClientStatus>>(
        &mut generator,
        &mut entries,
        "WireEnvelopeClientStatus",
    )?;
    Ok((generator.take_definitions(true), entries))
}
fn prefix_refs(value: &mut Value, prefix: &str) -> Result<(), Box<dyn Error>> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "$ref" {
                    let reference = value.as_str().ok_or("non-string reference")?;
                    let name = reference
                        .strip_prefix("#/$defs/")
                        .ok_or("non-local reference")?;
                    *value = Value::String(format!("#/$defs/{prefix}{name}"));
                } else {
                    prefix_refs(value, prefix)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                prefix_refs(value, prefix)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn canonical(value: &Value) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn write_or_check(path: &Path, bytes: &[u8], check: bool) -> Result<(), Box<dyn Error>> {
    if check {
        if std::fs::read(path)? != bytes {
            return Err(format!("generated drift: {}", path.display()).into());
        }
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut output = None;
    let mut errors_output = None;
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next(),
            "--errors-output" => errors_output = args.next(),
            "--check" => check = true,
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let output = output.ok_or("--output required")?;
    let errors_output = errors_output.ok_or("--errors-output required")?;
    let mut defs = Map::new();
    let mut entrypoints = Map::new();
    for (is_output, prefix) in [(false, "Input"), (true, "Output")] {
        let (definitions, entries) = definitions(is_output)?;
        for (name, mut value) in definitions {
            prefix_refs(&mut value, prefix)?;
            if defs.insert(format!("{prefix}{name}"), value).is_some() {
                return Err("duplicate schema name".into());
            }
        }
        for (name, mut value) in entries {
            prefix_refs(&mut value, prefix)?;
            entrypoints.insert(format!("{prefix}{name}"), value);
        }
    }
    let registry = serde_json::to_value(error_codes::REGISTRY)?;
    let schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://sc-observability.dev/bindings/v1.json","$defs":defs,"x-sc-entrypoints":entrypoints,"x-sc-error-registry":registry,"x-sc-bindings":{"schema_version":1,"integer":{"event_min":"-9223372036854775808","max":"18446744073709551615","counter_min":"0","canonical_pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)$"},"limits":{"request_bytes":65536,"container_depth":32,"query_limit":1000,"timeout_ms":60000,"diagnostic_string_bytes":4096,"remediation_steps":32},"defaults":{"query_limit":100,"query_order":"oldest_first"},"reserved_field_namespace":"sc_observability.binding.","generic_projections":[{"name":"Result","source":"OutputResultDtoAdmissionDto","parameter_ref":"OutputAdmissionDto"},{"name":"WireEnvelope","source":"OutputWireEnvelopeAdmissionDto","parameter_ref":"OutputAdmissionDto"}],"operations":{"try_log":{"input":"InputTryLogRequest","output":"OutputWireEnvelopeAdmissionDto"},"query":{"input":"InputQueryRequest","output":"OutputWireEnvelopeLogSnapshotDto"},"health":{"input":"InputHealthRequest","output":"OutputWireEnvelopeLogHealthDto"},"flush":{"input":"InputFlushRequest","output":"OutputWireEnvelopeCompletionDto"},"change_level":{"input":"InputLevelChangeRequest","output":"OutputWireEnvelopeLevelChangeDto"}}}});
    write_or_check(Path::new(&output), &canonical(&schema)?, check)?;
    write_or_check(
        Path::new(&errors_output),
        &canonical(&schema["x-sc-error-registry"])?,
        check,
    )?;
    Ok(())
}
