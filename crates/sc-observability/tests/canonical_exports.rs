//! Compile-only public-signature proof for the opt-in core facade.

use std::path::PathBuf;

use sc_observability::v2::{InitError, Logger, LoggerBuilder, LoggerConfig};
use sc_observability_types::ServiceName;

fn requires_send_sync<T: Send + Sync>() {}

#[test]
fn canonical_core_exports_have_real_public_signatures() {
    let _: fn(ServiceName, PathBuf) -> LoggerConfig = LoggerConfig::default_for;
    let _: fn(LoggerConfig) -> Result<LoggerBuilder, InitError> = LoggerBuilder::new;
    let _: fn(LoggerBuilder) -> Result<Logger, InitError> = LoggerBuilder::build_canonical;
    let _: for<'a> fn(&'a Logger) -> &'a ServiceName = Logger::service_name;

    requires_send_sync::<Logger>();
}
