//! Workspace parity test for the B.1a-B.1d checked error inventory.
//!
//! Every canonical error code baked into `sc_observability_types::typed`'s
//! nine `define_failure!` families is hard-coded as a string literal there
//! (neutral types must not depend on the runtime crates that actually own
//! each code). This test is the single place able to see all four crates'
//! `error_codes` registries at once and asserts each typed constructor's
//! produced diagnostic code equals its owning crate's registry constant, so
//! a renamed or removed registry constant is caught here instead of only by
//! runtime string-matching drift.

use sc_observability_types::Remediation;
use sc_observability_types::typed::{
    ClassifiedError, EventFailure, ExportFailure, FlushFailure, IdentityFailure, InitFailure,
    LogSinkFailure, ProjectionFailure, ShutdownFailure, SubscriberFailure,
};

fn remediation() -> Remediation {
    Remediation::not_recoverable("parity test remediation")
}

macro_rules! assert_owning_code {
    ($failure:expr, $owning_const:expr) => {
        assert_eq!(
            $failure.context().diagnostic().code,
            $owning_const,
            "typed constructor diverged from its owning crate's error_codes registry constant"
        );
    };
}

#[test]
fn identity_failure_matches_owning_registry() {
    assert_owning_code!(
        IdentityFailure::resolution_failed("x", remediation()),
        sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED
    );
}

#[test]
fn init_failure_matches_owning_registry() {
    assert_owning_code!(
        InitFailure::logger_initialization("x", remediation()),
        sc_observability::error_codes::LOGGER_INIT_FAILED
    );
    assert_owning_code!(
        InitFailure::observation_initialization("x", remediation()),
        sc_observe::error_codes::OBSERVABILITY_INIT_FAILED
    );
    assert_owning_code!(
        InitFailure::invalid_telemetry_config("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_INVALID_CONFIG
    );
    assert_owning_code!(
        InitFailure::invalid_protocol("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_INVALID_PROTOCOL
    );
    assert_owning_code!(
        InitFailure::exporter_initialization("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_EXPORTER_INIT_FAILED
    );
    assert_owning_code!(
        InitFailure::identity_resolution("x", remediation()),
        sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED
    );
}

#[test]
fn event_failure_matches_owning_registry() {
    assert_owning_code!(
        EventFailure::invalid_event("x", remediation()),
        sc_observability::error_codes::LOGGER_INVALID_EVENT
    );
    assert_owning_code!(
        EventFailure::closed("x", remediation()),
        sc_observability::error_codes::LOGGER_SHUTDOWN
    );
    assert_owning_code!(
        EventFailure::queue_full("x", remediation()),
        sc_observability::error_codes::LOGGER_QUEUE_FULL
    );
    assert_owning_code!(
        EventFailure::writer_degraded("x", remediation()),
        sc_observability::error_codes::LOGGER_WRITER_DEGRADED
    );
    assert_owning_code!(
        EventFailure::shutdown_timed_out("x", remediation()),
        sc_observability::error_codes::LOGGER_SHUTDOWN_TIMED_OUT
    );
    assert_owning_code!(
        EventFailure::span_assembly("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED
    );
}

#[test]
fn flush_failure_matches_owning_registry() {
    assert_owning_code!(
        FlushFailure::logger_flush("x", remediation()),
        sc_observability::error_codes::LOGGER_FLUSH_FAILED
    );
    assert_owning_code!(
        FlushFailure::writer_degraded("x", remediation()),
        sc_observability::error_codes::LOGGER_WRITER_DEGRADED
    );
    assert_owning_code!(
        FlushFailure::observation_flush("x", remediation()),
        sc_observe::error_codes::OBSERVABILITY_FLUSH_FAILED
    );
    assert_owning_code!(
        FlushFailure::telemetry_flush("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_FLUSH_FAILED
    );
    assert_owning_code!(
        FlushFailure::closed("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_SHUTDOWN
    );
}

#[test]
fn shutdown_failure_matches_owning_registry() {
    assert_owning_code!(
        ShutdownFailure::telemetry_flush("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_FLUSH_FAILED
    );
    assert_owning_code!(
        ShutdownFailure::incomplete_spans("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_INCOMPLETE_SPAN_DROPPED
    );
    assert_owning_code!(
        ShutdownFailure::writer_degraded("x", remediation()),
        sc_observability::error_codes::LOGGER_WRITER_DEGRADED
    );
    assert_owning_code!(
        ShutdownFailure::timed_out("x", remediation()),
        sc_observability::error_codes::LOGGER_SHUTDOWN_TIMED_OUT
    );
}

#[test]
fn projection_failure_matches_owning_registry() {
    assert_owning_code!(
        ProjectionFailure::telemetry_closed("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_SHUTDOWN
    );
    assert_owning_code!(
        ProjectionFailure::telemetry_export("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_EXPORT_FAILED
    );
    assert_owning_code!(
        ProjectionFailure::span_assembly("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED
    );
    assert_owning_code!(
        ProjectionFailure::routing("x", remediation()),
        sc_observe::error_codes::OBSERVATION_ROUTING_FAILURE
    );
}

#[test]
fn subscriber_failure_matches_owning_registry() {
    assert_owning_code!(
        SubscriberFailure::routing("x", remediation()),
        sc_observe::error_codes::OBSERVATION_ROUTING_FAILURE
    );
}

#[test]
fn log_sink_failure_matches_owning_registry() {
    assert_owning_code!(
        LogSinkFailure::write("x", remediation()),
        sc_observability::error_codes::LOGGER_SINK_WRITE_FAILED
    );
    assert_owning_code!(
        LogSinkFailure::maintenance("x", remediation()),
        sc_observability::error_codes::LOGGER_MAINTENANCE_FAILED
    );
    // `LogSinkFailure::fault_injected`'s owning code is gated behind
    // sc-observability's `fault-injection` feature, which this crate does
    // not enable (adding it as a dev-dependency-only feature would drift
    // scripts/ci/validate_dependency_bans.sh's and
    // validate_repo_boundaries.sh's allowed otlp dev-dependency baseline).
    // That case is covered instead by sc-observability's own
    // `fault_injected_failure_matches_owning_registry` test, which already
    // builds with the feature natively.
}

#[test]
fn export_failure_matches_owning_registry() {
    assert_owning_code!(
        ExportFailure::export("x", remediation()),
        sc_observability_otlp::error_codes::TELEMETRY_EXPORT_FAILED
    );
}
