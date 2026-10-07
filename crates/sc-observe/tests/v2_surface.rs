use std::path::PathBuf;
use std::time::Duration;

use sc_observe::v2::{
    FlushError, InitError, Observability, ObservabilityConfig, ObservabilityHealthProvider,
    Observable, Observation, ObservationError, ProjectionRegistration, ServiceName, ShutdownError,
    SubscriberRegistration, ToolName,
};

fn v2_only_imports<T: Observable>(
    runtime: &Observability,
    observation: Observation<T>,
) -> Result<(), ObservationError> {
    let _ = std::any::TypeId::of::<ProjectionRegistration<T>>();
    let _ = std::any::TypeId::of::<SubscriberRegistration<T>>();
    let _ = std::any::TypeId::of::<ServiceName>();
    let _ = std::any::TypeId::of::<dyn ObservabilityHealthProvider>();
    runtime.emit(observation)
}

#[test]
fn v2_observability_exposes_timeout_signatures_without_v1_imports() {
    let _: fn(ToolName, PathBuf) -> Result<ObservabilityConfig, InitError> =
        ObservabilityConfig::default_for;
    let _: for<'a> fn(&'a Observability) -> Result<(), FlushError> = Observability::flush;
    let _: for<'a> fn(&'a Observability, Duration) -> Result<(), FlushError> =
        Observability::flush_with_timeout;
    let _: for<'a> fn(&'a Observability) -> Result<(), ShutdownError> = Observability::shutdown;
    let _: for<'a> fn(&'a Observability, Duration) -> Result<(), ShutdownError> =
        Observability::shutdown_with_timeout;
    let _ = v2_only_imports::<()>
        as fn(&Observability, Observation<()>) -> Result<(), ObservationError>;
}
