---
id: B.1
status: proposed
branch: feature/phase-b-1-copy
base: develop
---

# B.1 — Copy the corrected generic BTIT crates

## Goal and entry gate

Copy the code, with only mechanical workspace adaptation. Entry requires the
BTIT review at `docs/plans/phase-a/review-a-5.md` to record resolved Blocking and
Important findings, accepted re-review, and the exact source commit. The
independent sc-observability design-review requirement in that record must be
satisfied. Record the actual commit; never substitute the known-unapproved
`f6f69dc`. Until that exists, this sprint is waiting for source readiness.

## Dependencies

- `must_follow`: BTIT fix/re-review handoff, because it determines the source
  behavior and exported signatures.
- B.2 `must_follow` B.1; no binding sprint or publication is part of B.1.

## Deliverables (authoritative)

1. Copy the complete reviewed `crates/sc-observability-log/`,
   `crates/sc-observability-log-macros/`, and CI-only
   `crates/sc-observability-log-consumer-check/` trees, including Rust source,
   docs, tests, UI fixtures, licenses, and macro-consumer evidence. Do not copy
   BTIT's transitional workspace root, Tauri application, UI, paths, or policy.
2. Wire root `Cargo.toml`/`Cargo.lock`, workspace lints, test dependencies, and
   CI so the copied packages build against this repo's public core. Both library
   crates temporarily retain `publish = false`; the consumer check stays private
   permanently. Preserve the exact bridge-to-macro version pin. Allowed source
   differences are package metadata, inherited dependency paths/versions, and
   paths in tests/docs needed for relocation. Do not alter runtime algorithms,
   public signatures, or diagnostic behavior.
3. Create `docs/plans/phase-b/import-provenance.json` and
   `scripts/ci/validate_log_import.py`. Record repository URL, accepted full SHA,
   source review path/verdict, every copied file's source Git blob ID, and each
   permitted adaptation with reason. The validator accepts `--source-repo PATH`,
   verifies the immutable source and complete file inventory, and rejects any
   unexplained addition, deletion, or content difference. Snapshot the accepted
   public signatures in the handoff rather than inventing replacement APIs.
4. Update the affected dependency/structural CI allowlists and normative
   dependency diagram only to recognize the companion crates. Resolve any
   conflict with TYP-030 by an explicit scoped exception for the copied bridge's
   public errors/health types, documented in requirements/API checklist; keep
   existing core ownership intact. Add `docs/api-approvals/phase-b-log-import.md`
   with Scope/Approval/Affected Artifacts and actual reviewer approval before
   merge. Store execution evidence in `docs/plans/phase-b/handoff-b-1.md`.

The [BTIT API handoff recommendations](btit-api-handoff.md) are coordination
input for the source API freeze. Record their disposition and the accepted public
signatures; they do not authorize API redesign in this copy sprint.

## Boundary samples

Destination manifest shape (versions inherit this workspace; do not hard-code
an old BTIT version):

```toml
[package]
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
repository.workspace = true
publish = false

[lints]
workspace = true
```

`sc-observability-log` depends on the macros through an exact `=V` version plus
workspace path; `V` equals both copied packages' workspace version. The macros
crate must not acquire a dependency back to the bridge. The consumer-check
package must depend directly only on the bridge, preserving the external
macro-expansion hygiene check.

## Acceptance criteria (authoritative)

- AC1: The provenance validator covers the entire accepted source tree and
  allows only the recorded mechanical changes. The copied runtime/public API
  matches the accepted BTIT baseline.
- AC2: All imported compatibility, formatter/panic, lifecycle, health, and UI
  tests pass on macOS/Linux/Windows, alongside existing workspace tests; no
  ignored failing fixtures or regeneration used to disguise behavioral changes.
- AC3: Existing core dependency/API invariants hold; neither core crate gains
  `log`, Tauri, Specta, PyO3, or any dependency on the copied crates. Every
  governance exception is scoped and approved; the three packages remain
  unpublished in this sprint.

## Required validation (authoritative)

From repo root; `BTIT_SOURCE_REPO` is the local checkout containing the accepted
commit, recorded in the handoff:

```sh
python3 scripts/ci/validate_log_import.py --source-repo "$BTIT_SOURCE_REPO"
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p sc-observability-log-consumer-check
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_docs_consistency.sh
```

The imported API freeze/consumer fixtures validate the new private packages;
existing published-core API/semver gates must also remain green in normal CI.
A pre-existing failure needs recorded disposition; it cannot be silently waived.

## Paths to delete

None. No files in BTIT are deleted or edited by this sprint.

## Non-closure

No publish, BTIT dependency switch, new runtime design, binding, or claim that
moving code itself closes unresolved source review findings. If mechanical
adaptation cannot preserve the accepted source contract, stop this copy scope
and return a concrete incompatibility finding before planning additional work.
