# B.1e migration-prep handoff

## Source and scope

- Branch: `feature/phase-b-1e-migration-prep`
- Parent: `feature/phase-b-1d-telemetry-prep`
- Parent merged before final preparation: `b76495c05be5e7d59a5c3ed8d383763c9de68aee`
- Authoritative sources: `sprint-b-1e-error-adoption.md` and
  `error-api-contract.md`
- Scope: exact migration guide, warning inventory and adoption-skill routing
- Explicitly out of scope: deprecation attributes, ordinary production
  migration, copied crates, workspace/CI changes, publication and removal

Scoped completeness PASS: aobs accepted C01/C02/C03 and the final
`build_typed` guidance at `a0f7c1f4186e4630b8cc7f70c5c3b63497b9344b`
(`01M2Q5Y03GC0JHXD57X0M613ND`). This closeout commit records only the scoped
documentation preparation; full B.1e warning activation and validator/fixture
work remain pending.

The task plan was missing at dispatch; `task-b-1e-migration-prep.md` now
records that mismatch and the scoped work. The final telemetry parent removed
the unplanned `TelemetryProjectors::with_typed_*` builders; the guide uses
explicit typed-module adapters with unchanged legacy registration methods.

## Evidence matrix

| Requirement | Evidence | Result |
| --- | --- | --- |
| Exact old/new symbols and signatures | `warning-inventory-b-1e.md`, `references/migrate-error-api.md` | Complete for merged B.1b–B.1d surface |
| Nine family mapping and fallback | Migration reference §Nine wrapper families; contract mapping | Complete |
| Diagnostic/source preservation | Migration reference §Nine wrapper families and §Custom traits | Complete; uses existing `context`/conversion contract |
| Explicit adapters | Migration reference §Custom traits and corrected projector example | Complete; no `with_typed_*` claim |
| Owner-method exemptions | Inventory §Explicit method exemptions | Complete |
| Warning/version policy | Inventory §Version and activation; reference §Prerequisite | Complete for prep; B.1e activation/validation remains pending, B.2 qualifies |
| Success and failure guide paths | Reference §Success and failure checks; current integration tests | Passed against current APIs |
| Requirements/API/release consistency | `requirements.md`, `api-design.md`, `migration-guide.md`, release docs updates | Complete for preparation status |
| Downstream validator and three fixtures | Authoritative B.1e deliverable 4 | Pending team-lead/CI follow-up; not claimed here |

## Validation commands

### B1E-C01/C02/C03 fix round

- C01 fixed: the docs now state that B.1e implementation selects, activates
  and validates warnings; this scoped preparation leaves activation pending,
  B.2 qualifies the B.1e result, and B.7 publishes it.
- C02 fixed: new migration guidance recommends `log_typed` and
  `try_log_typed`; the supported infallible `LoggerBuilder::build` and the
  additive fallible `build_typed` distinction is explicit. OTLP constructors
  retain their actual `impl Into<String>` signatures.
- C03 fixed: both `legacy_*` and `typed_*` adapter directions are listed,
  including `sc_observability::typed::{legacy_sink,typed_sink}`. The guide's
  complete kind-matching, success/failure and projector-adapter examples were
  compiled and run by the temporary downstream consumer below.
- C02 final fixed: the primary migration reference now states exactly that
  `LoggerBuilder::build` remains supported and infallible, while
  `build_typed` is the recommendation for recoverable startup errors.

The guide's current API paths were checked by:

```text
cargo run --manifest-path /tmp/b1e-guide-example/Cargo.toml
```

This temporary downstream consumer compiled both the documented successful
`OtlpEndpoint::new_typed` path and the invalid-input `InitFailureKind` match.

The workspace evidence was:

```text
cargo test -p sc-observability-otlp --test full_stack_integration typed_projector_inputs_forward_through_retained_registration
cargo test -p sc-observe --test typed_observation typed_and_legacy_construction_failures_classify_consistently
cargo test -p sc-observability-types --test neutral_contracts typed_to_legacy_adapters_preserve_errors_and_invoke_once
cargo test -p sc-observability-otlp otlp_endpoint_rejects_empty_or_scheme_less_values
cargo test -p sc-observability-types --test neutral_contracts
bash scripts/ci/validate_docs_consistency.sh
cargo fmt --all -- --check
cargo test --workspace
python3 scripts/ci/validate_public_api_semver.py
```

Final source/doc second pass reported 13 unique `_typed` declarations, nine
wrapper-warning rows plus 20 mapped method rows, and no
`TelemetryProjectors::with_typed_*` source symbols. Formatting, the full
workspace test/doctest suite, docs consistency, and semver validation passed.

The final handoff records the exact accepted source head and second-pass
inventory above. Full B.1e remains open until the downstream Cargo
fixtures, JSON warning validator, all-four-crate checks and independent QA
pass are delivered by their owners.

## Non-closure

This handoff does not claim deprecation rollout, a warning-free legacy build,
release publication, a removal schedule, a major release, or sprint-wide
completion. B.1e implementation/validation activates the warning path; this
scoped prep leaves that implementation pending, B.2 qualifies/stages its
result, and B.7 alone publishes it.
