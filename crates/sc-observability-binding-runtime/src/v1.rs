//! Released 1.x binding adapters retained for the migration window.

/// Attaches bounded operations to an existing released bridge control.
///
/// # Errors
///
/// Returns initialization or unavailable native snapshot diagnostics.
#[deprecated(note = "use bridge_backend_v2 with sc_observability_log::v2::LogControl")]
#[allow(
    deprecated,
    reason = "the released signature retains the deprecated control type"
)]
pub fn bridge_backend(
    control: sc_observability_log::LogControl,
) -> Result<super::BridgeControlBackend, sc_observability_dto::Failure> {
    super::bridge_backend_v2(control.into_v2())
}
