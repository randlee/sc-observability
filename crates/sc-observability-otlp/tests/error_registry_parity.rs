//! Workspace parity test for the B.1a-B.1d checked error inventory.
//!
//! Every canonical error code baked into `sc_observability_types::typed`'s
//! nine `define_failure!` families is hard-coded as a string literal there
//! (neutral types must not depend on the runtime crates that actually own
//! each code). This test is the single place able to see all four crates'
//! `error_codes` registries at once and asserts each typed constructor's
//! produced diagnostic code equals its owning crate's registry constant, so
//! a renamed registry constant is caught here instead of only by runtime
//! string-matching drift. Compatibility constructors whose legacy literals
//! remain types-owned are intentionally not asserted against this transport
//! registry: D.21 exposes only the types-owned OTLP registry.

use sc_observability_types::Remediation;
use sc_observability_types::typed::{
    ClassifiedError, EventFailure, FlushFailure, IdentityFailure, InitFailure, LogSinkFailure,
    ProjectionFailure, ShutdownFailure, SubscriberFailure,
};
use sc_observability_types::v2::{
    AggregationTemporality, FiniteF64, HistogramPoint, MetricModelError, MetricRecord, MetricValue,
};
use sc_observability_types::{MetricName, ServiceName, Timestamp};

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
}

#[test]
fn shutdown_failure_matches_owning_registry() {
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

fn assert_metric_model_failure(
    error: MetricModelError,
    expected: sc_observability_types::ErrorCode,
) {
    assert_eq!(error.diagnostic().code, expected);
    let context = std::error::Error::source(&error).expect("preserved error context source");
    assert!(
        std::error::Error::source(context).is_none(),
        "the stable model error retains exactly its original context"
    );
}

fn metric(value: MetricValue, timestamp: Timestamp) -> Result<MetricRecord, MetricModelError> {
    MetricRecord::try_new(
        timestamp,
        ServiceName::new("test-service").expect("valid service"),
        MetricName::new("test.metric").expect("valid metric"),
        value,
    )
}

fn one_second_after_epoch() -> Timestamp {
    serde_json::from_str("\"1970-01-01T00:00:01Z\"").expect("valid timestamp")
}

#[test]
fn metric_model_failures_match_types_owned_registry_and_preserve_source() {
    let invalid_histogram = HistogramPoint::try_new(
        vec![1.0],
        vec![1],
        1,
        FiniteF64::new(1.0).expect("finite value"),
    )
    .expect_err("one bound requires two buckets");
    assert_metric_model_failure(
        invalid_histogram,
        sc_observability_types::error_codes::SC_METRIC_INVALID_HISTOGRAM,
    );

    let invalid_temporality = metric(
        MetricValue::Sum {
            value: FiniteF64::new(1.0).expect("finite sum"),
            monotonic: true,
            temporality: AggregationTemporality::Delta,
            start_time: Timestamp::UNIX_EPOCH,
        },
        Timestamp::UNIX_EPOCH,
    )
    .expect_err("delta requires a nonempty interval");
    assert_metric_model_failure(
        invalid_temporality,
        sc_observability_types::error_codes::SC_METRIC_INVALID_TEMPORALITY,
    );

    let invalid_interval = metric(
        MetricValue::Sum {
            value: FiniteF64::new(1.0).expect("finite sum"),
            monotonic: false,
            temporality: AggregationTemporality::Cumulative,
            start_time: one_second_after_epoch(),
        },
        Timestamp::UNIX_EPOCH,
    )
    .expect_err("start cannot follow the point timestamp");
    assert_metric_model_failure(
        invalid_interval,
        sc_observability_types::error_codes::SC_METRIC_INVALID_INTERVAL,
    );
}
