# d-14: Observe error migration

## Plan metadata

- Wave: 12
- Branch: `sprint/d-14-observe-error-migration`
- PR target: `sprint/d-8-otlp-http-json-transplant`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observe/**`
  - `crates/sc-observe/tests/**`

## Deliverables

1. Migrate the ObserveError and error wrappers in sc-observe to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observe.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe\n\n| Current wrapper or error | D.12 target | ErrorContext fields |\n| --- | --- | --- |\n| crate-local diagnostic wrapper | canonical non-exhaustive error enum | operation, source, diagnostic code |\n\n## Implementation targets\n\n- Each owned source module: replace local wrapper construction with the D.12 enum variant (deliverable 1).\n- Each owned test module: assert the typed variant and source context (deliverable 3).\n\n

## Acceptance criteria

- 
running 16 tests
test tests::typed_builder_reports_empty_routes_and_logger_startup_failures ... ok
test tests::queue_capacity_override_propagates_to_logger_config ... ok
test tests::top_level_health_exposes_attached_telemetry_provider ... ok
test tests::routing_failure_occurs_when_no_eligible_path_remains ... ok
test tests::filter_acceptance_and_rejection_are_respected ... ok
test tests::routing_failure_occurs_when_all_projectors_fail ... ok
test tests::registration_order_routing_is_deterministic ... ok
test tests::concurrent_typed_shutdown_is_idempotent ... ok
test tests::subscriber_failures_are_isolated ... ok
test tests::top_level_health_aggregates_logging_and_routing_state ... ok
test tests::post_shutdown_emission_returns_shutdown_error ... ok
test tests::typed_builder_uses_real_adapters_and_preserves_lifecycle_contract ... ok
test tests::flush_forwards_logger_flush_behavior_directly ... ok
test tests::projector_failures_are_isolated ... ok
test tests::shutdown_unwind_releases_condition_waiters ... ok
test tests::in_flight_shutdown_preserves_flush_health_and_repeated_shutdown ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s


running 1 test
test one_observation_can_fan_out_to_subscribers_logs_spans_and_metrics ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 10 tests
test observation_adapter_preserves_custom_and_cross_family_context ... ok
test invalid_deserialized_names_fail_paired_facade_checks ... ok
test typed_and_legacy_construction_failures_classify_consistently ... ok
test metric_adapters_retain_custom_and_wrong_family_context ... ok
test span_adapters_retain_custom_and_wrong_family_context ... ok
test log_adapters_retain_custom_and_wrong_family_context ... ok
test paired_filters_ordering_and_invocation_counts_match ... ok
test typed_routes_execute_real_subscriber_and_projector_adapters ... ok
test paired_no_matching_route_and_failure_outcomes_are_classified ... ok
test paired_projection_routes_preserve_output_family_invocation_counts ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 2 tests
test crates/sc-observe/src/lib.rs - ObservabilityConfig::default_for (line 81) ... ok
test crates/sc-observe/src/lib.rs - Observability::builder (line 290) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

all doctests ran in 0.41s; merged doctests compilation took 0.12s passes with migrated typed-error assertions.\n-  returns zero remaining legacy constructions.\n- The crate's sprint doc and typed-error tests exist and name the D.12 enum.
