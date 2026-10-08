//! Core-logger mapping, nonblocking export and provider-ownership tests for
//! [`OtelLogSink`].

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

use opentelemetry::logs::{AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity};
use opentelemetry::{InstrumentationScope, Key};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::{
    BatchConfigBuilder, BatchLogProcessor, LogBatch, LogExporter, SdkLogRecord, SdkLoggerProvider,
};
use sc_observability::v2::{LogSink, LoggerBuilder, LoggerConfig, SinkRegistration};
use sc_observability_types::{
    ActionName, CorrelationId, Level, LogEvent, ProcessIdentity, ServiceName, SpanId,
    TargetCategory, Timestamp, TraceContext, TraceId,
};
use serde_json::json;

use super::OtelLogSink;

const FIXTURE_WATCHDOG: Duration = Duration::from_secs(10);
const TRACE_ID: &str = "0af7651916cd43dd8448eb211c80319c";
const SPAN_ID: &str = "b7ad6b7169203331";
const PARENT_SPAN_ID: &str = "a7ad6b7169203331";

/// Gate a test holds closed to stall every export until it is released.
#[derive(Default)]
struct ExportGate {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Default)]
struct GateState {
    closed: bool,
    exports_entered: usize,
}

impl ExportGate {
    fn closed() -> Arc<Self> {
        let gate = Self::default();
        gate.state.lock().expect("gate state").closed = true;
        Arc::new(gate)
    }

    fn enter_and_wait(&self) {
        let mut state = self.state.lock().expect("gate state");
        state.exports_entered += 1;
        self.changed.notify_all();
        let (_state, timeout) = self
            .changed
            .wait_timeout_while(state, FIXTURE_WATCHDOG, |state| state.closed)
            .expect("gate wait");
        assert!(!timeout.timed_out(), "test never released the export gate");
    }

    fn wait_for_export(&self) {
        let state = self.state.lock().expect("gate state");
        let (_state, timeout) = self
            .changed
            .wait_timeout_while(state, FIXTURE_WATCHDOG, |state| state.exports_entered == 0)
            .expect("gate wait");
        assert!(!timeout.timed_out(), "the batch processor never exported");
    }

    fn release(&self) {
        self.state.lock().expect("gate state").closed = false;
        self.changed.notify_all();
    }
}

/// Test exporter that records every exported record, optionally stalling.
#[derive(Clone, Debug, Default)]
struct CapturingExporter {
    records: Arc<Mutex<Vec<SdkLogRecord>>>,
    gate: Option<Arc<ExportGateHandle>>,
}

/// `Debug` wrapper so the exporter satisfies the SDK's `Debug` bound.
struct ExportGateHandle(Arc<ExportGate>);

impl std::fmt::Debug for ExportGateHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ExportGateHandle")
    }
}

impl LogExporter for CapturingExporter {
    fn export(&self, batch: LogBatch<'_>) -> impl Future<Output = OTelSdkResult> + Send {
        if let Some(gate) = &self.gate {
            gate.0.enter_and_wait();
        }
        let exported = batch.iter().map(|(record, _)| record.clone());
        self.records
            .lock()
            .expect("captured records")
            .extend(exported);
        std::future::ready(Ok(()))
    }
}

impl CapturingExporter {
    fn records(&self) -> Vec<SdkLogRecord> {
        self.records.lock().expect("captured records").clone()
    }
}

fn provider(exporter: CapturingExporter) -> SdkLoggerProvider {
    let processor = BatchLogProcessor::builder(exporter)
        .with_batch_config(
            BatchConfigBuilder::default()
                .with_max_export_batch_size(1)
                .build(),
        )
        .build();
    SdkLoggerProvider::builder()
        .with_resource(
            Resource::builder_empty()
                .with_service_name("otel-resource-service")
                .build(),
        )
        .with_log_processor(processor)
        .build()
}

fn scope() -> InstrumentationScope {
    InstrumentationScope::builder("core-logger").build()
}

fn event(target: &str) -> LogEvent {
    LogEvent {
        version: sc_observability_types::SchemaVersion::new(
            sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
        )
        .expect("schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Warn,
        service: ServiceName::new("core-event-service").expect("service name"),
        target: TargetCategory::new(target).expect("target"),
        action: ActionName::new("upload.retry").expect("action"),
        message: Some("Authorization: Bearer abc123".to_owned()),
        identity: ProcessIdentity::default(),
        trace: Some(TraceContext {
            trace_id: TraceId::new(TRACE_ID).expect("trace id"),
            span_id: SpanId::new(SPAN_ID).expect("span id"),
            parent_span_id: Some(SpanId::new(PARENT_SPAN_ID).expect("parent span id")),
        }),
        request_id: Some(CorrelationId::new("req-7").expect("request id")),
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::from_iter([
            ("token".to_owned(), json!("raw-secret")),
            ("attempt".to_owned(), json!(2)),
            ("ratio".to_owned(), json!(0.5)),
            ("cached".to_owned(), json!(true)),
            ("missing".to_owned(), json!(null)),
            (
                "shape".to_owned(),
                json!({"kind": "box", "contents": [true, null]}),
            ),
        ]),
    }
}

fn attribute(record: &SdkLogRecord, key: &str) -> Option<AnyValue> {
    let key = Key::from(key.to_owned());
    record
        .attributes_iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.clone())
}

fn text(value: &str) -> AnyValue {
    AnyValue::from(value.to_owned())
}

fn map(entries: impl IntoIterator<Item = (&'static str, AnyValue)>) -> AnyValue {
    AnyValue::Map(Box::new(
        entries
            .into_iter()
            .map(|(key, value)| (Key::from(key.to_owned()), value))
            .collect::<HashMap<_, _>>(),
    ))
}

#[test]
fn core_logger_events_are_redacted_then_mapped_to_native_records() {
    let exporter = CapturingExporter::default();
    let provider = provider(exporter.clone());
    let root = tempfile::tempdir().expect("log root");
    let mut config = LoggerConfig::default_for(
        ServiceName::new("core-event-service").expect("service name"),
        root.path().to_path_buf(),
    );
    config.redaction.denylist_keys.push("token".to_owned());
    config.enable_console_sink = false;
    let mut builder = LoggerBuilder::new(config).expect("logger builder");
    builder.register_sink(SinkRegistration::typed(Arc::new(OtelLogSink::new(
        &provider,
        scope(),
    ))));
    let logger = builder.build().expect("logger");
    logger.log(event("upload.client")).expect("event admitted");
    logger.flush().expect("core logger flush");
    provider.force_flush().expect("provider flush");

    let records = exporter.records();
    assert_eq!(records.len(), 1, "one event, one native record");
    let record = &records[0];
    assert_eq!(record.timestamp(), Some(SystemTime::UNIX_EPOCH));
    assert!(
        record.observed_timestamp() > Some(SystemTime::UNIX_EPOCH),
        "observed timestamp is the emit time, distinct from the event time"
    );
    assert_eq!(record.severity_number(), Some(Severity::Warn));
    assert_eq!(
        record.severity_text(),
        Some(crate::severity::fields(Level::Warn).1)
    );
    let Some(AnyValue::String(body)) = record.body() else {
        panic!("message maps to a string body");
    };
    assert!(
        !body.as_str().contains("abc123"),
        "bearer token redacted: {body}"
    );
    assert_eq!(record.target().map(AsRef::as_ref), Some("upload.client"));
    assert_eq!(attribute(record, "event.name"), Some(text("upload.retry")));
    assert_eq!(
        attribute(record, "sc.observability.log.service"),
        Some(text("core-event-service")),
        "the event service is an attribute distinct from the resource service"
    );
    assert_eq!(
        attribute(record, "sc.observability.log.request_id"),
        Some(text("\"req-7\""))
    );
    assert_eq!(
        attribute(record, "sc.observability.log.parent_span_id"),
        Some(text(PARENT_SPAN_ID))
    );
    assert_ne!(
        attribute(record, "token"),
        Some(text("raw-secret")),
        "denylisted field redacted"
    );
    assert!(
        attribute(record, "token").is_some(),
        "redacted field still present"
    );
    assert_eq!(attribute(record, "attempt"), Some(AnyValue::Int(2)));
    assert_eq!(attribute(record, "ratio"), Some(AnyValue::Double(0.5)));
    assert_eq!(attribute(record, "cached"), Some(AnyValue::Boolean(true)));
    assert_eq!(
        attribute(record, "missing"),
        Some(map([])),
        "null fields remain visible as the native empty-map representation"
    );
    assert_eq!(
        attribute(record, "shape"),
        Some(map([
            ("kind", text("box")),
            (
                "contents",
                AnyValue::ListAny(Box::new(vec![AnyValue::Boolean(true), map([]),])),
            ),
        ])),
        "objects and arrays retain their native recursive structure"
    );
    let context = record.trace_context().expect("valid trace context mapped");
    assert_eq!(context.trace_id.to_string(), TRACE_ID);
    assert_eq!(context.span_id.to_string(), SPAN_ID);

    logger.shutdown().expect("core logger shutdown");
    provider.shutdown().expect("provider shutdown");
}

#[test]
fn all_zero_trace_ids_are_not_mapped_as_trace_context() {
    let exporter = CapturingExporter::default();
    let provider = provider(exporter.clone());
    let sink = OtelLogSink::new(&provider, scope());
    let mut invalid = event("upload.client");
    invalid.trace = Some(TraceContext {
        trace_id: TraceId::new("0".repeat(32)).expect("well-formed trace id"),
        span_id: SpanId::new(SPAN_ID).expect("span id"),
        parent_span_id: None,
    });
    sink.write(&invalid).expect("write");
    provider.force_flush().expect("provider flush");
    let records = exporter.records();
    assert_eq!(records.len(), 1);
    assert!(records[0].trace_context().is_none());
    provider.shutdown().expect("provider shutdown");
}

#[test]
fn sdk_diagnostic_targets_are_dropped() {
    let exporter = CapturingExporter::default();
    let provider = provider(exporter.clone());
    let sink = OtelLogSink::new(&provider, scope());
    sink.write(&event("opentelemetry_sdk"))
        .expect("diagnostic write");
    sink.write(&event("opentelemetry.otlp"))
        .expect("diagnostic write");
    sink.write(&event("application"))
        .expect("application write");
    provider.force_flush().expect("provider flush");
    let records = exporter.records();
    assert_eq!(records.len(), 1, "only the application event is emitted");
    assert_eq!(records[0].target().map(AsRef::as_ref), Some("application"));
    provider.shutdown().expect("provider shutdown");
}

#[test]
fn write_and_flush_return_while_the_export_is_stalled() {
    let gate = ExportGate::closed();
    let exporter = CapturingExporter {
        gate: Some(Arc::new(ExportGateHandle(Arc::clone(&gate)))),
        ..CapturingExporter::default()
    };
    let provider = provider(exporter.clone());
    let sink = OtelLogSink::new(&provider, scope());
    sink.write(&event("application")).expect("first write");
    gate.wait_for_export();

    // The exporter is now blocked inside export and stays blocked until the
    // release below, so returning here proves write and flush never wait on
    // the network.
    for _ in 0..3 {
        sink.write(&event("application"))
            .expect("write while stalled");
    }
    sink.flush().expect("flush while stalled");
    assert_eq!(
        sink.health().state,
        sc_observability_types::SinkHealthState::Healthy
    );
    assert!(
        exporter.records().is_empty(),
        "nothing completed while stalled"
    );

    gate.release();
    provider
        .force_flush()
        .expect("provider flush after release");
    assert_eq!(exporter.records().len(), 4);
    provider.shutdown().expect("provider shutdown");
}

#[test]
fn provider_remains_usable_after_the_sink_is_dropped() {
    let exporter = CapturingExporter::default();
    let provider = provider(exporter.clone());
    let sink = OtelLogSink::new(&provider, scope());
    sink.write(&event("application")).expect("write");
    drop(sink);

    let logger = provider.logger("after-sink");
    let mut record = logger.create_log_record();
    record.set_body(AnyValue::from("direct"));
    record.add_attribute("direct", true);
    logger.emit(record);
    provider.force_flush().expect("provider flush");
    assert_eq!(exporter.records().len(), 2);
    provider.shutdown().expect("caller shuts the provider down");
}
