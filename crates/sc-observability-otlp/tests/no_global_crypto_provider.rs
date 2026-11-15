//! Each test binary starts with its own process-global rustls state.
#![cfg(any(feature = "sync-http", feature = "otlp-sdk"))]

use sc_observability_otlp::v2::{
    ExporterBackend, LogsConfig, OtelConfig, OtlpEndpoint, OtlpProtocol, Telemetry,
    TelemetryConfigBuilder,
};
use sc_observability_types::ServiceName;

fn build(backend: ExporterBackend, protocol: OtlpProtocol) -> Telemetry {
    let mut transport = OtelConfig::default();
    transport.enabled = true;
    transport.backend = backend;
    transport.protocol = protocol;
    transport.endpoint = Some(OtlpEndpoint::new_typed("https://localhost:4318").unwrap());
    let config = TelemetryConfigBuilder::new(ServiceName::new("provider-test").unwrap())
        .enable_logs(LogsConfig::default())
        .with_transport(transport)
        .build_typed()
        .unwrap();
    Telemetry::new_typed(config).unwrap()
}

#[test]
fn clients_do_not_install_a_process_global_provider() {
    assert!(rustls::crypto::CryptoProvider::get_default().is_none());
    #[cfg(feature = "sync-http")]
    {
        let telemetry = build(ExporterBackend::SyncHttp, OtlpProtocol::HttpJson);
        assert!(rustls::crypto::CryptoProvider::get_default().is_none());
        telemetry.shutdown().unwrap();
    }
    #[cfg(feature = "otlp-sdk")]
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let telemetry = build(ExporterBackend::OpenTelemetrySdk, OtlpProtocol::HttpBinary);
            assert!(rustls::crypto::CryptoProvider::get_default().is_none());
            telemetry.shutdown_async().await.unwrap();
        });
    }
    assert!(rustls::crypto::CryptoProvider::get_default().is_none());
}
