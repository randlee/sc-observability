---
id: B.1b-logger-prep
status: in_progress
branch: feature/phase-b-1b-logger-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1b-logger-prep
parent: fix/phase-b-1-provenance-integrity
authoritative-sprint-doc: sprint-b-1b-logger-errors.md
contract: error-api-contract.md
---

# B.1b logger preparation — typed logger operations and sink interoperability

This task plan was reconstructed at task start because it was absent from the
verified parent. The named sprint document and error API contract remain the
authoritative requirements.

## Scope

1. Add neutral `LogFailure` and `TryLogFailure` values with lossless conversion
   to the retained logger admission errors.
2. Add typed logger and builder construction, admission, and flush methods.
   The typed path is the common production implementation; legacy methods
   adapt its results while preserving `emit`'s conditional flush behavior.
3. Add `sc_observability::typed::TypedLogSink` and explicit adapters, then
   move built-in sink write/flush production paths to that one implementation.
4. Add focused old/new parity, adapter, source-preservation, construction, and
   queue/filter/owner fixtures; record commands and outcomes in the handoff.

## Boundaries

Do not alter existing public logger signatures, representations, bridge
contracts, control/lifecycle behavior, warning policy, provenance tooling, or
neutral-fixture tests owned by other layers. No B.1 copy, publication, or
warning activation is part of this task.

## Validation checklist

- `cargo fmt --all -- --check`
- `cargo test --locked -p sc-observability --all-targets`
- workspace doctests and clippy
- public API/docs review and bridge regression evidence
- two passes: contract/API completeness, then Rust behavior/source integrity

