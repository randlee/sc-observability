//! Profile exporter capability staged for D34.
use sc_observability_types::v2::ExportError;
#[allow(
    dead_code,
    reason = "staged by d-29; wired by d-33/d-34 under durable-store"
)]
pub(crate) trait ProfileExporter<T>: Send + Sync {
    fn export_profiles(&self, batch: &[T]) -> Result<(), ExportError>;
}
