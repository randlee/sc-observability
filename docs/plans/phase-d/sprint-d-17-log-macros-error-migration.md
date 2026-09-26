# d-17: Log macros error migration

## Plan metadata

- Wave: 15
- Branch: `sprint/d-17-log-macros-error-migration`
- PR target: `sprint/d-16-dto-error-migration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-log-macros/**`
  - ``

## Deliverables

1. Migrate the event, fields, and instrument macro diagnostics in sc-observability-log-macros to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-log-macros.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.
4. Update the sprint documentation for the migrated error surface.

## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.

## Design

## Migration recipe\n\n| Current wrapper or error | D.12 target | ErrorContext fields |\n| --- | --- | --- |\n| crate-local diagnostic wrapper | canonical non-exhaustive error enum | operation, source, diagnostic code |\n\n## Implementation targets\n\n- Each owned source module: replace local wrapper construction with the D.12 enum variant (deliverable 1).\n- Each owned test module: assert the typed variant and source context (deliverable 3).\n\n

## Acceptance criteria

- 
running 5 tests
test fields::tests::parses_event_level ... ok
test fields::tests::parses_brace_field_set ... ok
test fields::tests::rejects_unsupported_forms ... ok
test fields::tests::parses_prefixes_fields_and_message ... ok
test fields::tests::instrument_context_rejects_value_less_fields ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 7 tests
test crates/sc-observability-log-macros/src/lib.rs - debug (line 48) ... ignored
test crates/sc-observability-log-macros/src/lib.rs - error (line 81) ... ignored
test crates/sc-observability-log-macros/src/lib.rs - event (line 92) ... ignored
test crates/sc-observability-log-macros/src/lib.rs - info (line 59) ... ignored
test crates/sc-observability-log-macros/src/lib.rs - instrument (line 119) ... ignored
test crates/sc-observability-log-macros/src/lib.rs - trace (line 37) ... ignored
test crates/sc-observability-log-macros/src/lib.rs - warn (line 70) ... ignored

test result: ok. 0 passed; 0 failed; 7 ignored; 0 measured; 0 filtered out; finished in 0.00s

all doctests ran in 0.29s; merged doctests compilation took 0.09s passes with migrated typed-error assertions.\n-  returns zero remaining legacy constructions.\n- The crate's sprint doc and typed-error tests exist and name the D.12 enum.
