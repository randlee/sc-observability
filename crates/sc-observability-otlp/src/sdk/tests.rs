//! Focused caller-runtime tests for the official SDK adapter.

use super::build_exporter_set;
use super::implementation::{CallerRuntime, group_by_resource, project_metrics};
use crate::config::{
    ExporterBackend, OtelConfig, OtlpEndpoint, OtlpProtocol, validated_backend_connection,
    validated_transport_bounds,
};
use crate::contracts::{ExportRecord, InstrumentationScope, Resource};
use sc_observability_types::v2::{
    AttributeValue, Attributes, FiniteF64, MetricRecord, MetricValue,
};
use sc_observability_types::{MetricName, ServiceName, Timestamp};

#[test]
fn sdk_adapter_requires_an_entered_caller_runtime() {
    assert!(CallerRuntime::try_capture().is_none());
}

#[test]
fn sdk_adapter_captures_the_entered_caller_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");

    runtime.block_on(async {
        assert!(CallerRuntime::try_capture().is_some());
    });
}

#[test]
fn sdk_constructor_builds_one_shared_admission_core_from_explicit_connection() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let mut config = OtelConfig::new(ExporterBackend::OpenTelemetrySdk, OtlpProtocol::Grpc);
    config.enabled = true;
    config.endpoint = Some(OtlpEndpoint::new_typed("http://127.0.0.1:4317").expect("endpoint"));
    let bounds = validated_transport_bounds(&config).expect("bounds");
    let connection = validated_backend_connection(&config).expect("connection");

    runtime.block_on(async move {
        let adapter = build_exporter_set(connection, &bounds).expect("SDK adapter set");
        assert_eq!(adapter.lifecycle.health().admitted_records, 0);
        // The handoff returns both capabilities and the exact core that the
        // capabilities delegate to; no second core is constructed here.
        let _ = adapter.exporters.lifecycle.flush_async().await;
    });
}

#[test]
fn resource_grouping_keeps_each_resource_and_its_record_order() {
    let first = Resource {
        attributes: Attributes::from([(
            "resource.id".to_owned(),
            AttributeValue::String("first".to_owned()),
        )]),
        schema_url: Some("https://example.test/one".to_owned()),
    };
    let second = Resource {
        attributes: Attributes::from([(
            "resource.id".to_owned(),
            AttributeValue::String("second".to_owned()),
        )]),
        schema_url: Some("https://example.test/two".to_owned()),
    };
    let scope = InstrumentationScope::default();
    let groups = group_by_resource(&[
        ExportRecord {
            resource: first.clone(),
            scope: scope.clone(),
            record: 1_u8,
        },
        ExportRecord {
            resource: second.clone(),
            scope: scope.clone(),
            record: 2_u8,
        },
        ExportRecord {
            resource: first.clone(),
            scope: scope.clone(),
            record: 3_u8,
        },
    ]);

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].resource, first);
    assert_eq!(groups[0].scopes[0].scope, scope);
    assert_eq!(groups[0].scopes[0].records, vec![1, 3]);
    assert_eq!(groups[1].resource, second);
    assert_eq!(groups[1].scopes[0].records, vec![2]);
}

#[test]
fn metric_projection_keeps_resource_scope_and_histogram_distribution() {
    let resource = Resource {
        attributes: Attributes::from([(
            "service.instance.id".to_owned(),
            AttributeValue::String("blue-1".to_owned()),
        )]),
        schema_url: Some("https://schema.example/resource".to_owned()),
    };
    let scope = InstrumentationScope {
        name: "checkout".to_owned(),
        version: Some("1.2.3".to_owned()),
        schema_url: Some("https://schema.example/scope".to_owned()),
        attributes: Attributes::from([(
            "library.language".to_owned(),
            AttributeValue::String("rust".to_owned()),
        )]),
    };
    let histogram = sc_observability_types::v2::HistogramPoint::try_new(
        vec![FiniteF64::new(10.0).expect("finite bound")],
        vec![2, 1],
        3,
        FiniteF64::new(18.0).expect("finite sum"),
    )
    .expect("valid histogram");
    let metric = MetricRecord::try_new(
        Timestamp::UNIX_EPOCH,
        ServiceName::new("checkout").expect("service"),
        MetricName::new("request.duration").expect("metric name"),
        MetricValue::Histogram {
            point: histogram,
            temporality: sc_observability_types::v2::AggregationTemporality::Cumulative,
            start_time: Timestamp::UNIX_EPOCH,
        },
    )
    .expect("metric")
    .with_attributes(Attributes::from([(
        "http.route".to_owned(),
        AttributeValue::String("/checkout".to_owned()),
    )]));

    let projected = project_metrics(&[ExportRecord {
        resource,
        scope,
        record: metric,
    }]);

    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].schema_url, "https://schema.example/resource");
    assert_eq!(
        projected[0].scope_metrics[0]
            .scope
            .as_ref()
            .expect("scope")
            .name,
        "checkout"
    );
    assert_eq!(
        projected[0].scope_metrics[0].metrics[0].name,
        "request.duration"
    );
    let Some(opentelemetry_proto::tonic::metrics::v1::metric::Data::Histogram(histogram)) =
        &projected[0].scope_metrics[0].metrics[0].data
    else {
        panic!("expected histogram projection");
    };
    assert_eq!(histogram.data_points[0].bucket_counts, vec![2, 1]);
    assert_eq!(histogram.data_points[0].explicit_bounds, vec![10.0]);
    assert_eq!(histogram.data_points[0].count, 3);
    assert_eq!(histogram.data_points[0].sum, Some(18.0));
}
