//! Executable consumer contracts for the released OTLP facade traits.

#![allow(
    deprecated,
    reason = "these tests exercise the released 1.x compatibility surface"
)]

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sc_observability_otlp::{
    LogsConfig, OtelConfig, Telemetry, TelemetryConfigBuilder, TelemetryProjectors,
};
use sc_observability_types::v2::ProjectionError;
use sc_observability_types::{
    ActionName, Level, LogEvent, LogProjector, ObservabilityHealthProvider, Observation,
    ProcessIdentity, SchemaVersion, ServiceName, TargetCategory, TelemetryHealthReport,
    TelemetryHealthState, Timestamp, ToolName,
};
use sc_observe::{Observability, ObservabilityConfig};
use serde_json::Map;

fn service() -> ServiceName {
    ServiceName::new("released-trait-contracts").expect("valid service")
}

fn disabled_telemetry() -> Telemetry {
    let transport = OtelConfig {
        enabled: false,
        ..OtelConfig::default()
    };
    let config = TelemetryConfigBuilder::new(service())
        .enable_logs(LogsConfig::default())
        .with_transport(transport)
        .build()
        .expect("valid disabled telemetry config");
    Telemetry::new(config).expect("disabled telemetry builds")
}

fn health_from_released_trait(provider: &dyn ObservabilityHealthProvider) -> TelemetryHealthReport {
    provider.telemetry_health()
}

#[test]
fn released_telemetry_provides_health_through_the_shared_trait() {
    let telemetry = disabled_telemetry();

    let report = health_from_released_trait(&telemetry);

    assert_eq!(report, telemetry.health());
    assert_eq!(report.state, TelemetryHealthState::Disabled);
}

#[derive(Debug)]
struct ProjectedPayload;

struct CountingProjector(Arc<AtomicUsize>);

impl LogProjector<ProjectedPayload> for CountingProjector {
    fn project_logs(
        &self,
        observation: &Observation<ProjectedPayload>,
    ) -> Result<Vec<LogEvent>, ProjectionError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![LogEvent {
            version: SchemaVersion::new(sc_observability_types::OBSERVATION_ENVELOPE_VERSION)
                .expect("valid schema version"),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: observation.service.clone(),
            target: TargetCategory::new("released.compat").expect("valid target"),
            action: ActionName::new("released.projected").expect("valid action"),
            message: Some("compat projector ran".to_owned()),
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: None,
            diagnostic: None,
            state_transition: None,
            fields: Map::new(),
        }])
    }
}

struct TestLogRoot(PathBuf);

impl TestLogRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "released-trait-contracts-{}-{nonce}",
            std::process::id()
        )))
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestLogRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn released_projectors_register_with_observability_and_run_for_old_consumers() {
    let telemetry = Arc::new(disabled_telemetry());
    let root = TestLogRoot::new();
    let projected = Arc::new(AtomicUsize::new(0));
    let config = ObservabilityConfig::default_for(
        ToolName::new("released-trait-contracts").expect("valid tool name"),
        root.path().to_owned(),
    )
    .expect("valid observability config");
    let runtime = Observability::builder(config)
        .with_observability_health_provider(telemetry.clone())
        .register_projection(
            TelemetryProjectors::new(telemetry.clone())
                .with_log_projector(Arc::new(CountingProjector(Arc::clone(&projected))))
                .into_registration(),
        )
        .build()
        .expect("released projector registration builds");

    runtime
        .emit(Observation::new(service(), ProjectedPayload))
        .expect("compat projector dispatches");
    runtime.flush().expect("routed log flushes");

    assert_eq!(projected.load(Ordering::SeqCst), 1);
    let routed_log = root
        .path()
        .join(sc_observability::constants::DEFAULT_LOG_DIR_NAME)
        .join(format!(
            "released-trait-contracts{}",
            sc_observability::constants::DEFAULT_LOG_FILE_SUFFIX
        ));
    let contents = fs::read_to_string(routed_log).expect("routed log is written");
    assert!(contents.contains("\"action\":\"released.projected\""));
    assert_eq!(
        runtime
            .health()
            .telemetry
            .expect("released provider is attached")
            .state,
        TelemetryHealthState::Disabled
    );

    runtime.shutdown().expect("routing runtime shuts down");
    telemetry.shutdown().expect("released telemetry shuts down");
}
