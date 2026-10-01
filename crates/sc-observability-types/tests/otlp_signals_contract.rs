use sc_observability_types::otlp::signals::{
    AnyValue, KeyValues, OtlpDouble, StringIndex, TraceState,
};

#[test]
fn values_preserve_variants_and_nested_data() {
    let values = [
        AnyValue::String("text".into()),
        AnyValue::Bool(true),
        AnyValue::Int(-4),
        AnyValue::UInt(4),
        AnyValue::Double(1.5.into()),
        AnyValue::Bytes(vec![0, 127, 255]),
        AnyValue::StringIndex(StringIndex::try_new(1).unwrap()),
        AnyValue::Array(vec![AnyValue::UInt(3)]),
        AnyValue::KvList(
            KeyValues::try_from_iter([("key".into(), AnyValue::Bytes(vec![42]))]).unwrap(),
        ),
    ];
    for value in values {
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<AnyValue>(&json).unwrap(),
            value,
            "{json}"
        );
    }
    assert_eq!(
        serde_json::to_string(&AnyValue::Bytes(vec![0, 127, 255])).unwrap(),
        r#"{"kind":"bytes","data":"007fff"}"#
    );
}

#[test]
fn non_finite_doubles_round_trip_as_proto_json_strings() {
    for (number, spelling) in [
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
    ] {
        let value = OtlpDouble::new(number);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, format!("\"{spelling}\""));
        let decoded = serde_json::from_str::<OtlpDouble>(&json).unwrap();
        assert_eq!(decoded.get().to_bits(), number.to_bits());
        let tagged = AnyValue::Double(value);
        assert_eq!(
            serde_json::from_str::<AnyValue>(&serde_json::to_string(&tagged).unwrap()).unwrap(),
            tagged
        );
    }
}

#[test]
fn plain_maps_sort_keys_and_reject_duplicates_at_every_depth() {
    let values: KeyValues = serde_json::from_str(r#"{"z":1,"a":{"nested":true}}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&values).unwrap(),
        r#"[["a",{"kind":"kv_list","data":[["nested",{"kind":"bool","data":true}]]}],["z",{"kind":"int","data":1}]]"#
    );
    for json in [
        r#"{"a":1,"a":2}"#,
        r#"{"outer":{"a":1,"a":2}}"#,
        r#"{"a":null}"#,
        r#"[["a",1],["a",2]]"#,
    ] {
        assert!(serde_json::from_str::<KeyValues>(json).is_err(), "{json}");
    }
    assert!(serde_json::from_str::<AnyValue>("null").is_err());
    assert_eq!(
        serde_json::from_str::<AnyValue>("18446744073709551615").unwrap(),
        AnyValue::UInt(u64::MAX)
    );
    assert_eq!(
        serde_json::from_str::<AnyValue>("1e2").unwrap(),
        AnyValue::Double(100.0.into())
    );
}

#[test]
fn trace_state_and_string_indices_validate_on_deserialization() {
    for valid in ["", "tenant@system=opaque", "vendor=abc,other=xyz"] {
        assert_eq!(TraceState::try_new(valid).unwrap().as_str(), valid);
    }
    for invalid in ["a=x,a=y", "A=x", "a=", "a=a=b", "a@System=v", "a=x,=y"] {
        assert!(TraceState::try_new(invalid).is_err(), "{invalid}");
        assert!(serde_json::from_str::<TraceState>(&format!("\"{invalid}\"")).is_err());
    }
    assert!(serde_json::from_str::<StringIndex>("-1").is_err());
    assert!(serde_json::from_str::<StringIndex>("2147483648").is_err());
}

use sc_observability_types::{
    Timestamp,
    otlp::signals::{
        AggregationTemporality, DataPointFlags, ExponentialBuckets, ExponentialHistogramDataPoint,
        HistogramDataPoint, InstrumentationScope, KeyValueAndUnit, MetricData, NumberPoint,
        NumberValue, Profile, ProfilesDictionary, Resource, ResourceRecord, SummaryDataPoint,
        ValueAtQuantile,
    },
};

fn timestamp(seconds: i64) -> Timestamp {
    Timestamp::UNIX_EPOCH + time::Duration::seconds(seconds)
}
fn number() -> NumberPoint {
    NumberPoint::try_new(
        KeyValues::default(),
        Some(timestamp(1)),
        timestamp(2),
        NumberValue::Int(5),
        vec![],
        DataPointFlags::default(),
    )
    .unwrap()
}

#[test]
fn every_metric_form_round_trips_and_deserialization_checks_semantics() {
    let histogram = HistogramDataPoint::try_new(
        KeyValues::default(),
        Some(timestamp(1)),
        timestamp(2),
        3,
        Some(f64::INFINITY.into()),
        vec![1, 2],
        vec![5.0.into()],
        vec![],
        DataPointFlags::new(1),
        Some(f64::NEG_INFINITY.into()),
        Some(f64::NAN.into()),
    )
    .unwrap();
    let exponential = ExponentialHistogramDataPoint::try_new(
        KeyValues::default(),
        Some(timestamp(1)),
        timestamp(2),
        3,
        Some(f64::NAN.into()),
        0,
        1,
        0.0.into(),
        ExponentialBuckets::new(0, vec![1]),
        ExponentialBuckets::new(0, vec![1]),
        vec![],
        DataPointFlags::default(),
        None,
        None,
    )
    .unwrap();
    let summary = SummaryDataPoint::try_new(
        KeyValues::default(),
        None,
        timestamp(2),
        2,
        f64::INFINITY.into(),
        vec![ValueAtQuantile::try_new(0.5.into(), f64::NAN.into()).unwrap()],
        DataPointFlags::default(),
    )
    .unwrap();
    let forms = [
        MetricData::Gauge {
            points: vec![number()],
        },
        MetricData::Sum {
            points: vec![number()],
            temporality: AggregationTemporality::Delta,
            monotonic: true,
        },
        MetricData::Histogram {
            points: vec![histogram.clone()],
            temporality: AggregationTemporality::Cumulative,
        },
        MetricData::ExponentialHistogram {
            points: vec![exponential.clone()],
            temporality: AggregationTemporality::Delta,
        },
        MetricData::Summary {
            points: vec![summary.clone()],
        },
    ];
    for form in forms {
        let json = serde_json::to_string(&form).unwrap();
        assert_eq!(serde_json::from_str::<MetricData>(&json).unwrap(), form);
    }
    let mut bad = serde_json::to_value(&histogram).unwrap();
    bad["bucket_counts"] = serde_json::json!([1]);
    assert!(serde_json::from_value::<HistogramDataPoint>(bad).is_err());
    let mut bad = serde_json::to_value(&histogram).unwrap();
    bad["explicit_bounds"] = serde_json::json!(["NaN"]);
    assert!(serde_json::from_value::<HistogramDataPoint>(bad).is_err());
    let mut bad = serde_json::to_value(&exponential).unwrap();
    bad["scale"] = serde_json::json!(21);
    assert!(serde_json::from_value::<ExponentialHistogramDataPoint>(bad).is_err());
    let mut bad = serde_json::to_value(&summary).unwrap();
    bad["quantile_values"][0]["quantile"] = serde_json::json!(1.5);
    assert!(serde_json::from_value::<SummaryDataPoint>(bad).is_err());
    for (temporality, value, start) in [
        ("unspecified", 5, 1),
        ("delta", 5, 2),
        ("cumulative", -1, 1),
    ] {
        let mut bad = serde_json::to_value(MetricData::Sum {
            points: vec![number()],
            temporality: AggregationTemporality::Delta,
            monotonic: true,
        })
        .unwrap();
        bad["data"]["temporality"] = serde_json::json!(temporality);
        bad["data"]["points"][0]["value"]["data"] = serde_json::json!(value);
        bad["data"]["points"][0]["start_time"] = serde_json::to_value(timestamp(start)).unwrap();
        assert!(serde_json::from_value::<MetricData>(bad).is_err());
    }
}

#[test]
fn profiles_check_nested_value_and_attribute_key_indices() {
    let mut dict = ProfilesDictionary::default();
    dict.string_table.push("sample".into());
    dict.attribute_table.push(KeyValueAndUnit::new(
        1,
        Some(AnyValue::StringIndex(StringIndex::try_new(1).unwrap())),
        0,
    ));
    let mut resource = Resource::default();
    resource.attributes = KeyValues::try_from_iter([(
        sc_observability_types::otlp::signals::AttributeKey::Index(
            StringIndex::try_new(1).unwrap(),
        ),
        AnyValue::String("value".into()),
    )])
    .unwrap();
    let record = ResourceRecord::new(
        resource,
        InstrumentationScope::default(),
        Profile::new(
            None,
            vec![],
            timestamp(1),
            10,
            None,
            0,
            [1; 16],
            0,
            None,
            vec![],
            vec![1],
        ),
    );
    dict.validate_references(std::slice::from_ref(&record))
        .unwrap();
    let json = serde_json::to_string(&record).unwrap();
    assert!(json.contains("01010101010101010101010101010101"));
    assert_eq!(
        serde_json::from_str::<ResourceRecord<Profile>>(&json).unwrap(),
        record
    );
    let mut invalid = dict.clone();
    invalid.attribute_table[1].value =
        Some(AnyValue::StringIndex(StringIndex::try_new(2).unwrap()));
    assert!(
        invalid
            .validate_references(std::slice::from_ref(&record))
            .is_err()
    );
    let mut invalid_record = record.clone();
    invalid_record.resource.attributes = KeyValues::try_from_iter([(
        sc_observability_types::otlp::signals::AttributeKey::Index(
            StringIndex::try_new(2).unwrap(),
        ),
        AnyValue::Int(1),
    )])
    .unwrap();
    assert!(dict.validate_references(&[invalid_record]).is_err());
    let mut invalid = dict.clone();
    invalid.stack_table[0].location_indices.push(9);
    assert!(invalid.validate_references(&[record]).is_err());
}

fn null_path<T: std::fmt::Debug>(
    result: Result<T, sc_observability_types::otlp::signals::SignalValidationError>,
    expected: &str,
) {
    match result.unwrap_err() {
        sc_observability_types::otlp::signals::SignalValidationError::Validation {
            path, ..
        } => assert_eq!(path, expected),
        _ => panic!("unexpected error variant"),
    }
}
fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: &T,
) {
    assert_eq!(
        &serde_json::from_str::<T>(&serde_json::to_string(value).unwrap()).unwrap(),
        value
    );
}
#[test]
fn resource_conversion_rejects_null_at_exact_path() {
    use sc_observability_types::{otlp::OtlpResource, v2::AttributeValue};
    let mut source = OtlpResource::default();
    source.attributes.insert("key".into(), AttributeValue::Null);
    null_path(Resource::try_from(source.clone()), "attributes.key");
    source
        .attributes
        .insert("key".into(), AttributeValue::UInt(u64::MAX));
    round_trip(&Resource::try_from(source).unwrap());
}
#[test]
fn scope_conversion_rejects_null_at_exact_path() {
    use sc_observability_types::{otlp::OtlpInstrumentationScope, v2::AttributeValue};
    let mut source = OtlpInstrumentationScope::default();
    source.attributes.insert(
        "key".into(),
        AttributeValue::Array(vec![AttributeValue::Null]),
    );
    null_path(
        InstrumentationScope::try_from(source.clone()),
        "attributes.key[0]",
    );
    source
        .attributes
        .insert("key".into(), AttributeValue::String("ok".into()));
    round_trip(&InstrumentationScope::try_from(source).unwrap());
}
#[test]
fn metric_conversion_rejects_null_at_exact_path() {
    use sc_observability_types::{
        MetricName, ServiceName,
        otlp::signals::MetricStream,
        v2::{AttributeValue, FiniteF64, MetricRecord, MetricValue},
    };
    let source = MetricRecord::try_new(
        timestamp(2),
        ServiceName::new("test").unwrap(),
        MetricName::new("gauge").unwrap(),
        MetricValue::Gauge(FiniteF64::new(1.0).unwrap()),
    )
    .unwrap();
    null_path(
        MetricStream::try_from(
            source
                .clone()
                .with_attributes([("key".into(), AttributeValue::Null)].into()),
        ),
        "attributes.key",
    );
    round_trip(&MetricStream::try_from(source).unwrap());
}
#[test]
fn span_conversion_rejects_null_at_exact_path() {
    use sc_observability_types::{
        ActionName, ServiceName, SpanId, SpanStatus, TraceId,
        otlp::signals::SpanPoint,
        v2::{AttributeValue, SpanRecord, TraceContext, TraceFlags},
    };
    let make = |attributes| {
        SpanRecord::new(
            timestamp(1),
            ServiceName::new("test").unwrap(),
            ActionName::new("test").unwrap(),
            TraceContext::new(
                TraceId::new("11111111111111111111111111111111").unwrap(),
                SpanId::new("1111111111111111").unwrap(),
                TraceFlags::new(1),
            ),
            attributes,
        )
        .end(SpanStatus::Ok, 10u64.into())
    };
    null_path(
        SpanPoint::try_from(make([("key".into(), AttributeValue::Null)].into())),
        "attributes.key",
    );
    round_trip(&SpanPoint::try_from(make(std::collections::BTreeMap::default())).unwrap());
}
#[test]
fn log_conversion_rejects_null_at_exact_path() {
    use sc_observability_types::{
        ActionName, Level, LogEvent, ProcessIdentity, SchemaVersion, ServiceName, TargetCategory,
        otlp::signals::LogPoint,
    };
    let mut source = LogEvent {
        version: SchemaVersion::new("v1").unwrap(),
        timestamp: timestamp(1),
        level: Level::Info,
        service: ServiceName::new("test").unwrap(),
        target: TargetCategory::new("test").unwrap(),
        action: ActionName::new("test").unwrap(),
        message: Some("complete".into()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    };
    source.fields.insert("key".into(), serde_json::Value::Null);
    null_path(LogPoint::try_from(source.clone()), "fields.key");
    source.fields.insert("key".into(), serde_json::json!(true));
    round_trip(&LogPoint::try_from(source).unwrap());
}

#[test]
fn nonfinite_measurements_survive_exemplars_and_all_double_point_fields() {
    use sc_observability_types::otlp::signals::Exemplar;
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let number = NumberValue::Double(value.into());
        round_trip(&number);
        let exemplar = Exemplar::new(KeyValues::default(), timestamp(1), number, None, None);
        round_trip(&exemplar);
        let histogram = HistogramDataPoint::try_new(
            KeyValues::default(),
            None,
            timestamp(2),
            0,
            Some(value.into()),
            vec![0],
            vec![],
            vec![exemplar],
            DataPointFlags::default(),
            Some(value.into()),
            Some(value.into()),
        )
        .unwrap();
        round_trip(&histogram);
        let summary = SummaryDataPoint::try_new(
            KeyValues::default(),
            None,
            timestamp(2),
            0,
            value.into(),
            vec![ValueAtQuantile::try_new(0.5.into(), value.into()).unwrap()],
            DataPointFlags::default(),
        )
        .unwrap();
        round_trip(&summary);
    }
}
