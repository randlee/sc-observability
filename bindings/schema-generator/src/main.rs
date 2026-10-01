//! Deterministic canonical schema compiler. Rust DTOs are the sole shape authority.
use sc_observability_dto::*;
use schemars::{
    JsonSchema,
    generate::{SchemaGenerator, SchemaSettings},
};
use serde_json::{Map, Value, json};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
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
type SchemaMap = Map<String, Value>;
const SCHEMA_REGENERATION_COMMAND: &str = "cargo run --locked --manifest-path bindings/schema-generator/Cargo.toml --bin sc-observability-schema -- --output bindings/schema/v1.json --errors-output bindings/schema/errors-v1.json";

fn definitions(output: bool) -> Result<(SchemaMap, SchemaMap), Box<dyn Error>> {
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
    register::<CanonicalDiagnosticDto>(&mut generator, &mut entries, "CanonicalDiagnosticDto")?;
    register::<CanonicalFailureDto>(&mut generator, &mut entries, "CanonicalFailureDto")?;
    register::<TraceContextV2Dto>(&mut generator, &mut entries, "TraceContextV2Dto")?;
    register::<SpanLinkDto>(&mut generator, &mut entries, "SpanLinkDto")?;
    register::<SpanKindDto>(&mut generator, &mut entries, "SpanKindDto")?;
    register::<AggregationTemporalityDto>(
        &mut generator,
        &mut entries,
        "AggregationTemporalityDto",
    )?;
    register::<HistogramPointDto>(&mut generator, &mut entries, "HistogramPointDto")?;
    register::<MetricValueDto>(&mut generator, &mut entries, "MetricValueDto")?;
    register::<MetricRecordDto>(&mut generator, &mut entries, "MetricRecordDto")?;
    register::<SpanStatusDto>(&mut generator, &mut entries, "SpanStatusDto")?;
    register::<SpanRecordDto>(&mut generator, &mut entries, "SpanRecordDto")?;
    register::<SpanEventDto>(&mut generator, &mut entries, "SpanEventDto")?;
    register::<SpanSignalDto>(&mut generator, &mut entries, "SpanSignalDto")?;
    register::<CanonicalWireEnvelope<AdmissionDto>>(
        &mut generator,
        &mut entries,
        "CanonicalWireEnvelopeAdmissionDto",
    )?;
    let mut defs = generator.take_definitions(true);
    // Public schema names survive internal aliases to shared generic DTOs.
    for (entrypoint, public_name) in [
        ("CanonicalFailureDto", "CanonicalFailureDto"),
        ("CanonicalWireEnvelopeAdmissionDto", "CanonicalWireEnvelope"),
    ] {
        let reference = entries[entrypoint]["$ref"]
            .as_str()
            .ok_or_else(|| format!("missing definition reference for {entrypoint}"))?
            .to_owned();
        let generated_name = reference
            .strip_prefix("#/$defs/")
            .ok_or_else(|| format!("entrypoint {entrypoint}: non-local reference {reference:?}"))?;
        if generated_name == public_name {
            continue;
        }
        if defs.contains_key(public_name) {
            return Err(format!("public schema name collision: {public_name}").into());
        }
        let definition = defs
            .remove(generated_name)
            .ok_or_else(|| format!("missing definition: {generated_name}"))?;
        defs.insert(public_name.into(), definition);
        let public_reference = format!("#/$defs/{public_name}");
        for node in defs.values_mut().chain(entries.values_mut()) {
            rename_reference(node, &reference, &public_reference);
        }
    }
    Ok((defs, entries))
}
fn rename_reference(value: &mut Value, old: &str, new: &str) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "$ref" && child.as_str() == Some(old) {
                    *child = Value::String(new.into());
                } else {
                    rename_reference(child, old, new);
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                rename_reference(child, old, new);
            }
        }
        _ => {}
    }
}
fn input_strict(value: &mut Value, output: bool) {
    if let Some(map) = value.as_object_mut()
        && map.remove("x-sc-input-strict") == Some(Value::Bool(true))
        && !output
    {
        if let Some(Value::Array(variants)) = map.get_mut("oneOf") {
            for variant in variants {
                variant["additionalProperties"] = Value::Bool(false);
            }
        } else {
            map.insert("additionalProperties".into(), Value::Bool(false));
        }
    }
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

fn prefix_definition_refs(
    definition_name: &str,
    value: &mut Value,
    prefix: &str,
) -> Result<(), Box<dyn Error>> {
    prefix_refs(value, prefix)
        .map_err(|error| format!("schema definition {definition_name}: {error}").into())
}

fn supported(node: &Value, defs: &Map<String, Value>) -> Result<(), Box<dyn Error>> {
    if node.is_boolean() {
        return Ok(());
    }
    let object = node
        .as_object()
        .ok_or("schema node must be object or boolean")?;
    const KEYS: &[&str] = &[
        "$schema",
        "$id",
        "$defs",
        "$ref",
        "title",
        "description",
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "enum",
        "const",
        "oneOf",
        "anyOf",
        "allOf",
        "default",
        "format",
        "minimum",
        "maximum",
        "pattern",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
        "x-sc-integer-domain",
    ];
    for (key, value) in object {
        if !KEYS.contains(&key.as_str()) {
            return Err(format!("unsupported schema keyword: {key}").into());
        }
        match key.as_str() {
            "$ref" => {
                let reference = value
                    .as_str()
                    .and_then(|r| r.strip_prefix("#/$defs/"))
                    .ok_or("non-local reference")?;
                if !defs.contains_key(reference) {
                    return Err(format!("unresolved reference: {reference}").into());
                }
            }
            "properties" | "$defs" => {
                for child in value.as_object().ok_or("invalid property map")?.values() {
                    supported(child, defs)?;
                }
            }
            "items" | "additionalProperties" => supported(value, defs)?,
            "oneOf" | "anyOf" | "allOf" => {
                for child in value.as_array().ok_or("invalid schema union")? {
                    supported(child, defs)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_named_schema(
    name: &str,
    node: &Value,
    defs: &Map<String, Value>,
) -> Result<(), Box<dyn Error>> {
    supported(node, defs).map_err(|error| format!("schema {name}: {error}").into())
}

fn insert_definition(
    defs: &mut SchemaMap,
    name: String,
    value: Value,
) -> Result<(), Box<dyn Error>> {
    if defs.insert(name.clone(), value).is_some() {
        return Err(format!("duplicate schema name: {name}").into());
    }
    Ok(())
}

fn canonical(value: &Value) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Debug)]
struct SchemaReadError {
    path: PathBuf,
    source: std::io::Error,
}

impl std::fmt::Display for SchemaReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unable to read `{}`: {}; regenerate with `{SCHEMA_REGENERATION_COMMAND}`",
            self.path.display(),
            self.source
        )
    }
}

impl Error for SchemaReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

fn write_or_check(path: &Path, bytes: &[u8], check: bool) -> Result<(), Box<dyn Error>> {
    if check {
        let existing = std::fs::read(path).map_err(|source| SchemaReadError {
            path: path.to_path_buf(),
            source,
        })?;
        if existing != bytes {
            return Err(generated_drift_error(path).into());
        }
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

fn generated_drift_error(path: &Path) -> String {
    format!(
        "generated drift: {}; regenerate with `{SCHEMA_REGENERATION_COMMAND}`",
        path.display()
    )
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
            let definition_name = format!("{prefix}{name}");
            input_strict(&mut value, is_output);
            prefix_definition_refs(&definition_name, &mut value, prefix)?;
            insert_definition(&mut defs, definition_name, value)?;
        }
        for (name, mut value) in entries {
            let entrypoint_name = format!("{prefix}{name}");
            prefix_definition_refs(&entrypoint_name, &mut value, prefix)?;
            entrypoints.insert(entrypoint_name, value);
        }
    }
    for (name, node) in defs.iter().chain(entrypoints.iter()) {
        validate_named_schema(name, node, &defs)?;
    }
    let registry = serde_json::to_value(error_codes::REGISTRY)?;
    let schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://sc-observability.dev/bindings/v1.json","$defs":defs,"x-sc-entrypoints":entrypoints,"x-sc-error-registry":registry,"x-sc-bindings":{"schema_version":1,"integer":{"event_min":"-9223372036854775808","max":"18446744073709551615","counter_min":"0","canonical_pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)(?![\\s\\S])"},"limits":{"request_bytes":constants::MAX_WIRE_PAYLOAD_BYTES,"container_depth":constants::MAX_CONTAINER_DEPTH,"query_limit":constants::MAX_QUERY_LIMIT,"timeout_ms":constants::MAX_TIMEOUT_MS,"diagnostic_string_bytes":constants::MAX_DIAGNOSTIC_FIELD_BYTES,"remediation_steps":constants::MAX_REMEDIATION_STEPS},"defaults":{"query_limit":constants::DEFAULT_QUERY_LIMIT,"query_order":"oldest_first"},"reserved_field_namespace":"sc_observability.binding.","generic_projections":[{"name":"Result","source":"OutputResultDtoAdmissionDto","parameter_ref":"OutputAdmissionDto"},{"name":"WireEnvelope","source":"OutputWireEnvelopeAdmissionDto","parameter_ref":"OutputAdmissionDto"}],"operations":{"try_log":{"input":"InputTryLogRequest","output":"OutputWireEnvelopeAdmissionDto"},"query":{"input":"InputQueryRequest","output":"OutputWireEnvelopeLogSnapshotDto"},"health":{"input":"InputHealthRequest","output":"OutputWireEnvelopeLogHealthDto"},"flush":{"input":"InputFlushRequest","output":"OutputWireEnvelopeCompletionDto"},"change_level":{"input":"InputLevelChangeRequest","output":"OutputWireEnvelopeLevelChangeDto"}}}});
    write_or_check(Path::new(&output), &canonical(&schema)?, check)?;
    write_or_check(
        Path::new(&errors_output),
        &canonical(&schema["x-sc-error-registry"])?,
        check,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_errors_include_the_schema_definition_name() {
        for (reference, expected) in [
            (Value::Null, "non-string reference"),
            (
                Value::String("https://example.com/schema".into()),
                "non-local reference",
            ),
        ] {
            let mut schema = json!({"$ref": reference});
            let error = prefix_definition_refs("InputExample", &mut schema, "Input")
                .expect_err("invalid reference should fail")
                .to_string();
            assert!(error.contains("schema definition InputExample"));
            assert!(error.contains(expected));
        }
    }

    #[test]
    fn invalid_schema_errors_include_the_schema_name() {
        let error = validate_named_schema("InputExample", &json!("invalid"), &SchemaMap::new())
            .expect_err("scalar schema nodes are unsupported")
            .to_string();
        assert!(error.contains("schema InputExample"));
        assert!(error.contains("schema node must be object or boolean"));
    }

    #[test]
    fn duplicate_schema_error_includes_the_duplicate_name() {
        let mut defs = SchemaMap::new();
        insert_definition(&mut defs, "InputExample".into(), json!({"type":"object"})).unwrap();
        let error = insert_definition(&mut defs, "InputExample".into(), json!({"type":"string"}))
            .expect_err("duplicate schema names should fail")
            .to_string();
        assert!(error.contains("duplicate schema name: InputExample"));
    }

    #[test]
    fn drift_error_includes_the_schema_regeneration_command() {
        let error = generated_drift_error(Path::new("bindings/schema/v1.json"));
        assert!(error.contains("bindings/schema/v1.json"));
        assert!(error.contains(SCHEMA_REGENERATION_COMMAND));
    }

    #[test]
    fn missing_checked_schema_reports_path_hint_and_source() {
        let path = std::env::temp_dir().join(format!(
            "sc-observability-schema-missing-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time is after the Unix epoch")
                .as_nanos()
        ));

        let error = write_or_check(&path, b"{}\n", true)
            .expect_err("checking a missing generated file should fail");
        let message = error.to_string();

        assert!(message.contains(&path.display().to_string()));
        assert!(message.contains(SCHEMA_REGENERATION_COMMAND));
        let source = error
            .source()
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .expect("the read error should remain in the error source chain");
        assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
    }
}
