# d-16: DTO error migration

## Plan metadata

- Wave: 14
- Branch: `sprint/d-16-dto-error-migration`
- PR target: `sprint/d-15-binding-runtime-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-dto/**`
  - ``

## Deliverables

1. Migrate the DTO conversion, wire, and error-code wrappers in sc-observability-dto to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-dto.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe\n\n| Current wrapper or error | D.12 target | ErrorContext fields |\n| --- | --- | --- |\n| crate-local diagnostic wrapper | canonical non-exhaustive error enum | operation, source, diagnostic code |\n\n## Implementation targets\n\n- Each owned source module: replace local wrapper construction with the D.12 enum variant (deliverable 1).\n- Each owned test module: assert the typed variant and source context (deliverable 3).\n\n

## Acceptance criteria

- 
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 18 tests
test diagnostics_preserve_unknown_codes_order_and_original_time ... ok
test checked_event_preserves_integer_and_host_stamp ... ok
test decimal_domains_are_canonical ... ok
test registry_has_unique_literals_and_exact_remediation ... ok
test protected_key_policy_is_shared_and_exact ... ok
test paths_keep_absence_and_non_unicode ... ok
test timeouts_are_bounded_integers ... ok
test level_change_errors_and_unsuccessful_diagnostics_preserve_payloads ... ok
test malformed_unknown_and_oversized_remote_errors_differ ... ok
test diagnostic_string_and_step_bounds_are_exact_and_never_truncate ... ok
test inputs_reject_missing_unknown_and_invalid_versions ... ok
test query_defaults_and_inclusive_bounds ... ok
test all_stored_event_fields_and_trusted_output_survive ... ok
test nested_output_additions_do_not_weaken_strict_inputs ... ok
test nonfinite_typed_values_cannot_bypass_input_checks ... ok
test complete_health_projection_and_unsigned_wire_counters ... ok
test spoofed_provenance_is_rejected_at_all_depths ... ok
test exact_request_size_and_depth_boundaries ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s passes with migrated typed-error assertions.\n-  returns zero remaining legacy constructions.\n- The crate's sprint doc and typed-error tests exist and name the D.12 enum.
