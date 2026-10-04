//! Payload-derived span/metric projectors and decoded collector assertions.
use sc_observability_types::v2;
use sc_observability_types::{
    ActionName, DurationMs, LogEvent, MetricName, Observation, SpanId, SpanStarted, SpanStatus,
    Timestamp, TraceId,
};

const TRACE: &str = "0123456789abcdef0123456789abcdef";
const SPAN: &str = "0123456789abcdef";
const PARENT: &str = "1111111111111111";
const LINK: &str = "2222222222222222";
const FLAGS: u8 = 0xa5;
const LINK_FLAGS: u8 = 0x81;
const METRIC: &str = "composition.payload";
const START: u64 = 1_000_000_000;

pub struct Projector;
fn timestamp(seconds: u8) -> Timestamp {
    serde_json::from_str(&format!("\"1970-01-01T00:00:{seconds:02}Z\"")).expect("fixture timestamp")
}
fn trace() -> v2::TraceContext {
    v2::TraceContext::new(
        TraceId::new(TRACE).expect("trace"),
        SpanId::new(SPAN).expect("span"),
        v2::TraceFlags::new(FLAGS),
    )
    .with_parent(SpanId::new(PARENT).expect("parent"))
}
impl v2::SpanProjector<LogEvent> for Projector {
    fn project_spans(
        &self,
        observation: &Observation<LogEvent>,
    ) -> Result<Vec<v2::SpanSignal>, v2::ProjectionError> {
        let event = &observation.payload;
        let started = v2::SpanRecord::<SpanStarted>::new(
            timestamp(1),
            event.service.clone(),
            event.action.clone(),
            trace(),
            v2::Attributes::from([(
                "bridge.message".to_owned(),
                v2::AttributeValue::String(event.message.clone().expect("bridge message")),
            )]),
        )
        .with_kind(v2::SpanKind::Client)
        .with_links(vec![v2::SpanLink::new(
            TraceId::new(TRACE).expect("linked trace"),
            SpanId::new(LINK).expect("linked span"),
            v2::TraceFlags::new(LINK_FLAGS),
            v2::Attributes::new(),
        )]);
        let point = v2::SpanEvent {
            timestamp: timestamp(1),
            trace: trace(),
            name: ActionName::new("composition.observed").expect("event"),
            attributes: v2::Attributes::new(),
            diagnostic: None,
        };
        let ended = started.clone().end(SpanStatus::Error, DurationMs::from(25));
        Ok(vec![
            v2::SpanSignal::Started(started),
            v2::SpanSignal::Event(point),
            v2::SpanSignal::Ended(ended),
        ])
    }
}
impl v2::MetricProjector<LogEvent> for Projector {
    fn project_metrics(
        &self,
        observation: &Observation<LogEvent>,
    ) -> Result<Vec<v2::MetricRecord>, v2::ProjectionError> {
        let point = v2::HistogramPoint::try_new(
            vec![
                v2::FiniteF64::new(1.0).expect("finite"),
                v2::FiniteF64::new(5.0).expect("finite"),
            ],
            vec![1, 2, 3],
            6,
            v2::FiniteF64::new(12.5).expect("finite sum"),
        )
        .expect("histogram");
        Ok(vec![
            v2::MetricRecord::try_new(
                timestamp(2),
                observation.payload.service.clone(),
                MetricName::new(METRIC).expect("metric"),
                v2::MetricValue::Histogram {
                    point,
                    temporality: v2::AggregationTemporality::Delta,
                    start_time: timestamp(1),
                },
            )
            .expect("metric record"),
        ])
    }
}
impl sc_observability_types::SpanProjector<LogEvent> for Projector {
    fn project_spans(
        &self,
        observation: &Observation<LogEvent>,
    ) -> Result<Vec<sc_observability_types::SpanSignal>, sc_observability_types::ProjectionError>
    {
        use sc_observability_types::{SpanRecord, SpanSignal, TraceContext};
        let event = &observation.payload;
        let started = SpanRecord::<SpanStarted>::new(
            timestamp(1),
            event.service.clone(),
            event.action.clone(),
            TraceContext {
                trace_id: TraceId::new(TRACE).expect("trace"),
                span_id: SpanId::new(SPAN).expect("span"),
                parent_span_id: Some(SpanId::new(PARENT).expect("parent")),
            },
            serde_json::Map::from_iter([(
                "bridge.message".to_owned(),
                serde_json::json!(event.message),
            )]),
        );
        let ended = started.clone().end(SpanStatus::Ok, DurationMs::from(25));
        Ok(vec![SpanSignal::Started(started), SpanSignal::Ended(ended)])
    }
}
impl sc_observability_types::MetricProjector<LogEvent> for Projector {
    fn project_metrics(
        &self,
        observation: &Observation<LogEvent>,
    ) -> Result<Vec<sc_observability_types::MetricRecord>, sc_observability_types::ProjectionError>
    {
        Ok(vec![sc_observability_types::MetricRecord {
            timestamp: timestamp(2),
            service: observation.payload.service.clone(),
            name: MetricName::new(METRIC).expect("metric"),
            kind: sc_observability_types::MetricKind::Gauge,
            value: f64::from(
                u32::try_from(observation.payload.message.as_ref().expect("message").len())
                    .expect("short fixture"),
            ),
            unit: None,
            attributes: serde_json::Map::new(),
        }])
    }
}

pub fn assert_grpc(exports: &super::grpc_collector::Exports, message: &str, released: bool) {
    use opentelemetry_proto::tonic::common::v1::any_value::Value;
    use opentelemetry_proto::tonic::metrics::v1::{metric::Data, number_data_point};
    let spans: Vec<_> = exports
        .traces
        .iter()
        .flat_map(|e| &e.resource_spans)
        .flat_map(|r| &r.scope_spans)
        .flat_map(|s| &s.spans)
        .collect();
    assert_eq!(spans.len(), 1, "real trace export");
    let span = spans[0];
    assert_eq!(
        span.trace_id,
        [
            1, 35, 69, 103, 137, 171, 205, 239, 1, 35, 69, 103, 137, 171, 205, 239
        ]
    );
    assert_eq!(span.span_id, [1, 35, 69, 103, 137, 171, 205, 239]);
    assert_eq!(span.parent_span_id, [17; 8]);
    assert_eq!(span.name, "composition.forward");
    assert_eq!(span.start_time_unix_nano, START);
    assert_eq!(span.end_time_unix_nano, START + 25_000_000);
    assert!(span.attributes.iter().any(|a| a.key == "bridge.message"
        && a.value.as_ref().and_then(|v| v.value.as_ref())
            == Some(&Value::StringValue(message.to_owned()))));
    if released {
        assert!(span.links.is_empty());
        assert_eq!(span.status.as_ref().map(|s| s.code), Some(1));
    } else {
        assert_eq!(span.flags & 0xff, u32::from(FLAGS));
        assert_eq!(span.kind, 3);
        assert_eq!(span.status.as_ref().map(|s| s.code), Some(2));
        assert_eq!(span.events.len(), 1);
        assert_eq!(span.events[0].name, "composition.observed");
        assert_eq!(span.links.len(), 1);
        assert_eq!(span.links[0].span_id, [34; 8]);
        assert_eq!(span.links[0].flags & 0xff, u32::from(LINK_FLAGS));
    }
    let metrics: Vec<_> = exports
        .metrics
        .iter()
        .flat_map(|e| &e.resource_metrics)
        .flat_map(|r| &r.scope_metrics)
        .flat_map(|s| &s.metrics)
        .collect();
    assert_eq!(metrics.len(), 1, "real metric export");
    assert_eq!(metrics[0].name, METRIC);
    if released {
        let Some(Data::Gauge(gauge)) = &metrics[0].data else {
            panic!("released gauge preserved")
        };
        assert_eq!(gauge.data_points.len(), 1);
        assert_eq!(
            gauge.data_points[0].value,
            Some(number_data_point::Value::AsDouble(f64::from(
                u32::try_from(message.len()).expect("short fixture")
            )))
        );
    } else {
        let Some(Data::Histogram(histogram)) = &metrics[0].data else {
            panic!("canonical histogram preserved")
        };
        assert_eq!(histogram.aggregation_temporality, 1);
        assert_eq!(histogram.data_points.len(), 1);
        let point = &histogram.data_points[0];
        assert_eq!(point.explicit_bounds, [1.0, 5.0]);
        assert_eq!(point.bucket_counts, [1, 2, 3]);
        assert_eq!(point.count, 6);
        assert_eq!(point.sum, Some(12.5));
        assert_eq!(point.start_time_unix_nano, START);
        assert_eq!(point.time_unix_nano, 2 * START);
    }
}

pub fn assert_http(requests: &[super::http_collector::Captured], message: &str, released: bool) {
    fn body(requests: &[super::http_collector::Captured], path: &str) -> serde_json::Value {
        let matching: Vec<_> = requests.iter().filter(|r| r.path == path).collect();
        assert_eq!(matching.len(), 1, "one real signal export at {path}");
        assert_eq!(matching[0].method, "POST");
        assert!(
            matching[0]
                .content_type
                .as_deref()
                .is_some_and(|t| t.starts_with("application/json"))
        );
        serde_json::from_slice(&matching[0].body).expect("decoded JSON")
    }
    let traces = body(requests, "/v1/traces");
    let spans = traces["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .expect("spans");
    assert_eq!(spans.len(), 1);
    let span = &spans[0];
    assert_eq!(span["traceId"], TRACE);
    assert_eq!(span["spanId"], SPAN);
    assert_eq!(span["parentSpanId"], PARENT);
    assert_eq!(span["name"], "composition.forward");
    assert_eq!(span["startTimeUnixNano"], START.to_string());
    assert_eq!(span["endTimeUnixNano"], (START + 25_000_000).to_string());
    assert!(
        span["attributes"]
            .as_array()
            .expect("span attrs")
            .iter()
            .any(|a| a["key"] == "bridge.message" && a["value"]["stringValue"] == message)
    );
    if released {
        assert_eq!(span["status"]["code"], "STATUS_CODE_OK");
    } else {
        assert_eq!(span["flags"], FLAGS);
        assert_eq!(span["kind"], 3);
        assert_eq!(span["status"]["code"], "STATUS_CODE_ERROR");
        assert_eq!(span["events"].as_array().map(Vec::len), Some(1));
        assert_eq!(span["events"][0]["name"], "composition.observed");
        assert_eq!(span["links"].as_array().map(Vec::len), Some(1));
        assert_eq!(span["links"][0]["spanId"], LINK);
        assert_eq!(span["links"][0]["flags"], LINK_FLAGS);
    }
    let metrics = body(requests, "/v1/metrics");
    let metrics = metrics["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
        .as_array()
        .expect("metrics");
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0]["name"], METRIC);
    if released {
        assert_eq!(
            metrics[0]["gauge"]["dataPoints"][0]["asDouble"],
            serde_json::json!(f64::from(
                u32::try_from(message.len()).expect("short fixture")
            ))
        );
    } else {
        let histogram = &metrics[0]["histogram"];
        assert_eq!(histogram["aggregationTemporality"], 1);
        let point = &histogram["dataPoints"][0];
        assert_eq!(point["explicitBounds"], serde_json::json!([1.0, 5.0]));
        assert_eq!(point["bucketCounts"], serde_json::json!([1, 2, 3]));
        assert_eq!(point["count"], 6);
        assert_eq!(point["sum"], 12.5);
        assert_eq!(point["startTimeUnixNano"], START.to_string());
        assert_eq!(point["timeUnixNano"], (2 * START).to_string());
    }
}
