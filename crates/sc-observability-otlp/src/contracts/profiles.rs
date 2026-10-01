//! Profile exporter capability staged for D34.
use sc_observability_types::v2::ExportError;
#[cfg_attr(
    not(feature = "durable-store"),
    expect(dead_code, reason = "used by the durable submission exporter")
)]
pub(crate) trait ProfileExporter<T>: Send + Sync {
    fn export_profiles(&self, batch: &[T]) -> Result<(), ExportError>;
}
