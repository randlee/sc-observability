//! Caller-runtime OTLP SDK configuration example.
//!
//! D.7 intentionally demonstrates configuration and the application-owned
//! Tokio host only. D.18 owns wiring this adapter into the root telemetry
//! facade, so this example must not activate a second lifecycle or exporter.

use sc_observability_otlp::v2::{
    ExporterBackend, LogsConfig, MetricsConfig, OtelConfig, OtlpProtocol, TelemetryConfigBuilder,
    TracesConfig,
};
use sc_observability_types::{DurationMs, ServiceName};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // The embedding application owns the runtime. A future enabled adapter
    // captures this handle; it never creates or blocks on an internal runtime.
    let mut transport = OtelConfig::new(ExporterBackend::OpenTelemetrySdk, OtlpProtocol::Grpc);
    transport.enabled = false;
    transport.queue_capacity = Some(1_024);
    transport.queue_byte_capacity = Some(1_048_576);
    transport.timeout_ms = Some(DurationMs::from(5_000));
    transport.lifecycle_flush_timeout_ms = Some(DurationMs::from(5_000));
    transport.lifecycle_shutdown_timeout_ms = Some(DurationMs::from(10_000));

    let _config = TelemetryConfigBuilder::new(
        ServiceName::new("otlp-sdk-example").expect("literal service name is valid"),
    )
    .with_transport(transport)
    .enable_logs(LogsConfig::default())
    .enable_traces(TracesConfig::default())
    .enable_metrics(MetricsConfig::default())
    .build_typed()
    .expect("disabled configuration is valid");
}
