//! Native OTLP export paths, driven by the telemetry end-to-end qualification.
//!
//! One scenario per process, because the `log` facade bridge installs once:
//!
//! ```text
//! otlp-native sync-client <endpoint>
//! otlp-native tokio <endpoint>
//! otlp-native compose-otel-only|compose-both|compose-file-only <endpoint> <log-root>
//! ```
//!
//! `<endpoint>` is an OTLP/HTTP base URL such as `http://127.0.0.1:4318`.
//! `sync-client` uses the blocking [`Client`]; `tokio` builds the official
//! SDK providers and OTLP exporters directly; the `compose-*` scenarios
//! register [`OtelLogSink`] on the core logger beside, or instead of, its file
//! sink and emit through the `sc-observability-log` macros and the `log` facade.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use opentelemetry::logs::{AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity};
use opentelemetry::metrics::MeterProvider as _;
use opentelemetry::trace::{
    Span as _, SpanKind, TraceContextExt as _, Tracer as _, TracerProvider as _,
};
use opentelemetry::{Context, InstrumentationScope, KeyValue};
use opentelemetry_otlp::{
    LogExporter, MetricExporter, Protocol, SpanExporter, WithExportConfig as _,
};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLoggerProvider};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{BatchSpanProcessor, SdkTracerProvider};
use sc_observability::v2::{LoggerBuilder, LoggerConfig, SinkRegistration};
use sc_observability_log::{
    ActionName, AttachmentOptions, BridgeEventDecision, BridgeEventPolicy, BridgeOptions, LogEvent,
    ServiceName, attach_logger,
};
use sc_observability_otlp::OtelLogSink;
use sc_observability_otlp::api::trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};
use sc_observability_otlp::sdk::trace::{SpanData, SpanEvents, SpanLinks};
use sc_observability_otlp::sync::Client;

type Failure = Box<dyn std::error::Error>;

fn main() -> Result<(), Failure> {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().ok_or("missing scenario")?;
    let endpoint = args.next().ok_or("missing endpoint")?;
    match scenario.as_str() {
        "sync-client" => sync_client(&endpoint),
        "tokio" => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(tokio_providers(&endpoint)),
        "compose-otel-only" => compose(
            &endpoint,
            &args.next().ok_or("missing log root")?,
            true,
            false,
        ),
        "compose-both" => compose(
            &endpoint,
            &args.next().ok_or("missing log root")?,
            true,
            true,
        ),
        "compose-file-only" => compose(
            &endpoint,
            &args.next().ok_or("missing log root")?,
            false,
            true,
        ),
        other => Err(format!("unknown scenario {other}").into()),
    }
}

fn resource(service: &str) -> Resource {
    Resource::builder_empty()
        .with_service_name(service.to_owned())
        .build()
}

fn scope() -> InstrumentationScope {
    InstrumentationScope::builder("e2e.native")
        .with_version("1.2.3")
        .build()
}

/// The blocking client sends one log, one span and one metric.
fn sync_client(endpoint: &str) -> Result<(), Failure> {
    let resource = resource("e2e-sync-client");
    let trace_id = TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e4736")?;
    let span_id = SpanId::from_hex("00f067aa0ba902b7")?;
    let mut client = Client::new(endpoint)?;
    client.send_log(&resource, scope(), |record| {
        record.set_timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000));
        record.set_severity_number(Severity::Warn);
        record.set_severity_text("WARN");
        record.set_body(AnyValue::from("sync client log"));
        record.set_trace_context(trace_id, span_id, None);
        record.add_attribute("job", "build");
        record.add_attribute("attempt", 2_i64);
        Ok(())
    })?;
    client.send_span(
        &resource,
        SpanData {
            span_context: SpanContext::new(
                trace_id,
                span_id,
                TraceFlags::SAMPLED,
                false,
                TraceState::NONE,
            ),
            parent_span_id: SpanId::INVALID,
            parent_span_is_remote: false,
            span_kind: SpanKind::Client,
            name: "sync client span".into(),
            start_time: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            end_time: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_005),
            attributes: vec![KeyValue::new("region", "west")],
            dropped_attributes_count: 0,
            events: SpanEvents::default(),
            links: SpanLinks::default(),
            status: opentelemetry::trace::Status::Ok,
            instrumentation_scope: scope(),
        },
    )?;
    client.send_metrics(&resource, scope(), |meter| {
        let counter = meter.f64_counter("sync.client.jobs").with_unit("1").build();
        counter.add(3.0, &[KeyValue::new("queue", "default")]);
        Ok(())
    })?;
    Ok(())
}

/// Standard official providers and OTLP/HTTP exporters inside a Tokio runtime.
async fn tokio_providers(endpoint: &str) -> Result<(), Failure> {
    let resource = resource("e2e-tokio");
    let logs = SdkLoggerProvider::builder()
        .with_resource(resource.clone())
        .with_log_processor(
            BatchLogProcessor::builder(
                LogExporter::builder()
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_endpoint(format!("{endpoint}/v1/logs"))
                    .build()?,
            )
            .build(),
        )
        .build();
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_span_processor(
            BatchSpanProcessor::builder(
                SpanExporter::builder()
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_endpoint(format!("{endpoint}/v1/traces"))
                    .build()?,
            )
            .build(),
        )
        .build();
    let metrics = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(
            PeriodicReader::builder(
                MetricExporter::builder()
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_endpoint(format!("{endpoint}/v1/metrics"))
                    .build()?,
            )
            .build(),
        )
        .build();

    let tracer = tracer_provider.tracer_with_scope(scope());
    let mut span = tracer
        .span_builder("tokio span")
        .with_kind(SpanKind::Server)
        .start(&tracer);
    span.set_attribute(KeyValue::new("region", "east"));
    // The log emitted inside the span's context carries that span's identity.
    let guard = Context::current_with_span(span).attach();
    let logger = logs.logger_with_scope(scope());
    let mut record = logger.create_log_record();
    record.set_timestamp(SystemTime::now());
    record.set_severity_number(Severity::Error);
    record.set_severity_text("ERROR");
    record.set_body(AnyValue::from("tokio log"));
    record.add_attribute("job", "deploy");
    logger.emit(record);
    Context::current().span().end();
    drop(guard);
    let counter = metrics
        .meter_with_scope(scope())
        .f64_counter("tokio.jobs")
        .with_unit("1")
        .build();
    counter.add(5.0, &[KeyValue::new("queue", "tokio")]);

    // The SDK shutdown calls block on the exporter; keep them off the runtime's async workers.
    tokio::task::spawn_blocking(
        move || -> Result<(), opentelemetry_sdk::error::OTelSdkError> {
            logs.shutdown()?;
            tracer_provider.shutdown()?;
            metrics.shutdown()
        },
    )
    .await??;
    Ok(())
}

/// Admits every assembled bridge event.
struct OpenPolicy;

impl BridgeEventPolicy for OpenPolicy {
    fn decide(&self, _event: &LogEvent) -> BridgeEventDecision {
        BridgeEventDecision::Admit
    }
}

/// Core logger composition: the built-in file sink and/or the native `OTel` sink.
fn compose(endpoint: &str, log_root: &str, otel: bool, file: bool) -> Result<(), Failure> {
    let mut config = LoggerConfig::default_for(ServiceName::new("e2e-compose")?, log_root.into());
    config.enable_file_sink = file;
    config.redaction.denylist_keys = vec!["password".to_owned()];
    let mut builder = LoggerBuilder::new(config)?;
    let provider = if otel {
        let provider = SdkLoggerProvider::builder()
            .with_resource(resource("e2e-compose"))
            .with_log_processor(
                BatchLogProcessor::builder(
                    LogExporter::builder()
                        .with_http()
                        .with_protocol(Protocol::HttpBinary)
                        .with_endpoint(format!("{endpoint}/v1/logs"))
                        .build()?,
                )
                .build(),
            )
            .build();
        builder.register_sink(SinkRegistration::typed(Arc::new(OtelLogSink::new(
            &provider,
            scope(),
        ))));
        Some(provider)
    } else {
        None
    };
    let logger = Arc::new(builder.build()?);
    let mut attachment = attach_logger(
        Arc::clone(&logger),
        AttachmentOptions::new(
            BridgeOptions {
                default_action: ActionName::new("log.record")?,
                parse_bracket_action: true,
            },
            Arc::new(OpenPolicy),
        ),
    )?;

    sc_observability_log::info!(
        target: "e2e.compose", password = "hunter2-secret", marker = "macro-event", "compose macro event"
    );
    log::info!(target: "e2e.compose", "compose bridge event");

    attachment.detach(Duration::from_secs(5))?;
    logger.flush_with_timeout(Duration::from_secs(10))?;
    logger.shutdown_with_timeout(Duration::from_secs(10))?;
    if let Some(provider) = provider {
        provider.shutdown()?;
    }
    Ok(())
}
