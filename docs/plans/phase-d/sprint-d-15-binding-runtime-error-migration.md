# d-15: Binding runtime error migration

Generated projection of `obs-d-15`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 13
- Assignee / model: lobs / luna
- Relation: `parallel_safe`
- Closure: `boundary`
- Target boundary: sc-observability-binding-runtime
- Branch: `sprint/d-15-binding-runtime-error-migration`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-15-binding-runtime-error-migration`
- PR target (merge order only): `sprint/d-14-observe-error-migration`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-002, LAY-003, LAY-006, LAY-007, LOG-001, LOG-003, LOG-004, LOG-007, LOG-010, LOG-014, LOG-015, LOG-016, LOG-017, LOG-018, LOG-019, LOG-023, LOG-037, LOG-038, LOG-047, LOG-048, NFR-001, NFR-002, NFR-005, NFR-006, NFR-007, NFR-009, NFR-012, PHB-002, PHB-003, PHB-004, PHB-005, PHB-006, PHB-007, PHB-008, PHB-009, PHB-010, PHB-011, PHB-012, PHB-013, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-011, ADR-012, ADR-013, ADR-014, ADR-015, ADR-017
- Owned paths (metadata projection):
  - `crates/sc-observability-binding-runtime/src/**`
  - `docs/plans/phase-d/sprint-d-15-binding-runtime-error-migration.md`

## Deliverables

1. Migrate the Callback, conversion, coordinator, operation, spawn, sync, and timer errors in sc-observability-binding-runtime to the D.12 non-exhaustive error enums carrying Box<ErrorContext>.
2. Remove local legacy error wrappers and update every call site within sc-observability-binding-runtime.
3. Update the crate tests to assert typed variants, stable diagnostics, and preserved source context.


## This Sprint Does Not Close

Workspace-wide wrapper removal, public re-exports, and release/API approval are closed by D.18.
Update the sprint doc as explanatory evidence alongside code; documentation is not a separate closure gate.

## Design

## Seven-family migration recipe

Consume obs-d-12 cause mapping/ErrorContext (ADR-017), preserving the binding runtime's existing operation/DTO semantics (PHB-002/010–013, ADR-014/015). This is a cause mapping, not a blanket conversion of every local runtime error to a sink failure.

| Family | Mapping/action | Fixture |
| --- | --- | --- |
| Callback | subscriber callback -> SubscriberError::Subscriber; sink callback -> LogSinkError::Write/Flush by operation; preserve foreign failure details | callback failure/source tests |
| conversion | invalid event -> EventError::Validation; invalid initialization input -> InitError::Configuration; preserve DTO tagged outcome | conversion.rs tests |
| coordinator | flush -> FlushError::Drain; shutdown deadline -> ShutdownError::Timeout; other shutdown failure -> ShutdownError::Drain | coordinator terminal-result tests |
| operation | preserve operation identity/receipt distinction and exact underlying canonical source; no error-to-success conversion | operation result tests |
| spawn | native startup failure -> InitError::Runtime; retain source/remediation | worker spawn failure tests |
| sync | preserve local synchronization state error and its cause; map only at the existing operation boundary to the relevant flush/shutdown error | concurrent completion tests |
| timer | deadline -> operation-specific FlushError::Drain or ShutdownError::Timeout; observer cancellation never cancels native work | timeout/cancellation tests |

All seven retain their local structural error families where not among the nine replaced wrappers; remove only duplicate legacy shared wrappers/classification. Migrate neutral model consumers and DTO conversion calls in this crate without changing schema ownership. Context keys live in bounded Diagnostic.details. No blanket TryLogFailure -> sink write conversion: preserve invalid-event/queue-full/shutdown distinctions.

## Replace vs coexist

The final ADR-017 surface replaces the nine wrappers; it does not permanently coexist with them. During this boundary wave use obs-d-12's v2 definitions and retain the minimal existing legacy-facing conversion needed for unfinished sibling consumers to compile and pass tests. No new legacy feature or duplicate classifier is introduced. D.18 receives the explicit handoff and removes all transitional wrappers/adapters when activating canonical exports. References above to removal/replacement apply to this bead's migrated v2 implementation; old public entry-point compatibility is retired only by D.18. Every boundary close still has a green all-features workspace check and workspace tests.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-15, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-binding-runtime/src/conversion.rs`
- `crates/sc-observability-binding-runtime/src/lib.rs`
- `crates/sc-observability-binding-runtime/src/tests.rs`

## Acceptance criteria

- [ ] `cargo test -p sc-observability-binding-runtime --locked` executes all seven family fixtures named in the recipe and checks code, remediation, typed source and tagged DTO outcome (D1/D3).
- [ ] The owned src files have no obsolete shared wrapper construction/classification; native ownership, retained shutdown results and observer timeout semantics remain unchanged (D2).
- [ ] This sprint does not close language wrapper/schema generation or release/API proof; D.18 does.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
