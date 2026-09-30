use super::*;
use crate::config::{
    ExporterBackend, LogsConfig, MetricsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol,
    TelemetryConfigBuilder, TracesConfig,
};
use crate::testing::{
    LifecycleCall, RecordingLifecycle, RecordingLogExporter, RecordingMetricExporter,
    RecordingTraceExporter,
};
use crate::{
    CanonicalTelemetryError as TelemetryError, RuntimeTelemetry as Telemetry,
    RuntimeTelemetryConfig as TelemetryConfig,
};
use sc_observability_types::DiagnosticInfo;
use sc_observability_types::{
    ActionName, Diagnostic, DurationMs, ErrorCode, Level, LogEvent, MetricKind, MetricName,
    ProcessIdentity, ServiceName, SpanEvent, SpanId, SpanRecord, SpanStarted, StateTransition,
    TargetCategory, Timestamp, TraceContext, TraceId,
};
use serde_json::{Map, json};

use crate::assembly::span_key;

struct SourcePreservingLogExporter;

impl LogExporter<LogEvent> for SourcePreservingLogExporter {
    fn export_logs(&self, _batch: &[LogEvent]) -> Result<(), ExportError> {
        Err(ExportError::Transport {
            context: Box::new(
                ErrorContext::new(
                    ErrorCode::new_static("SC_TEST_CUSTOM_EXPORT"),
                    "custom log exporter failed",
                    Remediation::not_recoverable("test native exporter source retention"),
                )
                .source(Box::new(std::io::Error::other(
                    "custom exporter native source",
                ))),
            ),
        })
    }
}

fn service_name() -> ServiceName {
    ServiceName::new("test-service").expect("valid service")
}

fn schema_version() -> sc_observability_types::SchemaVersion {
    sc_observability_types::SchemaVersion::new(
        sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION,
    )
    .expect("valid schema version")
}

fn outcome_label(value: &str) -> sc_observability_types::OutcomeLabel {
    sc_observability_types::OutcomeLabel::new(value).expect("valid outcome label")
}

fn telemetry_config() -> TelemetryConfig {
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .enable_traces(TracesConfig::default())
        .enable_metrics(MetricsConfig::default())
        .with_transport(OtelConfig {
            enabled: true,
            endpoint: Some(
                OtlpEndpoint::new_typed("https://otel.example.internal")
                    .expect("valid OTLP endpoint"),
            ),
            ..OtelConfig::default()
        })
        .build_typed()
        .expect("valid telemetry config")
}

fn legacy_telemetry_config(protocol: OtlpProtocol) -> TelemetryConfig {
    TelemetryConfigBuilder::new(service_name())
        .enable_logs(LogsConfig::default())
        .with_transport(OtelConfig {
            enabled: true,
            backend: ExporterBackend::LegacyHttpJson,
            protocol,
            endpoint: Some(
                OtlpEndpoint::new_typed("https://otel.example.internal")
                    .expect("valid OTLP endpoint"),
            ),
            ..OtelConfig::default()
        })
        .build_typed()
        .expect("valid legacy telemetry config")
}

/// Test-only construction seam: enabled production configurations must
/// receive concrete exporters rather than the factory inventing a
/// successful fallback exporter.
fn test_telemetry(config: TelemetryConfig) -> Telemetry {
    Telemetry::new_with_exporters(
        config,
        Arc::new(RecordingLogExporter::default()),
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("test telemetry")
}

fn test_telemetry_typed(config: TelemetryConfig) -> Telemetry {
    Telemetry::new_with_exporters_typed(
        config,
        Arc::new(RecordingLogExporter::default()),
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("typed test telemetry")
}

fn trace_context() -> TraceContext {
    TraceContext {
        trace_id: TraceId::new("0123456789abcdef0123456789abcdef").expect("valid trace id"),
        span_id: SpanId::new("0123456789abcdef").expect("valid span id"),
        parent_span_id: None,
    }
}

#[test]
fn trace_projection_preserves_parent_span_identity() {
    let parent = SpanId::new("fedcba9876543210").expect("valid parent span id");
    let trace = TraceContext {
        parent_span_id: Some(parent.clone()),
        ..trace_context()
    };

    let projected = super::trace_context(&trace);

    assert_eq!(projected.parent_span_id, Some(parent));
}

fn log_event(service: ServiceName, message: &str) -> LogEvent {
    LogEvent {
        version: schema_version(),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service,
        target: TargetCategory::new("test.agent").expect("valid target"),
        action: ActionName::new("agent.observe").expect("valid action"),
        message: Some(message.to_string()),
        identity: ProcessIdentity::default(),
        trace: Some(trace_context()),
        request_id: None,
        correlation_id: None,
        outcome: Some(outcome_label("ok")),
        diagnostic: Some(Diagnostic {
            timestamp: Timestamp::UNIX_EPOCH,
            code: ErrorCode::new_static("SC_TEST"),
            message: "projected".to_string(),
            cause: None,
            remediation: Remediation::recoverable("retry", ["inspect telemetry"]),
            docs: None,
            details: Map::new(),
        }),
        state_transition: Some(StateTransition {
            entity_kind: TargetCategory::new("agent").expect("valid target"),
            entity_id: Some(String::from("agent-123")),
            from_state: sc_observability_types::StateName::new("idle").expect("valid state"),
            to_state: sc_observability_types::StateName::new("running").expect("valid state"),
            reason: None,
            trigger: None,
        }),
        fields: Map::from_iter([("kind".to_string(), json!(message))]),
    }
}

fn metric_record() -> MetricRecord {
    MetricRecord {
        timestamp: Timestamp::UNIX_EPOCH,
        service: service_name(),
        name: MetricName::new("agent.events_total").expect("valid metric"),
        kind: MetricKind::Counter,
        value: 1.0,
        unit: Some(sc_observability_types::MetricUnit::new("1").expect("valid unit")),
        attributes: Map::new(),
    }
}

fn complete_span_signals() -> (SpanSignal, SpanSignal) {
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace_context(),
        Map::new(),
    );
    let ended = started
        .clone()
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(5));
    (SpanSignal::Started(started), SpanSignal::Ended(ended))
}

#[test]
fn telemetry_config_builder_defaults() {
    // TelemetryConfig is constructed independently of ObservabilityConfig (OTLP-018).
    let config = TelemetryConfigBuilder::new(service_name())
        .build_typed()
        .expect("valid config");

    assert!(config.logs.is_none());
    assert!(config.traces.is_none());
    assert!(config.metrics.is_none());
    assert!(!config.transport.enabled);
    assert_eq!(config.transport.protocol, OtlpProtocol::HttpBinary);
}

#[test]
fn telemetry_constructors_preserve_invalid_configuration_diagnostics() {
    let config = TelemetryConfig {
        service_name: service_name(),
        resource: ResourceAttributes::default(),
        transport: OtelConfig {
            enabled: true,
            endpoint: None,
            ..OtelConfig::default()
        },
        logs: Some(LogsConfig::default()),
        traces: None,
        metrics: None,
    };
    let Err(legacy) = Telemetry::new(config.clone()) else {
        panic!("legacy invalid config should fail");
    };
    let Err(typed) = Telemetry::new_typed(config) else {
        panic!("typed invalid config should fail");
    };

    assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
    assert_eq!(legacy.diagnostic().message, typed.diagnostic().message);
    assert_eq!(legacy.diagnostic().cause, typed.diagnostic().cause);
    assert_eq!(
        legacy.diagnostic().remediation,
        typed.diagnostic().remediation
    );
    let legacy_context = std::error::Error::source(&legacy).expect("legacy context");
    let typed_context = std::error::Error::source(&typed).expect("typed context");
    assert!(legacy_context.source().is_none());
    assert!(typed_context.source().is_none());
}

#[test]
fn enabled_backend_factory_returns_canonical_unsupported_backend() {
    let config = telemetry_config();
    let bounds = validated_transport_bounds(&config.transport).expect("valid transport");
    let Err(factory_error) = exporter_factory(&config, &bounds) else {
        panic!("an enabled backend needs an installed implementation");
    };
    #[cfg(not(feature = "otlp-sdk"))]
    {
        assert!(matches!(
            factory_error,
            ConfigFailure::UnsupportedBackend { .. }
        ));
        assert_eq!(
            factory_error.diagnostic().code,
            sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_BACKEND
        );
        assert_eq!(
            factory_error.diagnostic().details["backend"].as_str(),
            Some("opentelemetry_sdk")
        );
        assert_eq!(
            factory_error.diagnostic().details["feature"].as_str(),
            Some("otlp-sdk")
        );
    }
    #[cfg(feature = "otlp-sdk")]
    {
        assert!(matches!(
            factory_error,
            ConfigFailure::TokioRuntimeRequired { .. }
        ));
        assert_eq!(
            factory_error.diagnostic().code,
            sc_observability_types::error_codes::otlp::OTLP_TOKIO_RUNTIME_REQUIRED
        );
    }

    let Err(constructor_error) = Telemetry::new_typed(telemetry_config()) else {
        panic!("the retained constructor preserves the factory diagnostic");
    };
    #[cfg(not(feature = "otlp-sdk"))]
    assert_eq!(
        constructor_error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_BACKEND
    );
    #[cfg(feature = "otlp-sdk")]
    assert_eq!(
        constructor_error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_TOKIO_RUNTIME_REQUIRED
    );
}

#[test]
fn injected_exporter_set_routes_enabled_signals_and_reports_health() {
    let log_exporter = Arc::new(RecordingLogExporter::default());
    let trace_exporter = Arc::new(RecordingTraceExporter::default());
    let metric_exporter = Arc::new(RecordingMetricExporter::default());
    let telemetry = Telemetry::new_with_exporters(
        telemetry_config(),
        log_exporter.clone(),
        trace_exporter.clone(),
        metric_exporter.clone(),
    )
    .expect("enabled telemetry with injected exporters");

    telemetry
        .emit_log(&log_event(service_name(), "injected routing"))
        .expect("route log");
    let (started, ended) = complete_span_signals();
    telemetry.emit_span(&started).expect("route span start");
    telemetry.emit_span(&ended).expect("route span end");
    telemetry
        .emit_metric(&metric_record())
        .expect("route metric");
    telemetry.flush().expect("flush injected exporters");

    assert_eq!(*log_exporter.calls.lock().expect("calls poisoned"), vec![1]);
    assert_eq!(
        *trace_exporter.calls.lock().expect("calls poisoned"),
        vec![1]
    );
    assert_eq!(
        *metric_exporter.calls.lock().expect("calls poisoned"),
        vec![1]
    );
    let health = telemetry.health();
    assert_eq!(health.state, TelemetryHealthState::Healthy);
    assert!(
        health
            .exporter_statuses
            .iter()
            .all(|status| status.state == ExporterHealthState::Healthy)
    );
}

#[test]
fn legacy_factory_rejects_non_json_protocol_before_backend_availability() {
    let mut invalid_config = legacy_telemetry_config(OtlpProtocol::HttpJson);
    invalid_config.transport.protocol = OtlpProtocol::HttpBinary;
    let bounds = validated_transport_bounds(&invalid_config.transport)
        .expect("configuration bounds precede backend availability");
    let Err(protocol_error) = exporter_factory(&invalid_config, &bounds) else {
        panic!("the legacy HTTP/JSON backend must reject a binary protocol");
    };
    assert!(matches!(
        protocol_error,
        ConfigFailure::UnsupportedProtocol { .. }
    ));
    assert_eq!(
        protocol_error.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_UNSUPPORTED_PROTOCOL
    );
    assert_eq!(
        protocol_error.diagnostic().details["backend"].as_str(),
        Some("legacy_http_json")
    );

    let config = legacy_telemetry_config(OtlpProtocol::HttpJson);
    let bounds = validated_transport_bounds(&config.transport).expect("valid transport");
    #[cfg(feature = "legacy-http-json")]
    {
        let exporters = exporter_factory(&config, &bounds)
            .expect("the configured legacy backend is composed when enabled");
        exporters.lifecycle.blocking_preflight().expect("preflight");
    }
    #[cfg(not(feature = "legacy-http-json"))]
    {
        let Err(backend_error) = exporter_factory(&config, &bounds) else {
            panic!("the disabled legacy feature must reject construction");
        };
        assert!(matches!(
            backend_error,
            ConfigFailure::UnsupportedBackend { .. }
        ));
    }
}

#[test]
fn sdk_factory_rejects_http_json_before_feature_availability() {
    let mut config = telemetry_config();
    config.transport.protocol = OtlpProtocol::HttpJson;
    let bounds = validated_transport_bounds(&config.transport).expect("valid bounds");
    let Err(error) = exporter_factory(&config, &bounds) else {
        panic!("SDK has no HTTP/JSON transport");
    };
    assert!(matches!(error, ConfigFailure::UnsupportedProtocol { .. }));
    assert_eq!(
        error.diagnostic().details["backend"].as_str(),
        Some("opentelemetry_sdk")
    );
    assert_eq!(
        error.diagnostic().details["supported_protocols"].as_str(),
        Some("Grpc, HttpBinary")
    );
}

#[cfg(feature = "otlp-sdk")]
#[test]
fn sdk_factory_requires_a_caller_tokio_runtime_after_feature_checks() {
    let config = telemetry_config();
    let bounds = validated_transport_bounds(&config.transport).expect("valid bounds");
    let Err(error) = exporter_factory(&config, &bounds) else {
        panic!("no caller runtime is entered");
    };
    assert!(matches!(error, ConfigFailure::TokioRuntimeRequired { .. }));
    assert_eq!(
        error.diagnostic().details["feature"].as_str(),
        Some("otlp-sdk")
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let exporters = exporter_factory(&config, &bounds)
            .expect("the SDK adapter is composed when a caller runtime is entered");
        exporters.lifecycle.blocking_preflight().expect("preflight");
    });
}

#[test]
fn disabled_factory_constructs_and_telemetry_carries_one_exporter_set() {
    let config = TelemetryConfigBuilder::new(service_name())
        .build_typed()
        .expect("disabled configuration is valid");
    let bounds = validated_transport_bounds(&config.transport).expect("valid transport");
    let exporters = exporter_factory(&config, &bounds).expect("disabled factory set");
    exporters.logs.export_logs(&[]).expect("disabled logs");
    exporters.traces.export_spans(&[]).expect("disabled traces");
    exporters
        .metrics
        .export_metrics(&[])
        .expect("disabled metrics");
    exporters
        .lifecycle
        .blocking_preflight()
        .expect("disabled lifecycle");

    let telemetry = Telemetry::new_typed(config).expect("disabled telemetry");
    telemetry
        .exporters
        .lifecycle
        .flush_blocking()
        .expect("telemetry owns the factory lifecycle");
}

#[test]
fn facade_routes_blocking_lifecycle_calls_to_the_shared_exporter_set() {
    let lifecycle = Arc::new(RecordingLifecycle::default());
    let telemetry = Telemetry::new_with_exporter_set_typed(
        telemetry_config(),
        ExporterSet {
            logs: Arc::new(RecordingLogExporter::default()),
            traces: Arc::new(RecordingTraceExporter::default()),
            metrics: Arc::new(RecordingMetricExporter::default()),
            lifecycle: lifecycle.clone(),
        },
    )
    .expect("injected exporter set");

    telemetry.flush_typed().expect("shared flush");
    telemetry.shutdown_typed().expect("shared shutdown");

    assert_eq!(
        *lifecycle.calls.lock().expect("lifecycle calls"),
        vec![
            LifecycleCall::BlockingPreflight,
            LifecycleCall::FlushBlocking,
            LifecycleCall::BlockingPreflight,
            LifecycleCall::ShutdownBlocking
        ]
    );
}

#[test]
fn all_signals_disabled_rejects_at_construction() {
    let config = TelemetryConfigBuilder::new(service_name())
        .with_transport(OtelConfig {
            enabled: false,
            endpoint: None,
            ..OtelConfig::default()
        })
        .build_typed()
        .expect("valid config");

    assert!(Telemetry::new(config).is_ok());
}

#[test]
fn span_assembler_emits_complete_span_from_lifecycle() {
    let trace = trace_context();
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace.clone(),
        Map::new(),
    );
    let ended = started
        .clone()
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(42));
    let mut assembler = SpanAssembler::new();

    assert!(
        assembler
            .push(SpanSignal::Started(started))
            .expect("started")
            .is_none()
    );
    assert!(
        assembler
            .push(SpanSignal::Event(SpanEvent {
                timestamp: Timestamp::UNIX_EPOCH,
                trace: trace.clone(),
                name: ActionName::new("tool.call").expect("valid name"),
                attributes: Map::new(),
                diagnostic: None,
            }))
            .expect("event")
            .is_none()
    );
    let complete = assembler
        .push(SpanSignal::Ended(ended))
        .expect("ended")
        .expect("complete span");

    assert_eq!(complete.events.len(), 1);
    assert_eq!(complete.record.duration_ms(), Some(DurationMs::from(42)));
}

#[test]
fn bounded_span_assembly_evicts_oldest_span_and_excess_events_with_accounting() {
    let first_trace = trace_context();
    let mut second_trace = first_trace.clone();
    second_trace.span_id = SpanId::new("fedcba9876543210").expect("valid second span id");
    let start = |trace: TraceContext| {
        SpanSignal::Started(SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            trace,
            Map::new(),
        ))
    };
    let mut assembler = SpanAssembler::with_limits(1, 1);

    assembler
        .push_typed(start(first_trace.clone()))
        .expect("first span");
    assembler
        .push_typed(SpanSignal::Event(SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace: first_trace.clone(),
            name: ActionName::new("first.event").expect("valid event"),
            attributes: Map::new(),
            diagnostic: None,
        }))
        .expect("first event");
    assembler
        .push_typed(SpanSignal::Event(SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace: first_trace.clone(),
            name: ActionName::new("second.event").expect("valid event"),
            attributes: Map::new(),
            diagnostic: None,
        }))
        .expect("bounded event is accounted rather than rejected");
    assembler
        .push_typed(start(second_trace.clone()))
        .expect("second span");

    assert!(!assembler.has_started(&first_trace.trace_id, &first_trace.span_id));
    assert!(assembler.has_started(&second_trace.trace_id, &second_trace.span_id));
    assert_eq!(
        assembler.take_loss(),
        SpanAssemblyLoss {
            evicted_spans: 1,
            evicted_events: 1,
        }
    );
}

#[test]
fn typed_span_assembler_matches_legacy_completion() {
    let trace = trace_context();
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace.clone(),
        Map::new(),
    );
    let ended = started
        .clone()
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(42));
    let mut assembler = SpanAssembler::new();

    assert!(
        assembler
            .push_typed(SpanSignal::Started(started))
            .expect("typed started")
            .is_none()
    );
    let complete = assembler
        .push_typed(SpanSignal::Ended(ended))
        .expect("typed ended")
        .expect("complete span");

    assert!(complete.events.is_empty());
    assert_eq!(complete.record.duration_ms(), Some(DurationMs::from(42)));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the paired lifecycle transitions remain adjacent so parity is auditable"
)]
fn span_assembler_typed_and_legacy_errors_preserve_lifecycle_diagnostics() {
    fn assert_parity<L, T>(legacy: &L, typed: &T, expected_message: &str)
    where
        L: DiagnosticInfo + std::error::Error,
        T: DiagnosticInfo + std::error::Error,
    {
        assert_eq!(legacy.diagnostic().code, typed.diagnostic().code);
        assert_eq!(legacy.diagnostic().message, expected_message);
        assert_eq!(typed.diagnostic().message, expected_message);
        assert_eq!(legacy.diagnostic().cause, typed.diagnostic().cause);
        assert_eq!(
            legacy.diagnostic().remediation,
            typed.diagnostic().remediation
        );
        if let Some(legacy_context) = std::error::Error::source(legacy) {
            assert!(std::error::Error::source(legacy_context).is_none());
        }
        if let Some(typed_context) = std::error::Error::source(typed) {
            assert!(std::error::Error::source(typed_context).is_none());
        }
    }

    let orphan_trace = trace_context();
    let orphan_event = SpanSignal::Event(SpanEvent {
        timestamp: Timestamp::UNIX_EPOCH,
        trace: orphan_trace.clone(),
        name: ActionName::new("tool.call").expect("valid name"),
        attributes: Map::new(),
        diagnostic: None,
    });
    let orphan_ended = SpanSignal::Ended(
        SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("agent.run").expect("valid action"),
            orphan_trace,
            Map::new(),
        )
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(42)),
    );
    let mut legacy = SpanAssembler::new();
    let mut typed = SpanAssembler::new();
    let error = legacy
        .push(orphan_event.clone())
        .expect_err("legacy orphan event");
    let typed_error = typed
        .push_typed(orphan_event)
        .expect_err("typed orphan event");
    assert_parity(
        &error,
        &typed_error,
        "received span event without a matching started span",
    );

    let mut legacy = SpanAssembler::new();
    let mut typed = SpanAssembler::new();
    let error = legacy
        .push(orphan_ended.clone())
        .expect_err("legacy orphan ended span");
    let typed_error = typed
        .push_typed(orphan_ended)
        .expect_err("typed orphan ended span");
    assert_parity(
        &error,
        &typed_error,
        "received ended span without a matching started span",
    );

    let trace = trace_context();
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace.clone(),
        Map::new(),
    );
    let ended = started
        .clone()
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(42));
    let mut legacy = SpanAssembler::new();
    let mut typed = SpanAssembler::new();

    assert!(
        legacy
            .push(SpanSignal::Started(started.clone()))
            .expect("legacy started")
            .is_none()
    );
    assert!(
        typed
            .push_typed(SpanSignal::Started(started))
            .expect("typed started")
            .is_none()
    );
    let key = span_key(&trace.trace_id, &trace.span_id);
    legacy.remove_event_buffer(&key);
    typed.remove_event_buffer(&key);

    let legacy_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        legacy.push(SpanSignal::Ended(ended.clone()))
    }));
    let typed_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        typed.push_typed(SpanSignal::Ended(ended))
    }));
    assert!(
        legacy_panic.is_err(),
        "legacy internal invariant must panic"
    );
    assert!(typed_panic.is_err(), "typed internal invariant must panic");
}

#[test]
fn v1_assembly_rejects_mismatched_parent_without_losing_started_state() {
    let trace = trace_context();
    let mut mismatched = trace.clone();
    mismatched.parent_span_id = Some(SpanId::new("fedcba9876543210").expect("valid parent"));
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace,
        Map::new(),
    );
    let mismatched_ended = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        mismatched.clone(),
        Map::new(),
    )
    .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(1));
    let bad_signals = [
        SpanSignal::Event(SpanEvent {
            timestamp: Timestamp::UNIX_EPOCH,
            trace: mismatched,
            name: ActionName::new("tool.call").expect("valid event"),
            attributes: Map::new(),
            diagnostic: None,
        }),
        SpanSignal::Ended(mismatched_ended),
    ];
    for signal in bad_signals {
        let mut assembler = SpanAssembler::new();
        assembler
            .push_typed(SpanSignal::Started(started.clone()))
            .expect("started");
        let error = assembler.push_typed(signal).expect_err("parent mismatch");
        assert_eq!(
            error.diagnostic().code,
            error_codes::OTLP_SPAN_ASSEMBLY_FAILED
        );
        let ended = started
            .clone()
            .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(2));
        let complete = assembler
            .push_typed(SpanSignal::Ended(ended))
            .expect("valid end")
            .expect("retained started state");
        assert!(
            complete.events.is_empty(),
            "rejected event must not be buffered"
        );
    }
}

#[test]
fn incomplete_span_drop_accounting_is_paired_for_legacy_and_typed_shutdown() {
    let trace = trace_context();
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace,
        Map::new(),
    );
    let legacy = test_telemetry(telemetry_config());
    let typed = test_telemetry_typed(telemetry_config());

    legacy
        .emit_span(&SpanSignal::Started(started.clone()))
        .expect("legacy started");
    typed
        .emit_span(&SpanSignal::Started(started))
        .expect("typed started");
    legacy.shutdown().expect("legacy shutdown");
    typed.shutdown_typed().expect("typed shutdown");

    let legacy_health = legacy.health();
    let typed_health = typed.health();
    assert_eq!(legacy_health.dropped_exports_total, 1);
    assert_eq!(
        legacy_health.dropped_exports_total,
        typed_health.dropped_exports_total
    );
    assert_eq!(legacy_health.state, TelemetryHealthState::Unavailable);
    assert_eq!(legacy_health.state, typed_health.state);
}

#[test]
fn orphaned_ended_span_returns_export_failure_and_is_counted() {
    let trace = trace_context();
    let telemetry = test_telemetry(telemetry_config());
    let ended = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace,
        Map::new(),
    )
    .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(5));

    assert!(matches!(
        telemetry.emit_span(&SpanSignal::Ended(ended)),
        Err(TelemetryError::ExportFailure(_))
    ));
    let health = telemetry.health();
    assert_eq!(health.malformed_spans_total, 1);
    assert_eq!(
        health.last_error.and_then(|summary| summary.code),
        Some(error_codes::OTLP_SPAN_ASSEMBLY_FAILED)
    );
}

#[test]
fn orphaned_span_event_returns_export_failure_from_span_assembler() {
    let trace = trace_context();
    let telemetry = test_telemetry(telemetry_config());
    let event = SpanEvent {
        timestamp: Timestamp::UNIX_EPOCH,
        trace,
        name: ActionName::new("agent.tool_call").expect("valid action"),
        attributes: Map::new(),
        diagnostic: None,
    };

    let error = telemetry
        .emit_span(&SpanSignal::Event(event))
        .expect_err("event without a matching started span is rejected");
    let TelemetryError::ExportFailure(context) = error else {
        panic!("expected an export failure");
    };
    assert_eq!(
        context.diagnostic().code,
        error_codes::OTLP_SPAN_ASSEMBLY_FAILED
    );
    assert_eq!(
        context.diagnostic().message,
        "received span event without a matching started span"
    );
}

#[test]
fn export_failure_from_event_moves_original_context_without_reconstruction() {
    let context = Box::new(
        ErrorContext::new(
            error_codes::OTLP_EXPORT_TERMINAL,
            "received span event without a matching started span",
            Remediation::not_recoverable("emit started, event, and ended span signals in order"),
        )
        .source(Box::new(std::io::Error::other(
            "orphaned span event source",
        ))),
    );
    let original_timestamp = context.diagnostic().timestamp;
    let original_backtrace_ptr = std::ptr::from_ref(context.backtrace());
    let failure = EventFailure::from_context(context);

    let TelemetryError::ExportFailure(exported) = export_failure_from_event(failure) else {
        panic!("expected an export failure");
    };

    assert_eq!(exported.diagnostic().timestamp, original_timestamp);
    assert_eq!(
        std::ptr::from_ref(exported.context().backtrace()),
        original_backtrace_ptr,
        "the original context's backtrace allocation must be moved, not recaptured"
    );
    let source = std::error::Error::source(exported.context()).expect("source must be preserved");
    assert_eq!(source.to_string(), "orphaned span event source");
}

#[test]
fn shutdown_flush_failure_preserves_flush_context_as_native_source() {
    let flush_failure = FlushFailure::from_context(Box::new(
        ErrorContext::new(
            error_codes::OTLP_EXPORT_TERMINAL,
            "log export failed",
            Remediation::not_recoverable("test flush failure"),
        )
        .source(Box::new(std::io::Error::other("native flush source"))),
    ));

    let shutdown_failure = shutdown_flush_failure(flush_failure);

    assert_eq!(
        shutdown_failure.diagnostic().code,
        error_codes::OTLP_FLUSH_FAILED
    );
    assert_eq!(
        shutdown_failure.diagnostic().message,
        "failed to flush telemetry during shutdown"
    );
    let shutdown_context = std::error::Error::source(&shutdown_failure)
        .expect("shutdown failure preserves its context");
    let chained_flush_failure = shutdown_context
        .source()
        .expect("shutdown context preserves the flush failure");
    assert_eq!(
        chained_flush_failure.to_string(),
        "log export failed; caused by: native flush source"
    );
    let flush_context = chained_flush_failure
        .source()
        .expect("flush failure preserves its context");
    let native_source = flush_context
        .source()
        .expect("flush context preserves the native export source");
    assert_eq!(native_source.to_string(), "native flush source");
}

#[test]
fn exporter_failure_accounting_is_tracked() {
    let log_exporter = Arc::new(RecordingLogExporter::default());
    log_exporter.fail.store(true, Ordering::SeqCst);
    let telemetry = Telemetry::new_with_exporters(
        telemetry_config(),
        log_exporter,
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("telemetry");

    telemetry
        .emit_log(&log_event(service_name(), "export"))
        .expect("emit");
    let result = telemetry.flush();

    assert!(result.is_ok());
    let health = telemetry.health();
    assert_eq!(health.state, TelemetryHealthState::Degraded);
    assert_eq!(health.dropped_exports_total, 1);
    assert_eq!(
        health.exporter_statuses[0].state,
        ExporterHealthState::Degraded
    );
}

#[test]
fn exporter_health_recovers_after_a_successful_flush() {
    let log_exporter = Arc::new(RecordingLogExporter::default());
    log_exporter.fail.store(true, Ordering::SeqCst);
    let telemetry = Telemetry::new_with_exporters(
        telemetry_config(),
        log_exporter.clone(),
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("telemetry");

    telemetry
        .emit_log(&log_event(service_name(), "first"))
        .expect("emit first");
    telemetry.flush().expect("first flush remains fail-open");
    assert_eq!(telemetry.health().state, TelemetryHealthState::Degraded);

    log_exporter.fail.store(false, Ordering::SeqCst);
    telemetry
        .emit_log(&log_event(service_name(), "second"))
        .expect("emit second");
    telemetry.flush().expect("second flush");

    let health = telemetry.health();
    assert_eq!(health.state, TelemetryHealthState::Healthy);
    assert_eq!(
        health.exporter_statuses[0].state,
        ExporterHealthState::Healthy
    );
    assert!(health.exporter_statuses[0].last_error.is_none());
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the paired exporter matrix remains adjacent so all six consumers are auditable"
)]
fn legacy_and_typed_flush_record_and_recover_all_exporter_families() {
    let legacy_log = Arc::new(RecordingLogExporter::default());
    let legacy_trace = Arc::new(RecordingTraceExporter::default());
    let legacy_metric = Arc::new(RecordingMetricExporter::default());
    let typed_log = Arc::new(RecordingLogExporter::default());
    let typed_trace = Arc::new(RecordingTraceExporter::default());
    let typed_metric = Arc::new(RecordingMetricExporter::default());
    for failure_switch in [
        &legacy_log.fail,
        &legacy_trace.fail,
        &legacy_metric.fail,
        &typed_log.fail,
        &typed_trace.fail,
        &typed_metric.fail,
    ] {
        failure_switch.store(true, Ordering::SeqCst);
    }
    let legacy = Telemetry::new_with_exporters(
        telemetry_config(),
        legacy_log.clone(),
        legacy_trace.clone(),
        legacy_metric.clone(),
    )
    .expect("legacy telemetry");
    let typed = Telemetry::new_with_exporters_typed(
        telemetry_config(),
        typed_log.clone(),
        typed_trace.clone(),
        typed_metric.clone(),
    )
    .expect("typed telemetry");

    legacy
        .emit_log(&log_event(service_name(), "first"))
        .expect("legacy log");
    let (started, ended) = complete_span_signals();
    legacy.emit_span(&started).expect("legacy started");
    legacy.emit_span(&ended).expect("legacy ended");
    legacy.emit_metric(&metric_record()).expect("legacy metric");
    typed
        .emit_log(&log_event(service_name(), "first"))
        .expect("typed log");
    let (started, ended) = complete_span_signals();
    typed.emit_span(&started).expect("typed started");
    typed.emit_span(&ended).expect("typed ended");
    typed.emit_metric(&metric_record()).expect("typed metric");
    legacy.flush().expect("legacy fail-open flush");
    typed.flush_typed().expect("typed fail-open flush");
    assert!(
        legacy
            .health()
            .exporter_statuses
            .iter()
            .all(|status| status.state == ExporterHealthState::Degraded)
    );
    assert!(
        typed
            .health()
            .exporter_statuses
            .iter()
            .all(|status| status.state == ExporterHealthState::Degraded)
    );

    for failure_switch in [
        &legacy_log.fail,
        &legacy_trace.fail,
        &legacy_metric.fail,
        &typed_log.fail,
        &typed_trace.fail,
        &typed_metric.fail,
    ] {
        failure_switch.store(false, Ordering::SeqCst);
    }
    legacy
        .emit_log(&log_event(service_name(), "second"))
        .expect("legacy log");
    let (started, ended) = complete_span_signals();
    legacy.emit_span(&started).expect("legacy started");
    legacy.emit_span(&ended).expect("legacy ended");
    legacy.emit_metric(&metric_record()).expect("legacy metric");
    typed
        .emit_log(&log_event(service_name(), "second"))
        .expect("typed log");
    let (started, ended) = complete_span_signals();
    typed.emit_span(&started).expect("typed started");
    typed.emit_span(&ended).expect("typed ended");
    typed.emit_metric(&metric_record()).expect("typed metric");
    legacy.flush().expect("legacy recovery flush");
    typed.flush_typed().expect("typed recovery flush");
    assert!(
        legacy
            .health()
            .exporter_statuses
            .iter()
            .all(|status| status.state == ExporterHealthState::Healthy)
    );
    assert!(
        typed
            .health()
            .exporter_statuses
            .iter()
            .all(|status| status.state == ExporterHealthState::Healthy)
    );
    for calls in [
        &legacy_log.calls,
        &legacy_trace.calls,
        &legacy_metric.calls,
        &typed_log.calls,
        &typed_trace.calls,
        &typed_metric.calls,
    ] {
        assert_eq!(*calls.lock().expect("calls poisoned"), vec![1, 1]);
    }
}

#[test]
fn retained_emit_methods_return_shutdown_after_legacy_and_typed_lifecycle() {
    // Emitters intentionally remain the B.1 legacy public surface. This
    // pairs their unchanged calls after each lifecycle entry point rather
    // than adding parallel typed emitter methods to this preparation layer.
    let legacy = test_telemetry(telemetry_config());
    let typed = test_telemetry_typed(telemetry_config());
    let trace = trace_context();
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("agent.run").expect("valid action"),
        trace,
        Map::new(),
    );
    let metric = MetricRecord {
        timestamp: Timestamp::UNIX_EPOCH,
        service: service_name(),
        name: MetricName::new("agent.events_total").expect("valid metric"),
        kind: MetricKind::Counter,
        value: 1.0,
        unit: Some(sc_observability_types::MetricUnit::new("1").expect("valid metric unit")),
        attributes: Map::new(),
    };

    legacy.shutdown().expect("legacy shutdown");
    typed.shutdown_typed().expect("typed shutdown");

    assert!(matches!(
        legacy.emit_log(&log_event(service_name(), "after-shutdown")),
        Err(TelemetryError::Shutdown { .. })
    ));
    assert!(matches!(
        legacy.emit_span(&SpanSignal::Started(started.clone())),
        Err(TelemetryError::Shutdown { .. })
    ));
    assert!(matches!(
        legacy.emit_metric(&metric),
        Err(TelemetryError::Shutdown { .. })
    ));
    assert!(matches!(
        typed.emit_log(&log_event(service_name(), "after-shutdown")),
        Err(TelemetryError::Shutdown { .. })
    ));
    assert!(matches!(
        typed.emit_span(&SpanSignal::Started(started)),
        Err(TelemetryError::Shutdown { .. })
    ));
    assert!(matches!(
        typed.emit_metric(&metric),
        Err(TelemetryError::Shutdown { .. })
    ));
}

#[test]
fn shutdown_flushes_complete_spans_and_counts_incomplete_ones() {
    let trace_exporter = Arc::new(RecordingTraceExporter::default());
    let telemetry = Telemetry::new_with_exporters(
        telemetry_config(),
        Arc::new(RecordingLogExporter::default()),
        trace_exporter.clone(),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("telemetry");

    let complete_trace = trace_context();
    let started = SpanRecord::<SpanStarted>::new(
        Timestamp::UNIX_EPOCH,
        service_name(),
        ActionName::new("complete.run").expect("valid action"),
        complete_trace.clone(),
        Map::new(),
    );
    let ended = started
        .clone()
        .end(sc_observability_types::SpanStatus::Ok, DurationMs::from(5));
    telemetry
        .emit_span(&SpanSignal::Started(started))
        .expect("started");
    telemetry
        .emit_span(&SpanSignal::Ended(ended))
        .expect("ended");

    let incomplete_trace = TraceContext {
        trace_id: TraceId::new("abcdefabcdefabcdefabcdefabcdefab").expect("valid trace"),
        span_id: SpanId::new("abcdefabcdefabcd").expect("valid span"),
        parent_span_id: None,
    };
    telemetry
        .emit_span(&SpanSignal::Started(SpanRecord::<SpanStarted>::new(
            Timestamp::UNIX_EPOCH,
            service_name(),
            ActionName::new("incomplete.run").expect("valid action"),
            incomplete_trace,
            Map::new(),
        )))
        .expect("incomplete");

    telemetry.shutdown().expect("shutdown");

    assert_eq!(
        *trace_exporter.calls.lock().expect("calls poisoned"),
        vec![1]
    );
    assert_eq!(telemetry.health().dropped_exports_total, 1);
}

#[test]
fn repeated_shutdown_is_idempotent() {
    let telemetry = test_telemetry(telemetry_config());

    telemetry.shutdown().expect("first shutdown");
    telemetry.shutdown().expect("second shutdown");
}

#[test]
fn typed_telemetry_lifecycle_preserves_fail_open_and_repeat_shutdown() {
    let telemetry = test_telemetry_typed(telemetry_config());

    telemetry
        .flush_typed()
        .expect("typed flush remains fail-open");
    telemetry.shutdown_typed().expect("typed first shutdown");
    telemetry.shutdown_typed().expect("typed repeated shutdown");
}

#[test]
fn shutdown_propagates_flush_failures_with_legacy_and_typed_parity() {
    let legacy_exporter = Arc::new(RecordingLogExporter::default());
    legacy_exporter.fail.store(true, Ordering::SeqCst);
    let typed_exporter = Arc::new(RecordingLogExporter::default());
    typed_exporter.fail.store(true, Ordering::SeqCst);
    let legacy = Telemetry::new_with_exporters(
        telemetry_config(),
        legacy_exporter,
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("legacy telemetry");
    let typed = Telemetry::new_with_exporters_typed(
        telemetry_config(),
        typed_exporter,
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("typed telemetry");

    legacy
        .emit_log(&log_event(service_name(), "shutdown-export"))
        .expect("legacy emit");
    typed
        .emit_log(&log_event(service_name(), "shutdown-export"))
        .expect("typed emit");

    let legacy_error = legacy
        .shutdown()
        .expect_err("legacy shutdown should surface flush failures");
    let typed_error = typed
        .shutdown_typed()
        .expect_err("typed shutdown should surface flush failures");
    assert_eq!(
        legacy_error.diagnostic().code,
        typed_error.diagnostic().code
    );
    assert_eq!(
        legacy_error.diagnostic().message,
        "failed to flush telemetry during shutdown"
    );
    assert_eq!(
        legacy_error.diagnostic().message,
        typed_error.diagnostic().message
    );
    assert_eq!(
        legacy_error.diagnostic().details,
        typed_error.diagnostic().details
    );

    let legacy_health = legacy.health();
    let typed_health = typed.health();
    assert_eq!(legacy_health.state, TelemetryHealthState::Unavailable);
    assert_eq!(legacy_health.state, typed_health.state);
    assert_eq!(legacy_health.dropped_exports_total, 1);
    assert_eq!(
        legacy_health.dropped_exports_total,
        typed_health.dropped_exports_total
    );
    assert_eq!(
        legacy_health.exporter_statuses[0].state,
        ExporterHealthState::Degraded
    );
    assert_eq!(
        legacy_health.exporter_statuses[0].state,
        typed_health.exporter_statuses[0].state
    );
}

#[test]
fn shutdown_preserves_custom_export_code_and_native_source_for_both_apis() {
    fn assert_custom_source<E>(error: &E, telemetry: &Telemetry)
    where
        E: DiagnosticInfo + std::error::Error,
    {
        assert_eq!(
            error.diagnostic().details.get("exporter_error_code"),
            Some(&Value::String("SC_TEST_CUSTOM_EXPORT".to_owned()))
        );
        assert_eq!(
            telemetry
                .health()
                .last_error
                .and_then(|summary| summary.code),
            Some(ErrorCode::new_static("SC_TEST_CUSTOM_EXPORT"))
        );

        let mut source = std::error::Error::source(error);
        let mut saw_export_failure = false;
        let mut final_source = None;
        while let Some(current) = source {
            saw_export_failure |= current.downcast_ref::<ExportError>().is_some();
            final_source = Some(current.to_string());
            source = current.source();
        }
        assert!(
            saw_export_failure,
            "source chain retains the export failure"
        );
        assert_eq!(
            final_source.as_deref(),
            Some("custom exporter native source"),
            "source chain ends at the native exporter source"
        );
    }

    let legacy = Telemetry::new_with_exporters(
        telemetry_config(),
        Arc::new(SourcePreservingLogExporter),
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("legacy telemetry");
    let typed = Telemetry::new_with_exporters_typed(
        telemetry_config(),
        Arc::new(SourcePreservingLogExporter),
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("typed telemetry");
    legacy
        .emit_log(&log_event(service_name(), "shutdown-export"))
        .expect("legacy emit");
    typed
        .emit_log(&log_event(service_name(), "shutdown-export"))
        .expect("typed emit");

    let legacy_error = legacy
        .shutdown()
        .expect_err("legacy shutdown should retain the exporter failure");
    let typed_error = typed
        .shutdown_typed()
        .expect_err("typed shutdown should retain the exporter failure");
    assert_custom_source(&legacy_error, &legacy);
    assert_custom_source(&typed_error, &typed);
}

#[test]
fn combined_export_failure_and_incomplete_span_preserve_baseline_shutdown_summary() {
    fn assert_baseline_summary<E>(error: &E, telemetry: &Telemetry)
    where
        E: DiagnosticInfo,
    {
        assert_eq!(error.diagnostic().code, error_codes::OTLP_FLUSH_FAILED);
        assert_eq!(
            error.diagnostic().cause.as_deref(),
            Some("dropped incomplete spans during shutdown")
        );
        assert_eq!(
            error.diagnostic().details.get("exporter_error_code"),
            Some(&Value::String(
                error_codes::OTLP_INCOMPLETE_SPAN_DROPPED
                    .as_str()
                    .to_owned(),
            ))
        );
        let health = telemetry.health();
        assert_eq!(health.state, TelemetryHealthState::Unavailable);
        assert_eq!(health.dropped_exports_total, 2);
        assert_eq!(
            health.last_error.and_then(|summary| summary.code),
            Some(error_codes::OTLP_INCOMPLETE_SPAN_DROPPED)
        );
        assert_eq!(
            health.exporter_statuses[0].state,
            ExporterHealthState::Degraded
        );
        assert_eq!(
            health.exporter_statuses[1].state,
            ExporterHealthState::Degraded
        );
    }

    let legacy_exporter = Arc::new(RecordingLogExporter::default());
    legacy_exporter.fail.store(true, Ordering::SeqCst);
    let typed_exporter = Arc::new(RecordingLogExporter::default());
    typed_exporter.fail.store(true, Ordering::SeqCst);
    let legacy = Telemetry::new_with_exporters(
        telemetry_config(),
        legacy_exporter,
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("legacy telemetry");
    let typed = Telemetry::new_with_exporters_typed(
        telemetry_config(),
        typed_exporter,
        Arc::new(RecordingTraceExporter::default()),
        Arc::new(RecordingMetricExporter::default()),
    )
    .expect("typed telemetry");

    for telemetry in [&legacy, &typed] {
        telemetry
            .emit_log(&log_event(service_name(), "shutdown-export"))
            .expect("emit log");
        let (started, _) = complete_span_signals();
        telemetry.emit_span(&started).expect("emit incomplete span");
    }

    let legacy_error = legacy
        .shutdown()
        .expect_err("legacy shutdown should report final export failure");
    let typed_error = typed
        .shutdown_typed()
        .expect_err("typed shutdown should report final export failure");
    assert_baseline_summary(&legacy_error, &legacy);
    assert_baseline_summary(&typed_error, &typed);
    assert_eq!(
        legacy_error.diagnostic().code,
        typed_error.diagnostic().code
    );
    assert_eq!(
        legacy_error.diagnostic().cause,
        typed_error.diagnostic().cause
    );
    assert_eq!(
        legacy_error.diagnostic().details,
        typed_error.diagnostic().details
    );

    legacy.shutdown().expect("legacy repeated shutdown");
    typed.shutdown_typed().expect("typed repeated shutdown");
}

// Released-versus-canonical `entity_id` admission. The active (exporting) state
// is only constructible through injected exporters, so the buffered-state
// assertions live here; `tests/released_emit_log.rs` covers the public surface.
mod entity_admission {
    use super::*;
    use crate::{TelemetryProjectors, V2TelemetryProjectors};
    use sc_observability_types::v2::FailureClassification;
    use sc_observability_types::{Observation, ProjectionRegistration};

    const INVALID_ID: &str = "entity invalid";

    fn event_with_id(id: &str) -> LogEvent {
        let mut event = log_event(service_name(), "entity");
        if let Some(transition) = event.state_transition.as_mut() {
            transition.entity_id = Some(id.to_string());
        }
        event
    }

    fn active() -> (Telemetry, Arc<RecordingLogExporter<LogEvent>>) {
        let exporter = Arc::new(RecordingLogExporter::<LogEvent>::default());
        let telemetry = Telemetry::new_with_exporters(
            telemetry_config(),
            exporter.clone(),
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("active telemetry");
        (telemetry, exporter)
    }

    fn disabled() -> (Telemetry, Arc<RecordingLogExporter<LogEvent>>) {
        let exporter = Arc::new(RecordingLogExporter::<LogEvent>::default());
        let config = TelemetryConfigBuilder::new(service_name())
            .enable_logs(LogsConfig::default())
            .build_typed()
            .expect("disabled config");
        let telemetry = Telemetry::new_with_exporters(
            config,
            exporter.clone(),
            Arc::new(RecordingTraceExporter::default()),
            Arc::new(RecordingMetricExporter::default()),
        )
        .expect("disabled telemetry");
        (telemetry, exporter)
    }

    fn exported(exporter: &RecordingLogExporter<LogEvent>) -> usize {
        exporter
            .batches
            .lock()
            .expect("batches poisoned")
            .iter()
            .map(Vec::len)
            .sum()
    }

    fn observation() -> Observation<u8> {
        Observation::new(service_name(), 1_u8)
    }

    struct FixedLogProjector(&'static str);

    impl sc_observability_types::LogProjector<u8> for FixedLogProjector {
        fn project_logs(
            &self,
            _observation: &Observation<u8>,
        ) -> Result<Vec<LogEvent>, sc_observability_types::v2::ProjectionError> {
            Ok(vec![event_with_id(self.0)])
        }
    }

    #[test]
    fn v2_emit_log_inactive_with_invalid_id_reports_shutdown_first() {
        let (telemetry, exporter) = active();
        telemetry.shutdown_typed().expect("shutdown");
        let error = telemetry
            .emit_log(&event_with_id(INVALID_ID))
            .expect_err("closed admission");
        assert!(
            matches!(error, TelemetryError::Shutdown { .. }),
            "{error:?}"
        );
        assert_eq!(exported(&exporter), 0);
    }

    #[test]
    fn v2_emit_log_disabled_with_invalid_id_is_ok_and_unbuffered() {
        let (telemetry, exporter) = disabled();
        telemetry
            .emit_log(&event_with_id(INVALID_ID))
            .expect("disabled transport returns before the entity check");
        telemetry.flush().expect("flush");
        assert_eq!(exported(&exporter), 0);
    }

    #[test]
    fn v2_emit_log_active_with_invalid_id_returns_event_validation_unbuffered() {
        let (telemetry, exporter) = active();
        let error = telemetry
            .emit_log(&event_with_id(INVALID_ID))
            .expect_err("canonical admission rejects the id");
        assert!(matches!(error, TelemetryError::Event(_)), "{error:?}");
        assert_eq!(
            error.failure_classification(),
            FailureClassification::validation("event")
        );
        assert_ne!(error.failure_classification(), FailureClassification::Io);
        assert_eq!(
            error.code(),
            sc_observability_types::error_codes::DIAGNOSTIC_INVALID
        );
        telemetry.flush().expect("flush");
        assert_eq!(exported(&exporter), 0, "rejected events are never buffered");
    }

    #[test]
    fn v2_emit_log_active_with_valid_id_is_buffered() {
        let (telemetry, exporter) = active();
        telemetry
            .emit_log(&event_with_id("agent-123"))
            .expect("valid id");
        telemetry.flush().expect("flush");
        assert_eq!(exported(&exporter), 1);
    }

    #[test]
    fn root_emit_log_keeps_released_acceptance() {
        let (runtime, exporter) = active();
        let root = crate::Telemetry::from_runtime(runtime);
        root.emit_log(&event_with_id(INVALID_ID))
            .expect("released facade has no entity check");
        root.emit_log(&event_with_id("agent-123")).expect("valid");
        assert_eq!(root.runtime().flush().map(|()| exported(&exporter)), Ok(2));

        let (runtime, _exporter) = active();
        let closed = crate::Telemetry::from_runtime(runtime);
        closed.runtime().shutdown_typed().expect("shutdown");
        #[allow(deprecated, reason = "asserts the released legacy Shutdown variant")]
        let shutdown = matches!(
            closed.emit_log(&event_with_id(INVALID_ID)),
            Err(crate::TelemetryError::Shutdown)
        );
        assert!(shutdown);

        let (runtime, exporter) = disabled();
        let disabled = crate::Telemetry::from_runtime(runtime);
        disabled
            .emit_log(&event_with_id(INVALID_ID))
            .expect("disabled");
        assert_eq!(exported(&exporter), 0);
    }

    #[test]
    fn root_projectors_keep_released_acceptance_and_v2_projectors_stay_canonical() {
        let (runtime, exporter) = active();
        let root = Arc::new(crate::Telemetry::from_runtime(runtime));
        let registration: ProjectionRegistration<u8> = TelemetryProjectors::new(root.clone())
            .with_log_projector(Arc::new(FixedLogProjector(INVALID_ID)))
            .into_registration();
        let (log_projector, _, _, _) = registration.into_parts();
        let events = log_projector
            .expect("log projector")
            .project_logs(&observation())
            .expect("released projector ingress accepts the id");
        assert_eq!(events.len(), 1);
        root.runtime().flush().expect("flush");
        assert_eq!(exported(&exporter), 1);

        let (v2, v2_exporter) = active();
        let v2 = Arc::new(v2);
        let registration: ProjectionRegistration<u8> = V2TelemetryProjectors::new(v2.clone())
            .with_log_projector(Arc::new(FixedLogProjector(INVALID_ID)))
            .into_registration();
        let (log_projector, _, _, _) = registration.into_parts();
        let error = log_projector
            .expect("log projector")
            .project_logs(&observation())
            .expect_err("canonical projector ingress rejects the id");
        assert!(
            matches!(
                error,
                sc_observability_types::v2::ProjectionError::Projection { .. }
            ),
            "{error:?}"
        );
        // The projection failure preserves the admission context code.
        assert_eq!(
            error.diagnostic().code,
            sc_observability::error_codes::LOGGER_INVALID_EVENT
        );
        v2.flush().expect("flush");
        assert_eq!(exported(&v2_exporter), 0);
    }
    #[test]
    fn v2_logger_and_v2_emit_log_accept_and_reject_entity_ids_identically() {
        let long_valid = "a".repeat(512);
        let cases: Vec<(String, &str)> = vec![
            ("worker-1".into(), "valid"),
            ("A.b_c-9".into(), "valid punctuation edge"),
            ("x".into(), "single character"),
            (long_valid, "long valid"),
            (String::new(), "empty"),
            (" ".into(), "space"),
            ("entity invalid".into(), "embedded space"),
            (" leading".into(), "leading space"),
            ("trailing ".into(), "trailing space"),
            ("tab\tid".into(), "tab"),
            ("new\nline".into(), "newline"),
            ("agent/1".into(), "slash"),
            ("caf\u{e9}".into(), "non-ascii"),
            ("a:b".into(), "colon"),
        ];

        let service = service_name();
        let log_root = std::env::temp_dir().join(format!(
            "sc-otlp-entity-parity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let logger_config =
            sc_observability::LoggerConfig::default_for(service.clone(), log_root.clone());
        let logger = sc_observability::v2::Logger::new(logger_config).expect("logger");
        let (telemetry, _exporter) = active();

        for (id, label) in cases {
            let expected_valid = sc_observability_types::EntityId::new(id.clone()).is_ok();
            let logger_accepts = logger.log(event_with_id(&id)).is_ok();
            let otlp_result = telemetry.emit_log(&event_with_id(&id));
            assert_eq!(logger_accepts, expected_valid, "logger: {label}");
            assert_eq!(otlp_result.is_ok(), expected_valid, "otlp: {label}");
            if let Err(error) = otlp_result {
                assert!(matches!(error, TelemetryError::Event(_)), "{label}");
            }
        }
        let _stopped = logger.shutdown();
        let _ = std::fs::remove_dir_all(&log_root);
    }
}
