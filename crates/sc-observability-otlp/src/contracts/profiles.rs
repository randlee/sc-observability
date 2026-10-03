//! Profile exporter capability staged for D34.
use sc_observability_types::v2::ExportError;
pub(crate) trait ProfileExporter<T>: Send + Sync {
    fn export_profiles(&self, batch: &[T]) -> Result<(), ExportError>;
}
