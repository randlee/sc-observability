//! Compile-only proof that the event macros and `#[instrument]` need only the
//! `sc-observability-log` dependency: this package declares no other
//! `[dependencies]`, so any expansion path outside `::sc_observability_log`,
//! `::core` or `::std` fails to resolve.

use sc_observability_log::{Level, debug, error, event, info, instrument, trace, warn};

#[allow(
    unused_imports,
    reason = "compile-only proof for the Phase F canonical macro migration paths"
)]
use sc_observability_log::v2::{
    debug as v2_debug, error as v2_error, event as v2_event, info as v2_info,
    instrument as v2_instrument, trace as v2_trace, warn as v2_warn,
};

#[allow(
    dead_code,
    reason = "compile-only proof for the Phase F canonical type migration paths"
)]
fn phase_f_log_v2_paths_resolve(
    report: &sc_observability_log::v2::BridgeHealthReport,
) -> (
    sc_observability_log::v2::HelperHealth,
    sc_observability_log::v2::BridgeOptions,
    sc_observability_log::v2::LogAttachment,
    sc_observability_log::v2::LogControl,
    sc_observability_log::v2::Level,
    sc_observability_log::v2::LevelFilter,
    sc_observability_log::v2::LoggerConfig,
    sc_observability_log::v2::ServiceName,
) {
    let _ = sc_observability_log::v2::init;
    let _ = report.helpers;
    unreachable!()
}

include!("../../sc-observability-log/tests/compat/events.rs");
include!("../../sc-observability-log/tests/compat/instrument.rs");

/// Consumer fixture for the bridge control surface (review finding R-A4-005).
///
/// A status provider as a non-owning consumer writes it: it receives only a
/// [`LogControl`](sc_observability_log::v2::LogControl), never the `LogGuard`, and
/// reaches bounded flush and health through public items of
/// `sc-observability-log` alone (no `__private`, no `sc_observability::Logger`,
/// no other dependency). `tests/control_consumer.rs` runs it against an
/// installed bridge.
pub mod status {
    use std::time::Duration;

    use sc_observability_log::v2::{FlushError, LogControl};
    use sc_observability_log::{BridgeHealthReport, ControlError};

    /// One status reading: a bounded flush result and the health report, both plain data.
    #[derive(Debug, Clone)]
    pub struct StatusReading {
        /// `Ok` when the flush completed; otherwise its native typed error.
        pub flush: Result<(), FlushError>,
        /// The health result taken after the flush.
        pub health: Result<BridgeHealthReport, ControlError>,
    }

    /// Flushes through `control` (bounded by `timeout`) and reads health.
    #[must_use]
    pub fn read_status(control: &LogControl, timeout: Duration) -> StatusReading {
        StatusReading {
            flush: control.flush(timeout),
            health: control.health(),
        }
    }
}
