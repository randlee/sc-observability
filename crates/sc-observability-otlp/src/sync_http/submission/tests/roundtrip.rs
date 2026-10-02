use super::super::*;
use super::capture::{CAPTURE_TIMEOUT, capture_server};
use super::proto_json::{
    NonFiniteDouble, decode_any_value, decode_base64, decode_forms, decode_key_values,
};
use crate::config::{
    ExporterBackend, LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, ResourceAttributes,
    SyncHttpRetryPolicy, TelemetryConfig, prepared_backend_connection,
    validated_released_telemetry_bounds,
};
use crate::constants::MAX_OTLP_ENCODED_REQUEST_BYTES;
use opentelemetry_proto::tonic::{
    logs::v1::ResourceLogs, metrics::v1::ResourceMetrics, trace::v1::ResourceSpans,
};
use sc_observability_types::{
    ServiceName, SpanId, Timestamp, TraceId,
    otlp::{
        signals::{
            AnyValue, Exemplar, Function, KeyValueAndUnit, KeyValues, Line, Location, Mapping,
            MetricData, NumberValue, OtlpDouble, ProfileLink, Sample, Stack, StringIndex,
            ValueType,
        },
        submission::{IdSource, Signal, SubmissionEnvelope},
    },
    v2::ExportError,
};
use serde_json::Value;
use std::net::TcpListener;

struct Ids;
impl IdSource for Ids {
    fn trace_id(&mut self) -> TraceId {
        TraceId::new("0123456789abcdef0123456789abcdef").expect("trace id")
    }
    fn span_id(&mut self) -> SpanId {
        SpanId::new("0123456789abcdef").expect("span id")
    }
    fn now(&mut self) -> Timestamp {
        Timestamp::UNIX_EPOCH
    }
}

fn fixture(name: &str) -> SubmissionEnvelope {
    let input = golden_fixture(name, "input.json");
    let envelope = SubmissionEnvelope::from_json(&input, &mut Ids).expect("canonical envelope");
    let canonical = envelope.to_canonical_json();
    serde_json::from_str(&canonical).expect("store envelope parses")
}

fn envelope(input: &serde_json::Value) -> SubmissionEnvelope {
    SubmissionEnvelope::from_json(&input.to_string(), &mut Ids).expect("canonical envelope")
}

fn submission_exporter(
    endpoint: String,
    retry: Option<SyncHttpRetryPolicy>,
) -> SyncHttpSubmissionExporter {
    submission_exporter_with_shutdown(endpoint, retry, None)
}

fn submission_exporter_with_shutdown(
    endpoint: String,
    retry: Option<SyncHttpRetryPolicy>,
    shutdown_ms: Option<u64>,
) -> SyncHttpSubmissionExporter {
    let mut config = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
    config.enabled = true;
    config.lifecycle_shutdown_timeout_ms = shutdown_ms.map(Into::into);
    if let Some(shutdown) = shutdown_ms {
        config.timeout_ms = Some(shutdown.into());
        config.lifecycle_flush_timeout_ms = Some(shutdown.into());
    }
    config.endpoint = Some(OtlpEndpoint::new_typed(endpoint).expect("loopback endpoint"));
    config.sync_http_retry = retry;
    let telemetry = TelemetryConfig {
        service_name: ServiceName::new("submission-capture").expect("service name"),
        resource: ResourceAttributes::default(),
        transport: config,
        logs: Some(LogsConfig::default()),
        traces: None,
        metrics: None,
    };
    // The released compatibility preparation path explicitly permits immediate
    // retry delays, which makes these capture assertions independent of the
    // scheduler while retaining production retry behavior.
    let bounds = validated_released_telemetry_bounds(&telemetry).expect("valid test exporter");
    let connection = prepared_backend_connection(&telemetry.transport, &bounds)
        .expect("prepared test connection");
    let config = SyncHttpConfig::from_prepared(&connection, &bounds).expect("valid test exporter");
    let exporter = Arc::new(
        OtlpHttpExporter::from_prepared(config.clone(), &bounds)
            .expect("test exporter constructs eagerly"),
    );
    SyncHttpSubmissionExporter {
        config,
        bounds,
        exporter,
    }
}

fn retry_policy(max_retries: u32) -> SyncHttpRetryPolicy {
    SyncHttpRetryPolicy {
        max_retries: Some(max_retries),
        // Capture tests assert retry behavior directly, so keep their timing
        // independent of wall-clock scheduling.
        initial_backoff_ms: Some(0.into()),
        max_backoff_ms: Some(0.into()),
        retry_sequence_timeout_ms: Some(3_000.into()),
        retry_after_cap_ms: Some(20.into()),
        retry_jitter_percent: Some(0),
    }
}

fn json_u64(value: &Value, field: &str) -> u64 {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a proto-JSON integer string"))
        .parse()
        .unwrap_or_else(|error| panic!("{field} must be a u64: {error}"))
}

fn json_i64(value: &Value, field: &str) -> i64 {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a proto-JSON integer string"))
        .parse()
        .unwrap_or_else(|error| panic!("{field} must be an i64: {error}"))
}

fn json_usize(value: &Value, field: &str) -> usize {
    usize::try_from(
        value[field]
            .as_u64()
            .unwrap_or_else(|| panic!("{field} must be an index")),
    )
    .unwrap_or_else(|_| panic!("{field} must fit usize"))
}

fn index_usize(value: i32, field: &str) -> usize {
    usize::try_from(value).unwrap_or_else(|_| panic!("{field} must be a non-negative index"))
}

fn timestamp_nanos(value: Timestamp) -> u64 {
    u64::try_from(value.into_inner().unix_timestamp_nanos())
        .expect("test fixture timestamp must be non-negative")
}

fn assert_double(captured: &Value, expected: OtlpDouble) {
    let actual = match captured {
        Value::Number(value) => value.as_f64().expect("finite proto-JSON f64"),
        Value::String(value) if value == "NaN" => f64::NAN,
        Value::String(value) if value == "Infinity" => f64::INFINITY,
        Value::String(value) if value == "-Infinity" => f64::NEG_INFINITY,
        _ => panic!("OTLP double must be a number or a standard non-finite spelling"),
    };
    // Proto JSON deliberately collapses all NaN payloads to the `NaN` token.
    // Other doubles retain their exact IEEE-754 bit representation.
    if expected.get().is_nan() {
        assert!(
            actual.is_nan(),
            "the captured value must retain the NaN class"
        );
    } else {
        assert_eq!(actual.to_bits(), expected.get().to_bits());
    }
}

fn assert_number_value(captured: &Value, expected: &NumberValue) {
    match expected {
        NumberValue::Int(expected) => {
            assert_eq!(json_i64(captured, "asInt"), *expected);
            assert!(captured.get("asDouble").is_none());
        }
        NumberValue::Double(expected) => {
            assert_double(&captured["asDouble"], *expected);
            assert!(captured.get("asInt").is_none());
        }
        _ => unreachable!("new NumberValue variants require an explicit capture assertion"),
    }
}

fn assert_exemplars(captured: &Value, expected: &[Exemplar]) {
    let captured = captured.as_array().expect("captured exemplars");
    assert_eq!(captured.len(), expected.len());
    for (captured, expected) in captured.iter().zip(expected) {
        assert_eq!(
            decode_key_values(&captured["filteredAttributes"]).expect("decode exemplar attributes"),
            expected.filtered_attributes
        );
        assert_eq!(
            json_u64(captured, "timeUnixNano"),
            timestamp_nanos(expected.time)
        );
        assert_number_value(captured, &expected.value);
        assert_eq!(
            captured.get("traceId").and_then(Value::as_str),
            expected
                .trace_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref()
        );
        assert_eq!(
            captured.get("spanId").and_then(Value::as_str),
            expected
                .span_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref()
        );
    }
}

fn assert_common_metric_point(
    captured: &Value,
    attributes: &sc_observability_types::otlp::signals::KeyValues,
    start_time: Option<Timestamp>,
    time: Timestamp,
    flags: u32,
) {
    assert_eq!(
        decode_key_values(&captured["attributes"]).expect("decode metric attributes"),
        *attributes
    );
    assert_eq!(
        captured
            .get("startTimeUnixNano")
            .and_then(Value::as_str)
            .map(str::parse::<i128>)
            .transpose()
            .expect("start timestamp is numeric"),
        start_time.map(|value| value.into_inner().unix_timestamp_nanos())
    );
    assert_eq!(
        i128::from(json_u64(captured, "timeUnixNano")),
        time.into_inner().unix_timestamp_nanos()
    );
    assert_eq!(captured["flags"].as_u64(), Some(u64::from(flags)));
}

#[expect(
    clippy::too_many_lines,
    reason = "the test decoder keeps every metric wire field beside its canonical assertion"
)]
fn assert_captured_metric(
    captured: &Value,
    expected: &sc_observability_types::otlp::signals::MetricStream,
) {
    assert_eq!(captured["name"], expected.name.as_str());
    assert_eq!(
        captured.get("description").and_then(Value::as_str),
        expected.description.as_deref()
    );
    assert_eq!(
        captured.get("unit").and_then(Value::as_str),
        expected.unit.as_deref()
    );
    assert_eq!(
        captured
            .get("metadata")
            .map(|value| decode_key_values(value).expect("decode metadata")),
        (!expected.metadata.entries().is_empty()).then(|| expected.metadata.clone())
    );
    match &expected.data {
        MetricData::Gauge { points } => {
            let captured = captured["gauge"]["dataPoints"]
                .as_array()
                .expect("gauge points");
            assert_eq!(captured.len(), points.len());
            for (captured, expected) in captured.iter().zip(points) {
                assert_common_metric_point(
                    captured,
                    &expected.attributes,
                    expected.start_time,
                    expected.time,
                    expected.flags.bits(),
                );
                assert_number_value(captured, &expected.value);
                assert_exemplars(&captured["exemplars"], &expected.exemplars);
            }
        }
        MetricData::Sum {
            points,
            temporality,
            monotonic,
        } => {
            let sum = &captured["sum"];
            assert_eq!(
                sum["aggregationTemporality"].as_u64(),
                Some(match temporality {
                    sc_observability_types::otlp::signals::AggregationTemporality::Delta => 1,
                    sc_observability_types::otlp::signals::AggregationTemporality::Cumulative => 2,
                    _ => 0,
                })
            );
            assert_eq!(sum["isMonotonic"].as_bool(), Some(*monotonic));
            let captured = sum["dataPoints"].as_array().expect("sum points");
            assert_eq!(captured.len(), points.len());
            for (captured, expected) in captured.iter().zip(points) {
                assert_common_metric_point(
                    captured,
                    &expected.attributes,
                    expected.start_time,
                    expected.time,
                    expected.flags.bits(),
                );
                assert_number_value(captured, &expected.value);
                assert_exemplars(&captured["exemplars"], &expected.exemplars);
            }
        }
        MetricData::Histogram {
            points,
            temporality,
        } => {
            let histogram = &captured["histogram"];
            assert_eq!(
                histogram["aggregationTemporality"].as_u64(),
                Some(match temporality {
                    sc_observability_types::otlp::signals::AggregationTemporality::Delta => 1,
                    sc_observability_types::otlp::signals::AggregationTemporality::Cumulative => 2,
                    _ => 0,
                })
            );
            let captured = histogram["dataPoints"]
                .as_array()
                .expect("histogram points");
            assert_eq!(captured.len(), points.len());
            for (captured, expected) in captured.iter().zip(points) {
                assert_common_metric_point(
                    captured,
                    &expected.attributes,
                    expected.start_time,
                    expected.time,
                    expected.flags.bits(),
                );
                assert_eq!(json_u64(captured, "count"), expected.count);
                assert_optional_double(captured.get("sum"), expected.sum);
                assert_eq!(
                    captured["bucketCounts"]
                        .as_array()
                        .expect("buckets")
                        .iter()
                        .map(|value| value
                            .as_str()
                            .expect("bucket string")
                            .parse::<u64>()
                            .expect("bucket count"))
                        .collect::<Vec<_>>(),
                    expected.bucket_counts
                );
                assert_doubles(&captured["explicitBounds"], &expected.explicit_bounds);
                assert_exemplars(&captured["exemplars"], &expected.exemplars);
                assert_optional_double(captured.get("min"), expected.min);
                assert_optional_double(captured.get("max"), expected.max);
            }
        }
        MetricData::ExponentialHistogram {
            points,
            temporality,
        } => {
            let histogram = &captured["exponentialHistogram"];
            assert_eq!(
                histogram["aggregationTemporality"].as_u64(),
                Some(match temporality {
                    sc_observability_types::otlp::signals::AggregationTemporality::Delta => 1,
                    sc_observability_types::otlp::signals::AggregationTemporality::Cumulative => 2,
                    _ => 0,
                })
            );
            let captured = histogram["dataPoints"]
                .as_array()
                .expect("exponential histogram points");
            assert_eq!(captured.len(), points.len());
            for (captured, expected) in captured.iter().zip(points) {
                assert_common_metric_point(
                    captured,
                    &expected.attributes,
                    expected.start_time,
                    expected.time,
                    expected.flags.bits(),
                );
                assert_eq!(json_u64(captured, "count"), expected.count);
                assert_optional_double(captured.get("sum"), expected.sum);
                assert_eq!(captured["scale"].as_i64(), Some(i64::from(expected.scale)));
                assert_eq!(json_u64(captured, "zeroCount"), expected.zero_count);
                assert_double(&captured["zeroThreshold"], expected.zero_threshold);
                assert_buckets(&captured["positive"], &expected.positive);
                assert_buckets(&captured["negative"], &expected.negative);
                assert_exemplars(&captured["exemplars"], &expected.exemplars);
                assert_optional_double(captured.get("min"), expected.min);
                assert_optional_double(captured.get("max"), expected.max);
            }
        }
        MetricData::Summary { points } => {
            let captured = captured["summary"]["dataPoints"]
                .as_array()
                .expect("summary points");
            assert_eq!(captured.len(), points.len());
            for (captured, expected) in captured.iter().zip(points) {
                assert_common_metric_point(
                    captured,
                    &expected.attributes,
                    expected.start_time,
                    expected.time,
                    expected.flags.bits(),
                );
                assert_eq!(json_u64(captured, "count"), expected.count);
                assert_double(&captured["sum"], expected.sum);
                let quantiles = captured["quantileValues"].as_array().expect("quantiles");
                assert_eq!(quantiles.len(), expected.quantile_values.len());
                for (captured, expected) in quantiles.iter().zip(&expected.quantile_values) {
                    assert_double(&captured["quantile"], expected.quantile);
                    assert_double(&captured["value"], expected.value);
                }
            }
        }
        _ => unreachable!("new MetricData variants require a capture decoder"),
    }
}

fn assert_optional_double(captured: Option<&Value>, expected: Option<OtlpDouble>) {
    match (captured, expected) {
        (Some(captured), Some(expected)) => assert_double(captured, expected),
        (None, None) => {}
        _ => panic!("optional double presence must match the canonical envelope"),
    }
}

fn assert_doubles(captured: &Value, expected: &[OtlpDouble]) {
    let captured = captured.as_array().expect("captured doubles");
    assert_eq!(captured.len(), expected.len());
    for (captured, expected) in captured.iter().zip(expected) {
        assert_double(captured, *expected);
    }
}

fn assert_buckets(
    captured: &Value,
    expected: &sc_observability_types::otlp::signals::ExponentialBuckets,
) {
    assert_eq!(
        captured["offset"].as_i64(),
        Some(i64::from(expected.offset))
    );
    assert_eq!(
        captured["bucketCounts"]
            .as_array()
            .expect("bucket counts")
            .iter()
            .map(|value| value
                .as_str()
                .expect("bucket string")
                .parse::<u64>()
                .expect("bucket count"))
            .collect::<Vec<_>>(),
        expected.bucket_counts
    );
}

fn assert_captured_metrics(captured: &Value, envelopes: &[SubmissionEnvelope]) {
    let expected = envelopes
        .iter()
        .flat_map(|envelope| &envelope.metrics)
        .collect::<Vec<_>>();
    let captured = captured["resourceMetrics"]
        .as_array()
        .expect("resource metrics")
        .iter()
        .flat_map(|resource| resource["scopeMetrics"].as_array().expect("scope metrics"))
        .flat_map(|scope| scope["metrics"].as_array().expect("metrics"))
        .collect::<Vec<_>>();
    assert_eq!(captured.len(), expected.len());
    for (captured, expected) in captured.iter().zip(expected) {
        assert_captured_metric(captured, &expected.record);
    }
}

fn json_indices(value: &Value) -> Vec<i32> {
    value
        .as_array()
        .expect("captured index array")
        .iter()
        .map(|value| i32::try_from(value.as_i64().expect("captured i32 index")).expect("i32 index"))
        .collect()
}

#[expect(
    clippy::too_many_lines,
    reason = "the test decoder lists every pinned profiles dictionary field"
)]
fn assert_captured_dictionary(
    captured: &Value,
    expected: &sc_observability_types::otlp::signals::ProfilesDictionary,
) {
    let mappings = captured["mappingTable"].as_array().expect("mapping table");
    assert_eq!(mappings.len(), expected.mapping_table.len());
    for (captured, expected) in mappings.iter().zip(&expected.mapping_table) {
        assert_eq!(json_u64(captured, "memoryStart"), expected.memory_start);
        assert_eq!(json_u64(captured, "memoryLimit"), expected.memory_limit);
        assert_eq!(json_u64(captured, "fileOffset"), expected.file_offset);
        assert_eq!(
            captured["filenameStrindex"].as_i64(),
            Some(i64::from(expected.filename_strindex))
        );
        assert_eq!(
            json_indices(&captured["attributeIndices"]),
            expected.attribute_indices
        );
    }
    let locations = captured["locationTable"]
        .as_array()
        .expect("location table");
    assert_eq!(locations.len(), expected.location_table.len());
    for (captured, expected) in locations.iter().zip(&expected.location_table) {
        assert_eq!(
            captured["mappingIndex"].as_i64(),
            Some(i64::from(expected.mapping_index))
        );
        assert_eq!(json_u64(captured, "address"), expected.address);
        assert_eq!(
            json_indices(&captured["attributeIndices"]),
            expected.attribute_indices
        );
        let lines = captured["lines"]
            .as_array()
            .expect("profile location lines");
        assert_eq!(lines.len(), expected.lines.len());
        for (captured, expected) in lines.iter().zip(&expected.lines) {
            assert_eq!(
                captured["functionIndex"].as_i64(),
                Some(i64::from(expected.function_index))
            );
            assert_eq!(json_i64(captured, "line"), expected.line);
            assert_eq!(json_i64(captured, "column"), expected.column);
        }
    }
    let functions = captured["functionTable"]
        .as_array()
        .expect("function table");
    assert_eq!(functions.len(), expected.function_table.len());
    for (captured, expected) in functions.iter().zip(&expected.function_table) {
        assert_eq!(
            captured["nameStrindex"].as_i64(),
            Some(i64::from(expected.name_strindex))
        );
        assert_eq!(
            captured["systemNameStrindex"].as_i64(),
            Some(i64::from(expected.system_name_strindex))
        );
        assert_eq!(
            captured["filenameStrindex"].as_i64(),
            Some(i64::from(expected.filename_strindex))
        );
        assert_eq!(json_i64(captured, "startLine"), expected.start_line);
    }
    let links = captured["linkTable"].as_array().expect("link table");
    assert_eq!(links.len(), expected.link_table.len());
    for (captured, expected) in links.iter().zip(&expected.link_table) {
        assert_eq!(
            decode_base64(captured["traceId"].as_str().expect("profile trace id"))
                .expect("decode trace id"),
            expected.trace_id
        );
        assert_eq!(
            decode_base64(captured["spanId"].as_str().expect("profile span id"))
                .expect("decode span id"),
            expected.span_id
        );
    }
    assert_eq!(
        captured["stringTable"]
            .as_array()
            .expect("string table")
            .iter()
            .map(|value| value.as_str().expect("profile string"))
            .collect::<Vec<_>>(),
        expected
            .string_table
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    let attributes = captured["attributeTable"]
        .as_array()
        .expect("attribute table");
    assert_eq!(attributes.len(), expected.attribute_table.len());
    for (captured, expected) in attributes.iter().zip(&expected.attribute_table) {
        assert_eq!(
            captured["keyStrindex"].as_i64(),
            Some(i64::from(expected.key_strindex))
        );
        assert_eq!(
            captured["unitStrindex"].as_i64(),
            Some(i64::from(expected.unit_strindex))
        );
        assert_eq!(
            captured
                .get("value")
                .map(|value| decode_any_value(value).expect("decode profile attribute value")),
            expected.value
        );
    }
    let stacks = captured["stackTable"].as_array().expect("stack table");
    assert_eq!(stacks.len(), expected.stack_table.len());
    for (captured, expected) in stacks.iter().zip(&expected.stack_table) {
        assert_eq!(
            json_indices(&captured["locationIndices"]),
            expected.location_indices
        );
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the test resolves every profiles index family through its captured dictionary"
)]
fn assert_profile_indices_resolve(captured: &Value, expected: &SubmissionEnvelope) {
    let profiles = expected.profiles.as_ref().expect("source profiles");
    let dictionary = &captured["dictionary"];
    let strings = dictionary["stringTable"]
        .as_array()
        .expect("captured strings");
    let attributes = dictionary["attributeTable"]
        .as_array()
        .expect("captured attributes");
    let mappings = dictionary["mappingTable"]
        .as_array()
        .expect("captured mappings");
    let locations = dictionary["locationTable"]
        .as_array()
        .expect("captured locations");
    let functions = dictionary["functionTable"]
        .as_array()
        .expect("captured functions");
    let links = dictionary["linkTable"].as_array().expect("captured links");
    let stacks = dictionary["stackTable"]
        .as_array()
        .expect("captured stacks");
    let captured_profiles = captured["resourceProfiles"]
        .as_array()
        .expect("resource profiles")
        .iter()
        .flat_map(|resource| {
            resource["scopeProfiles"]
                .as_array()
                .expect("scope profiles")
        })
        .flat_map(|scope| scope["profiles"].as_array().expect("profiles"));
    for captured in captured_profiles {
        for index in json_indices(&captured["attributeIndices"]) {
            assert_eq!(
                attributes[index_usize(index, "profile attribute index")],
                attribute_json(
                    &profiles.dictionary.attribute_table
                        [index_usize(index, "profile attribute index")]
                )
            );
        }
        for sample in captured["samples"].as_array().expect("samples") {
            let stack_index = json_usize(sample, "stackIndex");
            assert_eq!(
                stacks[stack_index],
                stack_json(&profiles.dictionary.stack_table[stack_index])
            );
            for location_index in json_indices(&stacks[stack_index]["locationIndices"]) {
                let location_index = index_usize(location_index, "location index");
                assert_eq!(
                    locations[location_index],
                    location_json(&profiles.dictionary.location_table[location_index])
                );
                let mapping_index = json_usize(&locations[location_index], "mappingIndex");
                assert_eq!(
                    mappings[mapping_index],
                    mapping_json(&profiles.dictionary.mapping_table[mapping_index])
                );
                for attribute_index in json_indices(&locations[location_index]["attributeIndices"])
                {
                    assert_eq!(
                        attributes[index_usize(attribute_index, "location attribute index")],
                        attribute_json(
                            &profiles.dictionary.attribute_table
                                [index_usize(attribute_index, "location attribute index")]
                        )
                    );
                }
                for line in locations[location_index]["lines"]
                    .as_array()
                    .expect("lines")
                {
                    let function_index = json_usize(line, "functionIndex");
                    assert_eq!(
                        functions[function_index],
                        function_json(&profiles.dictionary.function_table[function_index])
                    );
                    for key in ["nameStrindex", "systemNameStrindex", "filenameStrindex"] {
                        let string_index = json_usize(&functions[function_index], key);
                        assert_eq!(
                            strings[string_index],
                            profiles.dictionary.string_table[string_index]
                        );
                    }
                }
            }
            let link_index = json_usize(sample, "linkIndex");
            assert_eq!(
                links[link_index],
                link_json(&profiles.dictionary.link_table[link_index])
            );
            for attribute_index in json_indices(&sample["attributeIndices"]) {
                assert_eq!(
                    attributes[index_usize(attribute_index, "sample attribute index")],
                    attribute_json(
                        &profiles.dictionary.attribute_table
                            [index_usize(attribute_index, "sample attribute index")]
                    )
                );
            }
        }
        for value_type in [captured.get("sampleType"), captured.get("periodType")]
            .into_iter()
            .flatten()
        {
            for key in ["typeStrindex", "unitStrindex"] {
                let string_index = json_usize(value_type, key);
                assert_eq!(
                    strings[string_index],
                    profiles.dictionary.string_table[string_index]
                );
            }
        }
    }
    for (index, mapping) in mappings.iter().enumerate() {
        let string_index = json_usize(mapping, "filenameStrindex");
        assert_eq!(
            strings[string_index],
            profiles.dictionary.string_table[string_index]
        );
        assert_eq!(
            mapping,
            &mapping_json(&profiles.dictionary.mapping_table[index])
        );
    }
    for (index, attribute) in attributes.iter().enumerate() {
        for key in ["keyStrindex", "unitStrindex"] {
            let string_index = json_usize(attribute, key);
            assert_eq!(
                strings[string_index],
                profiles.dictionary.string_table[string_index]
            );
        }
        assert_eq!(
            attribute,
            &attribute_json(&profiles.dictionary.attribute_table[index])
        );
    }
}

fn mapping_json(value: &Mapping) -> Value {
    serde_json::json!({"memoryStart": value.memory_start.to_string(), "memoryLimit": value.memory_limit.to_string(), "fileOffset": value.file_offset.to_string(), "filenameStrindex": value.filename_strindex, "attributeIndices": value.attribute_indices})
}
fn location_json(value: &Location) -> Value {
    serde_json::json!({"mappingIndex": value.mapping_index, "address": value.address.to_string(), "lines": value.lines.iter().map(|line| serde_json::json!({"functionIndex": line.function_index, "line": line.line.to_string(), "column": line.column.to_string()})).collect::<Vec<_>>(), "attributeIndices": value.attribute_indices})
}
fn function_json(value: &Function) -> Value {
    serde_json::json!({"nameStrindex": value.name_strindex, "systemNameStrindex": value.system_name_strindex, "filenameStrindex": value.filename_strindex, "startLine": value.start_line.to_string()})
}
fn link_json(value: &ProfileLink) -> Value {
    serde_json::json!({"traceId": base64(&value.trace_id), "spanId": base64(&value.span_id)})
}
fn attribute_json(value: &KeyValueAndUnit) -> Value {
    let mut result =
        serde_json::json!({"keyStrindex": value.key_strindex, "unitStrindex": value.unit_strindex});
    if let Some(value) = &value.value {
        result["value"] = any_value_json(value);
    }
    result
}
fn stack_json(value: &Stack) -> Value {
    serde_json::json!({"locationIndices": value.location_indices})
}
fn any_value_json(value: &AnyValue) -> Value {
    match value {
        AnyValue::String(value) => serde_json::json!({"stringValue": value}),
        AnyValue::StringIndex(value) => serde_json::json!({"stringValueStrindex": value.get()}),
        _ => panic!("profile index test uses only string index values"),
    }
}
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[((a & 0b11) << 4 | b >> 4) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((b & 0b1111) << 2 | c >> 6) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(c & 0b11_1111) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[expect(
    clippy::too_many_lines,
    reason = "the test decoder compares every profile record and nested sample field"
)]
fn assert_captured_profiles(captured: &Value, envelopes: &[SubmissionEnvelope]) {
    assert_eq!(
        envelopes.len(),
        1,
        "each profiles request must own one dictionary"
    );
    let expected = envelopes[0].profiles.as_ref().expect("source profiles");
    assert_captured_dictionary(&captured["dictionary"], &expected.dictionary);
    let captured_profiles = captured["resourceProfiles"]
        .as_array()
        .expect("resource profiles")
        .iter()
        .flat_map(|resource| {
            resource["scopeProfiles"]
                .as_array()
                .expect("scope profiles")
        })
        .flat_map(|scope| scope["profiles"].as_array().expect("profiles"))
        .collect::<Vec<_>>();
    assert_eq!(captured_profiles.len(), expected.profiles.len());
    for (captured, expected) in captured_profiles.iter().zip(&expected.profiles) {
        let expected = &expected.record;
        assert_eq!(
            json_u64(captured, "timeUnixNano"),
            timestamp_nanos(expected.time)
        );
        assert_eq!(json_u64(captured, "durationNano"), expected.duration_nanos);
        assert_eq!(json_i64(captured, "period"), expected.period);
        assert_eq!(
            decode_base64(captured["profileId"].as_str().expect("profile id"))
                .expect("decode profile id"),
            expected.profile_id
        );
        assert_eq!(
            captured["droppedAttributesCount"].as_u64(),
            Some(u64::from(expected.dropped_attributes_count))
        );
        assert_eq!(
            captured
                .get("originalPayloadFormat")
                .and_then(Value::as_str),
            expected.original_payload_format.as_deref()
        );
        assert_eq!(
            decode_base64(captured["originalPayload"].as_str().expect("payload"))
                .expect("decode payload"),
            expected.original_payload
        );
        assert_eq!(
            json_indices(&captured["attributeIndices"]),
            expected.attribute_indices
        );
        assert_eq!(
            captured.get("sampleType").map(|value| (
                value["typeStrindex"].as_i64(),
                value["unitStrindex"].as_i64()
            )),
            expected.sample_type.as_ref().map(|value| (
                Some(i64::from(value.type_strindex)),
                Some(i64::from(value.unit_strindex))
            ))
        );
        assert_eq!(
            captured.get("periodType").map(|value| (
                value["typeStrindex"].as_i64(),
                value["unitStrindex"].as_i64()
            )),
            expected.period_type.as_ref().map(|value| (
                Some(i64::from(value.type_strindex)),
                Some(i64::from(value.unit_strindex))
            ))
        );
        let samples = captured["samples"].as_array().expect("samples");
        assert_eq!(samples.len(), expected.samples.len());
        for (captured, expected) in samples.iter().zip(&expected.samples) {
            assert_eq!(
                captured["stackIndex"].as_i64(),
                Some(i64::from(expected.stack_index))
            );
            assert_eq!(
                captured["linkIndex"].as_i64(),
                Some(i64::from(expected.link_index))
            );
            assert_eq!(
                json_indices(&captured["attributeIndices"]),
                expected.attribute_indices
            );
            assert_eq!(
                captured["values"]
                    .as_array()
                    .expect("sample values")
                    .iter()
                    .map(|value| value
                        .as_str()
                        .expect("value string")
                        .parse::<i64>()
                        .expect("sample value"))
                    .collect::<Vec<_>>(),
                expected.values
            );
            assert_eq!(
                captured["timestampsUnixNano"]
                    .as_array()
                    .expect("timestamps")
                    .iter()
                    .map(|value| value
                        .as_str()
                        .expect("timestamp string")
                        .parse::<u64>()
                        .expect("timestamp"))
                    .collect::<Vec<_>>(),
                expected.timestamps_unix_nano
            );
        }
    }
    assert_profile_indices_resolve(captured, &envelopes[0]);
}

fn indexed_profile_fixture(variant: u8) -> SubmissionEnvelope {
    let mut envelope = fixture("profiles");
    let profiles = envelope
        .profiles
        .as_mut()
        .expect("profile fixture contains profiles");
    let dictionary = &mut profiles.dictionary;
    dictionary.string_table.extend([
        format!("mapping-{variant}"),
        format!("attribute-value-{variant}"),
        format!("attribute-key-{variant}"),
        format!("attribute-unit-{variant}"),
        format!("function-{variant}"),
    ]);
    dictionary.attribute_table.push(KeyValueAndUnit::new(
        3,
        Some(AnyValue::StringIndex(
            StringIndex::try_new(2).expect("profile string index"),
        )),
        4,
    ));
    dictionary
        .mapping_table
        .push(Mapping::new(u64::from(variant), 2, 3, 1, vec![1]));
    dictionary.function_table.push(Function::new(5, 5, 1, 7));
    dictionary.location_table.push(Location::new(
        1,
        u64::from(variant),
        vec![Line::new(1, 8, 9)],
        vec![1],
    ));
    dictionary
        .link_table
        .push(ProfileLink::new([variant; 16], [variant; 8]));
    dictionary.stack_table.push(Stack::new(vec![1]));

    let profile = &mut profiles.profiles[0].record;
    profile.sample_type = Some(ValueType::new(5, 4));
    profile.period_type = Some(ValueType::new(5, 4));
    profile.attribute_indices = vec![1];
    profile.samples = vec![Sample::new(
        1,
        vec![1],
        1,
        vec![i64::from(variant)],
        vec![1],
    )];
    envelope
}

fn add_exemplar(envelope: &mut SubmissionEnvelope) {
    let exemplar = || {
        Exemplar::new(
            KeyValues::default(),
            Timestamp::UNIX_EPOCH,
            NumberValue::Double(OtlpDouble::new(-13.25)),
            Some(TraceId::new("11111111111111111111111111111111").expect("trace id")),
            Some(SpanId::new("2222222222222222").expect("span id")),
        )
    };
    let metric = &mut envelope.metrics[0].record.data;
    match metric {
        MetricData::Gauge { points } | MetricData::Sum { points, .. } => {
            points[0].exemplars.push(exemplar());
        }
        MetricData::Histogram { points, .. } => points[0].exemplars.push(exemplar()),
        MetricData::ExponentialHistogram { points, .. } => points[0].exemplars.push(exemplar()),
        MetricData::Summary { .. } => {}
        _ => unreachable!("new MetricData variants require an exemplar fixture"),
    }
}

fn assert_captured_logs(captured: &Value, envelope: &SubmissionEnvelope) {
    let captured = captured["resourceLogs"]
        .as_array()
        .expect("resource logs")
        .iter()
        .flat_map(|resource| resource["scopeLogs"].as_array().expect("scope logs"))
        .flat_map(|scope| scope["logRecords"].as_array().expect("log records"))
        .collect::<Vec<_>>();
    assert_eq!(captured.len(), envelope.logs.len());
    for (captured, expected) in captured.iter().zip(&envelope.logs) {
        let expected = &expected.record;
        assert_eq!(
            i128::from(json_u64(captured, "observedTimeUnixNano")),
            expected.observed_time.into_inner().unix_timestamp_nanos()
        );
        assert_eq!(
            decode_key_values(&captured["attributes"]).expect("decode log attributes"),
            expected.attributes
        );
        assert_eq!(
            captured
                .get("body")
                .map(|body| decode_any_value(body).expect("decode log body")),
            expected.body
        );
    }
}

fn assert_captured_traces(captured: &Value, envelope: &SubmissionEnvelope) {
    let captured = captured["resourceSpans"]
        .as_array()
        .expect("resource spans")
        .iter()
        .flat_map(|resource| resource["scopeSpans"].as_array().expect("scope spans"))
        .flat_map(|scope| scope["spans"].as_array().expect("spans"))
        .collect::<Vec<_>>();
    assert_eq!(captured.len(), envelope.spans.len());
    for (captured, expected) in captured.iter().zip(&envelope.spans) {
        let expected = &expected.record;
        assert_eq!(
            captured["traceId"].as_str(),
            Some(expected.trace_id.as_str())
        );
        assert_eq!(captured["spanId"].as_str(), Some(expected.span_id.as_str()));
        assert_eq!(
            decode_key_values(&captured["attributes"]).expect("decode span attributes"),
            expected.attributes
        );
    }
}

const NAMED_CANONICAL_FIXTURE_ROUTES: &[(&str, Signal)] = &[
    ("logs", Signal::Logs),
    ("non_finite_doubles", Signal::Logs),
    ("paired_log_span_generated_ids", Signal::Logs),
    ("plain_attribute_map", Signal::Logs),
    ("traces", Signal::Traces),
    ("paired_log_span_generated_ids", Signal::Traces),
    ("metric_gauge", Signal::Metrics),
    ("metric_sum", Signal::Metrics),
    ("metric_histogram", Signal::Metrics),
    ("metric_exponential_histogram", Signal::Metrics),
    ("metric_summary", Signal::Metrics),
    ("profiles", Signal::Profiles),
];

#[test]
fn submission_exporter_round_trips_every_signal_variant() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let (captured, server) = capture_server(listener, &[200, 200, 200, 200]);
    let logs = [fixture("logs"), fixture("non_finite_doubles")];
    let traces = [fixture("traces")];
    let mut metrics = [
        fixture("metric_gauge"),
        fixture("metric_sum"),
        fixture("metric_histogram"),
        fixture("metric_exponential_histogram"),
        fixture("metric_summary"),
    ];
    for envelope in &mut metrics {
        add_exemplar(envelope);
    }
    let profiles = [fixture("profiles")];
    exporter.export(Signal::Logs, &logs).expect("logs delivery");
    exporter
        .export(Signal::Traces, &traces)
        .expect("traces delivery");
    exporter
        .export(Signal::Metrics, &metrics)
        .expect("metrics delivery");
    exporter
        .export(Signal::Profiles, &profiles)
        .expect("profiles delivery");
    let requests = (0..4)
        .map(|_| {
            captured
                .recv_timeout(CAPTURE_TIMEOUT)
                .expect("captured request")
        })
        .collect::<Vec<_>>();
    assert_eq!(server.join().expect("capture server exits"), 4);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.0.as_str())
            .collect::<Vec<_>>(),
        [
            "/v1/logs",
            "/v1/traces",
            "/v1/metrics",
            "/v1development/profiles"
        ]
    );
    assert_captured_metrics(&requests[2].1, &metrics);
    assert_captured_profiles(&requests[3].1, &profiles);
    let decoded = requests
        .iter()
        .map(|request| decode_forms(&request.1).expect("decode captured proto JSON"))
        .collect::<Vec<_>>();
    assert_eq!(
        decoded[0].non_finite_doubles,
        vec![
            NonFiniteDouble::NaN,
            NonFiniteDouble::PositiveInfinity,
            NonFiniteDouble::NegativeInfinity,
        ]
    );
    assert!(decoded[3].key_indices.contains(&0));
}

#[test]
fn every_named_canonical_submission_fixture_is_decoded_for_its_signal() {
    let fixture_root = format!(
        "{}/../sc-observability-types/tests/fixtures/otlp_submission/golden",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut expected_names = NAMED_CANONICAL_FIXTURE_ROUTES
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    expected_names.sort_unstable();
    expected_names.dedup();
    let mut discovered_names = std::fs::read_dir(fixture_root)
        .expect("golden fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| {
            path.join("input.json").is_file() && path.join("expected.envelope.json").is_file()
        })
        .map(|path| {
            path.file_name()
                .expect("fixture directory name")
                .to_str()
                .expect("UTF-8 fixture name")
                .to_owned()
        })
        .collect::<Vec<_>>();
    discovered_names.sort_unstable();
    assert_eq!(
        discovered_names, expected_names,
        "a new canonical fixture requires a decoder route"
    );

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let (captured, server) =
        capture_server(listener, &vec![200; NAMED_CANONICAL_FIXTURE_ROUTES.len()]);
    for (name, signal) in NAMED_CANONICAL_FIXTURE_ROUTES {
        let envelope = fixture(name);
        exporter
            .export(*signal, std::slice::from_ref(&envelope))
            .expect("fixture delivery");
        let request = captured
            .recv_timeout(CAPTURE_TIMEOUT)
            .expect("captured fixture request");
        match signal {
            Signal::Logs => assert_captured_logs(&request.1, &envelope),
            Signal::Traces => assert_captured_traces(&request.1, &envelope),
            Signal::Metrics => assert_captured_metrics(&request.1, std::slice::from_ref(&envelope)),
            Signal::Profiles => {
                assert_captured_profiles(&request.1, std::slice::from_ref(&envelope));
            }
            _ => unreachable!("new signals require canonical fixture decoding"),
        }
    }
    assert_eq!(
        server.join().expect("capture server exits"),
        NAMED_CANONICAL_FIXTURE_ROUTES.len()
    );
}

#[test]
fn submission_exporter_classifies_400_and_exhausted_503_as_terminal() {
    for status in [400, 503] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let exporter = submission_exporter(
            format!("http://{}", listener.local_addr().expect("address")),
            Some(retry_policy(1)),
        );
        let statuses = if status == 503 {
            vec![status, status]
        } else {
            vec![status]
        };
        let (captured, server) = capture_server(listener, &statuses);
        let result = exporter.export(Signal::Logs, &[fixture("logs")]);
        for _ in &statuses {
            captured
                .recv_timeout(CAPTURE_TIMEOUT)
                .expect("captured request");
        }
        assert_eq!(server.join().expect("capture server exits"), statuses.len());
        assert!(
            matches!(result, Err(SubmissionExportFailure::Terminal(_))),
            "status {status}"
        );
    }
}

#[test]
fn submission_exporter_retries_503_then_succeeds() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        Some(retry_policy(1)),
    );
    let (captured, server) = capture_server(listener, &[503, 200]);
    exporter
        .export(Signal::Logs, &[fixture("logs")])
        .expect("retry succeeds");
    let first = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("first request");
    let second = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("retried request");
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_eq!(first, second, "retry resubmits the same canonical payload");
}

#[test]
fn profiles_with_distinct_dictionaries_are_submitted_separately() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let first_fixture = indexed_profile_fixture(1);
    let second_fixture = indexed_profile_fixture(2);
    let (captured, server) = capture_server(listener, &[200, 200]);
    exporter
        .export(
            Signal::Profiles,
            &[first_fixture.clone(), second_fixture.clone()],
        )
        .expect("profiles delivery");
    let first = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("first request");
    let second = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("second request");
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_eq!(first.0, "/v1development/profiles");
    assert_eq!(second.0, "/v1development/profiles");
    assert_captured_profiles(&first.1, std::slice::from_ref(&first_fixture));
    assert_captured_profiles(&second.1, std::slice::from_ref(&second_fixture));
}

#[test]
fn generated_id_and_plain_attribute_fixtures_reach_their_signal_routes() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let paired = fixture("paired_log_span_generated_ids");
    let plain_attributes = fixture("plain_attribute_map");
    let expected_attributes = plain_attributes.logs[0].record.attributes.clone();
    let (captured, server) = capture_server(listener, &[200, 200]);

    exporter
        .export(Signal::Logs, &[paired.clone(), plain_attributes])
        .expect("log fixtures deliver");
    exporter
        .export(Signal::Traces, &[paired])
        .expect("generated-id span fixture delivers");

    let logs = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured logs fixture request");
    let traces = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured traces fixture request");
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_eq!(logs.0, "/v1/logs");
    assert_eq!(traces.0, "/v1/traces");
    let decoded_attributes = decode_key_values(
        &logs.1["resourceLogs"][0]["scopeLogs"][0]["logRecords"][1]["attributes"],
    )
    .expect("captured attributes decode back into canonical values");
    assert_eq!(
        decoded_attributes, expected_attributes,
        "the plain attribute fixture retains each canonical key/value pair"
    );
    assert!(
        traces.1["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["traceId"].is_string(),
        "the generated-id fixture is encoded into the trace request"
    );
}

#[test]
fn rich_log_and_span_fields_survive_capture_as_canonical_values() {
    let input = serde_json::json!({
        "version": 1,
        "logs": [{
            "time": "1970-01-01T00:00:00.000000000Z",
            "observed_time": "1970-01-01T00:00:01.000000000Z",
            "severity_number": 17,
            "severity_text": "ERROR",
            "event_name": "audit.event",
            "body": {"kind": "string", "data": "log body"},
            "attributes": [["log.attribute", {"kind": "bool", "data": true}]],
            "dropped_attributes_count": 2,
            "flags": 5,
            "trace_id": "0123456789abcdef0123456789abcdef",
            "span_id": "0123456789abcdef"
        }],
        "spans": [{
            "trace_id": "0123456789abcdef0123456789abcdef",
            "span_id": "0123456789abcdef",
            "trace_state": "vendor=state",
            "parent_span_id": "fedcba9876543210",
            "flags": 7,
            "name": "operation",
            "kind": "server",
            "start_time": "1970-01-01T00:00:00.000000000Z",
            "duration_nanos": 2_000_000_000_u64,
            "attributes": [["span.attribute", {"kind": "int", "data": 42}]],
            "dropped_attributes_count": 3,
            "events": [{
                "time": "1970-01-01T00:00:01.000000000Z",
                "name": "event",
                "attributes": [["event.attribute", {"kind": "string", "data": "value"}]],
                "dropped_attributes_count": 4
            }],
            "dropped_events_count": 5,
            "links": [{
                "trace_id": "11111111111111111111111111111111",
                "span_id": "2222222222222222",
                "trace_state": "link=state",
                "attributes": [["link.attribute", {"kind": "int", "data": 9}]],
                "dropped_attributes_count": 6,
                "flags": 8
            }],
            "dropped_links_count": 9,
            "status": {"code": "error", "message": "failed"}
        }]
    });
    let envelope = envelope(&input);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let (captured, server) = capture_server(listener, &[200, 200]);
    exporter
        .export(Signal::Logs, std::slice::from_ref(&envelope))
        .expect("log delivery");
    exporter
        .export(Signal::Traces, std::slice::from_ref(&envelope))
        .expect("span delivery");
    let logs = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured logs")
        .1;
    let traces = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured traces")
        .1;
    assert_eq!(server.join().expect("capture server exits"), 2);
    assert_captured_log(&logs, &envelope);
    assert_captured_span(&traces, &envelope);
}

fn assert_captured_log(captured: &serde_json::Value, envelope: &SubmissionEnvelope) {
    let expected = &envelope.logs[0].record;
    let log = &captured["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
    assert_eq!(log["timeUnixNano"], "0");
    assert_eq!(log["observedTimeUnixNano"], "1000000000");
    assert_eq!(log["severityNumber"], expected.severity_number.get());
    assert_eq!(
        log["severityText"],
        expected.severity_text.as_deref().unwrap()
    );
    assert_eq!(log["eventName"], expected.event_name.as_deref().unwrap());
    assert_eq!(
        decode_any_value(&log["body"]).expect("decode log body"),
        expected.body.clone().expect("log body")
    );
    assert_eq!(
        decode_key_values(&log["attributes"]).expect("decode log attributes"),
        expected.attributes
    );
    assert_eq!(log["droppedAttributesCount"], 2);
    assert_eq!(log["flags"], 5);
    assert_eq!(log["traceId"], "0123456789abcdef0123456789abcdef");
    assert_eq!(log["spanId"], "0123456789abcdef");
}

fn assert_captured_span(captured: &serde_json::Value, envelope: &SubmissionEnvelope) {
    let expected = &envelope.spans[0].record;
    let span = &captured["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
    assert_eq!(span["traceId"], "0123456789abcdef0123456789abcdef");
    assert_eq!(span["spanId"], "0123456789abcdef");
    assert_eq!(span["parentSpanId"], "fedcba9876543210");
    assert_eq!(
        span["traceState"],
        expected.trace_state.as_ref().unwrap().as_str()
    );
    assert_eq!(span["flags"], expected.flags);
    assert_eq!(span["name"], expected.name);
    assert_eq!(span["kind"], 2);
    assert_eq!(span["startTimeUnixNano"], "0");
    assert_eq!(span["endTimeUnixNano"], "2000000000");
    assert_eq!(
        decode_key_values(&span["attributes"]).expect("decode span attributes"),
        expected.attributes
    );
    assert_eq!(
        span["droppedAttributesCount"],
        expected.dropped_attributes_count
    );
    assert_eq!(span["droppedEventsCount"], expected.dropped_events_count);
    assert_eq!(span["droppedLinksCount"], expected.dropped_links_count);
    assert_eq!(span["status"]["code"], 2);
    assert_eq!(span["status"]["message"], "failed");
    let event = &span["events"][0];
    assert_eq!(event["timeUnixNano"], "1000000000");
    assert_eq!(event["name"], expected.events[0].name);
    assert_eq!(
        decode_key_values(&event["attributes"]).expect("decode event attributes"),
        expected.events[0].attributes
    );
    assert_eq!(event["droppedAttributesCount"], 4);
    let link = &span["links"][0];
    assert_eq!(link["traceId"], "11111111111111111111111111111111");
    assert_eq!(link["spanId"], "2222222222222222");
    assert_eq!(link["traceState"], "link=state");
    assert_eq!(
        decode_key_values(&link["attributes"]).expect("decode link attributes"),
        expected.links[0].attributes
    );
    assert_eq!(link["droppedAttributesCount"], 6);
    assert_eq!(link["flags"], 8);
}

#[test]
fn logs_are_grouped_by_distinct_resource_and_scope() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let first = fixture("logs");
    let mut second_scope = first.clone();
    second_scope.logs[0].scope.name = "second-scope".to_owned();
    second_scope.logs[0].scope.schema_url = Some("https://example.test/second-scope".to_owned());
    let mut second_resource = first.clone();
    second_resource.logs[0].resource.schema_url =
        Some("https://example.test/second-resource".to_owned());
    let (captured, server) = capture_server(listener, &[200]);

    exporter
        .export(Signal::Logs, &[first, second_scope, second_resource])
        .expect("grouped logs deliver");
    let (_, request) = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured grouped request");
    assert_eq!(server.join().expect("capture server exits"), 1);
    let resources = request["resourceLogs"].as_array().expect("resource groups");
    assert_eq!(resources.len(), 2, "different resources must not coalesce");
    assert_eq!(
        resources[0]["scopeLogs"].as_array().map(Vec::len),
        Some(2),
        "different scopes within one resource stay distinct"
    );
    let resource: ResourceLogs =
        serde_json::from_value(resources[1].clone()).expect("logs group is protocol-shaped");
    assert_eq!(resource.schema_url, "https://example.test/second-resource");
    assert!(
        resource
            .resource
            .expect("resource is present")
            .schema_url
            .is_empty()
    );
    let scope: ResourceLogs =
        serde_json::from_value(resources[0].clone()).expect("logs group is protocol-shaped");
    assert_eq!(
        scope.scope_logs[1].schema_url,
        "https://example.test/second-scope"
    );
    assert!(
        scope.scope_logs[1]
            .scope
            .expect("scope is present")
            .schema_url
            .is_empty()
    );
}

#[test]
fn traces_metrics_and_profiles_are_grouped_by_distinct_resource_and_scope() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let (captured, server) = capture_server(listener, &[200, 200, 200]);

    let traces = grouped_envelopes(fixture("traces"), Signal::Traces);
    let metrics = grouped_envelopes(fixture("metric_gauge"), Signal::Metrics);
    let profiles = grouped_profile_records();
    exporter
        .export(Signal::Traces, &traces)
        .expect("grouped traces deliver");
    exporter
        .export(Signal::Metrics, &metrics)
        .expect("grouped metrics deliver");
    exporter
        .export(Signal::Profiles, std::slice::from_ref(&profiles))
        .expect("grouped profiles deliver");

    let traces = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured traces")
        .1;
    let metrics = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured metrics")
        .1;
    let profiles = captured
        .recv_timeout(CAPTURE_TIMEOUT)
        .expect("captured profiles")
        .1;
    assert_eq!(server.join().expect("capture server exits"), 3);
    assert_grouping(&traces, "resourceSpans", "scopeSpans");
    assert_grouping(&metrics, "resourceMetrics", "scopeMetrics");
    assert_grouping(&profiles, "resourceProfiles", "scopeProfiles");
}

fn grouped_profile_records() -> SubmissionEnvelope {
    let mut envelope = fixture("profiles");
    let profiles = envelope.profiles.as_mut().expect("profiles signal");
    let first = profiles.profiles[0].clone();
    let mut second_scope = first.clone();
    second_scope.scope.name = "second-scope".to_owned();
    second_scope.scope.schema_url = Some("https://example.test/second-scope".to_owned());
    let mut second_resource = first;
    second_resource.resource.schema_url = Some("https://example.test/second-resource".to_owned());
    profiles.profiles.push(second_scope);
    profiles.profiles.push(second_resource);
    envelope
}

fn grouped_envelopes(first: SubmissionEnvelope, signal: Signal) -> [SubmissionEnvelope; 3] {
    let mut second_scope = first.clone();
    let mut second_resource = first.clone();
    match signal {
        Signal::Traces => {
            second_scope.spans[0].scope.name = "second-scope".to_owned();
            second_scope.spans[0].scope.schema_url =
                Some("https://example.test/second-scope".to_owned());
            second_resource.spans[0].resource.schema_url =
                Some("https://example.test/second-resource".to_owned());
        }
        Signal::Metrics => {
            second_scope.metrics[0].scope.name = "second-scope".to_owned();
            second_scope.metrics[0].scope.schema_url =
                Some("https://example.test/second-scope".to_owned());
            second_resource.metrics[0].resource.schema_url =
                Some("https://example.test/second-resource".to_owned());
        }
        Signal::Profiles => unreachable!("profiles must share one envelope dictionary"),
        _ => unreachable!("only non-log signal grouping is covered here"),
    }
    [first, second_scope, second_resource]
}

fn assert_grouping(captured: &Value, resource_key: &str, scope_key: &str) {
    let resources = captured[resource_key].as_array().expect("resource groups");
    assert_eq!(resources.len(), 2, "distinct resources must not coalesce");
    assert_eq!(
        resources[0][scope_key].as_array().map(Vec::len),
        Some(2),
        "distinct scopes within one resource stay distinct"
    );
    match resource_key {
        "resourceSpans" => {
            let resource: ResourceSpans = serde_json::from_value(resources[1].clone())
                .expect("trace resource group is shaped by the pinned protocol");
            assert_eq!(resource.schema_url, "https://example.test/second-resource");
            assert!(
                resource
                    .resource
                    .expect("resource is present")
                    .schema_url
                    .is_empty()
            );
            let scope: ResourceSpans = serde_json::from_value(resources[0].clone())
                .expect("trace scope group is shaped by the pinned protocol");
            assert_eq!(
                scope.scope_spans[1].schema_url,
                "https://example.test/second-scope"
            );
            assert!(
                scope.scope_spans[1]
                    .scope
                    .expect("scope is present")
                    .schema_url
                    .is_empty()
            );
        }
        "resourceMetrics" => {
            let resource: ResourceMetrics = serde_json::from_value(resources[1].clone())
                .expect("metric resource group is shaped by the pinned protocol");
            assert_eq!(resource.schema_url, "https://example.test/second-resource");
            assert!(
                resource
                    .resource
                    .expect("resource is present")
                    .schema_url
                    .is_empty()
            );
            let scope: ResourceMetrics = serde_json::from_value(resources[0].clone())
                .expect("metric scope group is shaped by the pinned protocol");
            assert_eq!(
                scope.scope_metrics[1].schema_url,
                "https://example.test/second-scope"
            );
            assert!(
                scope.scope_metrics[1]
                    .scope
                    .expect("scope is present")
                    .schema_url
                    .is_empty()
            );
        }
        "resourceProfiles" => {
            assert_eq!(
                resources[1]["schemaUrl"],
                "https://example.test/second-resource"
            );
            assert!(resources[1]["resource"]["schemaUrl"].is_null());
            assert_eq!(
                resources[0][scope_key][1]["schemaUrl"],
                "https://example.test/second-scope"
            );
            assert!(resources[0][scope_key][1]["scope"]["schemaUrl"].is_null());
        }
        _ => unreachable!("only grouped signal requests are asserted"),
    }
}

fn unsupported_after_split(envelopes: &[SubmissionEnvelope]) -> Result<Value, ExportError> {
    if envelopes.len() > 1 {
        return Ok(Value::String(
            "x".repeat(MAX_OTLP_ENCODED_REQUEST_BYTES.saturating_add(1)),
        ));
    }
    Err(super::super::values::unsupported_variant(
        "test split encoder",
    ))
}

#[test]
fn terminal_encoder_errors_survive_submission_routing_and_split_recursion() {
    let exporter = submission_exporter("http://127.0.0.1:1".to_owned(), None);
    let first = fixture("logs");
    let second = fixture("logs");

    let error = exporter
        .export_encoded(
            SubmissionRoute::Signal(Signal::Logs),
            &[first, second],
            unsupported_after_split,
        )
        .expect_err("the split encoder fails for each individual request");

    assert!(matches!(
        error,
        SubmissionExportFailure::Terminal(ExportError::TerminalExportFailure { .. })
    ));
}

#[test]
fn oversized_multi_envelope_submission_splits_at_encoded_request_limit() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
    let exporter = submission_exporter(
        format!("http://{}", listener.local_addr().expect("address")),
        None,
    );
    let mut first = fixture("logs");
    let mut second = fixture("logs");
    let first_body = "a".repeat(MAX_OTLP_ENCODED_REQUEST_BYTES / 2);
    let second_body = "b".repeat(MAX_OTLP_ENCODED_REQUEST_BYTES / 2);
    first.logs[0].record.body = Some(AnyValue::String(first_body.clone()));
    second.logs[0].record.body = Some(AnyValue::String(second_body.clone()));

    let (captured, server) = capture_server(listener, &[200, 200]);
    exporter
        .export(Signal::Logs, &[first, second])
        .expect("oversized batch is split and delivered");
    let requests = (0..2)
        .map(|_| {
            captured
                .recv_timeout(CAPTURE_TIMEOUT)
                .expect("split request")
        })
        .collect::<Vec<_>>();

    assert_eq!(server.join().expect("capture server exits"), 2);
    assert!(
        requests
            .iter()
            .all(|(_, request)| request.to_string().len() <= MAX_OTLP_ENCODED_REQUEST_BYTES),
        "every split request respects the encoded-byte limit"
    );
    assert_eq!(
        requests
            .iter()
            .map(
                |(_, request)| request["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["body"]
                    ["stringValue"]
                    .as_str()
            )
            .collect::<Vec<_>>(),
        [Some(first_body.as_str()), Some(second_body.as_str())],
        "the split retains each original envelope exactly once"
    );
}

#[test]
fn runtime_short_shutdown_does_not_requeue_submission() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let exporter = submission_exporter_with_shutdown(
        format!("http://{}", listener.local_addr().unwrap()),
        Some(retry_policy(1)),
        Some(500),
    );
    assert_eq!(
        exporter.exporter.submission_wait_budget(),
        std::time::Duration::from_millis(3000) + crate::constants::SUBMISSION_DISPATCH_MARGIN
    );
    // Zero backoff and explicit responses exercise the worker's result without
    // sleeping or asserting scheduler-dependent elapsed time.
    let (captured, server) = capture_server(listener, &[503, 200]);
    exporter.export(Signal::Logs, &[fixture("logs")]).unwrap();
    for _ in 0..2 {
        captured.recv_timeout(CAPTURE_TIMEOUT).unwrap();
    }
    assert_eq!(server.join().unwrap(), 2);
}

#[test]
fn runtime_routes_preserve_existing_endpoint_suffixes() {
    for (signal, name, suffix) in [
        (Signal::Logs, "logs", "/v1/logs"),
        (Signal::Traces, "traces", "/v1/traces"),
        (Signal::Metrics, "metric_gauge", "/v1/metrics"),
        (Signal::Profiles, "profiles", "/v1development/profiles"),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let exporter = submission_exporter(
            format!("http://{}{suffix}/", listener.local_addr().unwrap()),
            None,
        );
        let (captured, server) = capture_server(listener, &[200]);
        exporter.export(signal, &[fixture(name)]).unwrap();
        assert_eq!(captured.recv_timeout(CAPTURE_TIMEOUT).unwrap().0, suffix);
        assert_eq!(server.join().unwrap(), 1);
    }
}

#[test]
fn runtime_submission_rejects_blocking_calls_inside_tokio() {
    let exporter = submission_exporter("http://127.0.0.1:1".into(), None);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for (signal, name) in [(Signal::Logs, "logs"), (Signal::Profiles, "profiles")] {
            let error = exporter.export(signal, &[fixture(name)]).unwrap_err();
            assert!(matches!(
                error,
                SubmissionExportFailure::Retryable(
                    ExportError::BlockingBackendInAsyncContext { .. }
                )
            ));
        }
    });
}
