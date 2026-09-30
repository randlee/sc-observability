//! Compile-only public-signature proof for the opt-in core facade.

use std::path::PathBuf;

use sc_observability::v2::{InitError, Logger, LoggerBuilder, LoggerConfig};
use sc_observability_types::ServiceName;

#[test]
fn canonical_core_exports_have_real_public_signatures() {
    let _config: fn(ServiceName, PathBuf) -> LoggerConfig = LoggerConfig::default_for;
    let _new: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = LoggerBuilder::new;
    let _build: fn(LoggerBuilder) -> Result<Logger, InitError> = LoggerBuilder::build;
    let _service_name: for<'a> fn(&'a Logger) -> &'a ServiceName = Logger::service_name;

    fn requires_send_sync<T: Send + Sync>() {}
    requires_send_sync::<Logger>();
}
