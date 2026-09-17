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

## B.1e implementation handoff

The implementation correction pass is on
`feature/phase-b-1e-migration-validation`, based on and merged with the active
QA1 parent `origin/fix/phase-b-1ab-qa1` at `4299e25b7506a6e1d0852a2f3784d954a796329f`
before final validation. The initial correction source/fixture commit is
`331a0db`; the item/span and adapter-failure correction is `dc31ae7`. The
integrated parent merge head at this pass is `f79c4eb`, and the current
validated child head is `dc31ae7`. The child
activates all nine wrapper warnings and 20 mapped method warnings at
`since = "1.4.0"`, while retaining the three supported method exemptions and
`Logger::emit` at its existing `since = "1.2.0"`. Ordinary observation routing
uses `log_typed` and `flush_typed`; mixed production and compatibility modules
use named, reason-bearing allowances. The copied `sc-observability-log` bridge
keeps its public signatures unchanged and has only boundary/item allowances
for its retained legacy identity, logger, lifecycle and sink paths; those
allowances are recorded here as compatibility evidence, not as provenance
rewrites.

The M01–M05 correction evidence is:

- M01: the validator binds the actual local attribute block (including exact
  `since = "1.4.0"`) to each of exactly 29 targets, then binds each Cargo
  diagnostic to its expected deprecated item and exact `src/main.rs` fixture
  span. It requires one `deprecated` code and span, rejects secondary spans,
  unexpected notes/warnings, compares the complete compiler migration note
  rather than a substring, and compares the exact expected span multiset
  rather than only an aggregate note count. Real predicate controls cover
  misplaced attributes, wrong versions/notes, appended notes, duplicate
  allowed-line diagnostics, missing/extra diagnostics, wrong spans and broad
  allowances.
- M02: the legacy fixture exercises all nine wrapper names, every mapped
  method, explicit `InitError` tuple/field access, and the exempt owner
  constructors; the deny-deprecated fixture exercises both owner constructors
  and both typed counterparts without a blanket allowance, while the validator
  separates wrapper diagnostics from method diagnostics.
- M03: the migrated fixture runs root-glob imports, all five legacy/typed
  adapter directions, both sink directions, mixed subscriber/projector
  registration, custom and wrong-family failures through both projector
  adapter directions with one-call invocation assertions, retained
  code/kind/context/source checks, and the legacy fixture checks the serialized
  `InitError` golden and span path.
- M04: ordinary production projection/lifecycle paths call typed APIs; copied
  bridge signatures remain unchanged and its compatibility allowances are
  narrow, named and reason-bearing. The historical
  `import-provenance.json` remains unchanged; the separate
  `post-import-adaptations.json` record pins the three copied bridge files'
  before/after blobs and exact allowance blocks. The importer verifies that
  removing only those blocks reconstructs the accepted BTIT source byte for
  byte, rejecting undeclared, body or signature changes.
- M05: the local implementation checklist and this handoff record the second
  verification pass; B.2 qualification and B.7 publication remain pending.

The implementation evidence is:

```text
python3 scripts/ci/validate_error_migration.py
B.1e migration validation: PASS (source contract, JSON diagnostics, and all fixtures)
python3 -m unittest discover -s scripts/ci/tests -p 'test_validate_log_import.py'
Ran 51 tests, OK (including post-import warning-adaptation acceptance and
body/signature/undeclared-change rejection)
python3 scripts/ci/validate_log_import.py --source-repo /Users/randlee/github/beads-task-issue-tracker --post-import-adaptations docs/plans/phase-b/post-import-adaptations.json
B.1 import provenance and post-import warning-only adaptations are coherent
cargo check --workspace --message-format=short
PASS; remaining warnings are confined to the copied sc-observability-log bridge
cargo fmt --all -- --check
PASS
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
PASS
cargo test --workspace
PASS; all workspace tests and doctests passed
```

The validator runs standalone `legacy`, `migrated` and `partial` Cargo
workspaces, parses every target warning as JSON, executes each fixture, checks
the legacy serialized `InitError` golden, verifies typed kind fallback and
source-chain handling, exercises the full adapter matrix, and rejects broad
fixture allowances. Every fixture clean/check/run invocation uses `--locked`.
B.2 still owns qualification/staging and B.7 owns
publication; no removal schedule or major release is introduced.
