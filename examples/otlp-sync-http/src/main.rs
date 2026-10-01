//! Minimal D.21 legacy HTTP/JSON configuration fixture.

use sc_observability_otlp::v2::{
    ExporterBackend, LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, TelemetryConfigBuilder,
};
use sc_observability_types::ServiceName;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut transport = OtelConfig::new(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
    transport.enabled = true;
    transport.endpoint = Some(OtlpEndpoint::new_typed("http://127.0.0.1:4318")?);

    let config = TelemetryConfigBuilder::new(ServiceName::new("otlp-sync-http-example")?)
        .with_transport(transport)
        .enable_logs(LogsConfig::default())
        .build_typed()?;

    println!(
        "validated legacy OTLP/HTTP JSON contract for {} at {}",
        config.service_name,
        config
            .transport
            .endpoint
            .as_ref()
            .expect("fixture endpoint")
    );
    Ok(())
}
