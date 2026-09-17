---
id: B.7-publish-bindings-handoff
status: readiness_machinery_complete_publication_pending
branch: feature/phase-b-7-publish-bindings
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-7-publish-bindings
base: develop
generated_at: 2026-09-17T10:25:54Z
---

# B.7 binding-release readiness handoff

This document records what B.7's release-readiness machinery proves *today*,
against this branch's actual (partial) tree state, and what remains genuinely
pending before phase-end live publication can happen. It is not a
publication record: nothing described here has been published to any
registry. See `release/bindings-artifacts.toml` for the machine-readable
source of truth this document summarizes.

## Registry URLs

None of the 6 target registry entries resolve yet -- nothing has been
published. Their eventual pages, once live:

| Kind | Artifact | Eventual URL |
|------|----------|---------------|
| crates.io | `sc-observability-dto` | https://crates.io/crates/sc-observability-dto |
| crates.io | `sc-observability-binding-runtime` | https://crates.io/crates/sc-observability-binding-runtime |
| crates.io | `sc-observability-tauri` | https://crates.io/crates/sc-observability-tauri |
| crates.io | `sc-observability-py` | https://crates.io/crates/sc-observability-py |
| PyPI | `sc-observability` | https://pypi.org/project/sc-observability/ |
| npm | `@sc-observability/client` | https://www.npmjs.com/package/@sc-observability/client |

Name-preflight evidence (`~/.config/atm/share/sc-obs/b7-evidence/name-preflight.json`,
checked `2026-09-17T09:34:53Z`) confirms all 6 names currently 404 on their
registries. Per that evidence's own scope note, absence proves nothing about
namespace control, publishing permission, or reserved availability -- it must
be reverified at actual release time, not treated as a permanent guarantee.

## Versions

Workspace version is `1.4.0` (`Cargo.toml`'s `[workspace.package].version`).
Every "ready" entry in `release/bindings-artifacts.toml` resolves to this
same version today, verified live (not hardcoded) by
`scripts/release_bindings_artifacts.py verify-versions`:

- `sc-observability-dto`: `version.workspace = true` -> `1.4.0`
- `sc-observability-binding-runtime`: `version.workspace = true` -> `1.4.0`
- `sc-observability-py`: `version.workspace = true` -> `1.4.0`
- `sc-observability` (PyPI, `bindings/python/sc-observability-py/pyproject.toml`):
  literal `version = "1.4.0"`

`sc-observability-tauri` and `@sc-observability/client` are `status = "pending"`
in the manifest; their versions are not checked (the files do not exist in
this branch's tree). The frozen reference copy at
`feature/phase-b-3a-typescript@202e4d0` shows both already pinned to `1.4.0`
in that lineage.

## Source commit / tag

Pending. This will be an immutable `main` release commit/tag once B.7's
`gate-and-tag` job runs for a real release; no tag has been cut for this
readiness work.

## Hashes

N/A pending build. No crate archive, sdist, wheel, or npm tarball has been
produced from a release commit yet, so there are no hashes to record.

## Public API coverage

Existing approval records under `docs/api-approvals/` (note: these are
`.md` files with required `## Scope` / `## Approval` / `## Affected Artifacts`
headings, per `docs/api-approvals/README.md` -- there are no `.json` files in
that directory on this branch):

- `docs/api-approvals/phase-a-a3-writer-runtime.md`
- `docs/api-approvals/phase-b-log-import.md`
- `docs/api-approvals/phase-b-runtime-level.md` (owner-deferred; not a
  current approval)

None of these three specifically approves `sc-observability-dto` or
`sc-observability-binding-runtime`'s public surface -- no dedicated approval
record for either crate exists on this branch yet. That is a gap this
readiness task surfaces rather than papers over: a human/API reviewer should
confirm whether these two crates need their own approval artifact before
first publish, consistent with the same owner-deferral pattern
`phase-b-runtime-level.md` already records for the runtime-level contract.
`sc-observability-tauri`, `sc-observability-py`'s API, the PyPI package, and
the npm client have no approval coverage recorded on this branch; the latter
two are additionally blocked on the files not existing yet.

## Platform results

The B.4a Python wheel matrix (`.github/workflows/b4a-python-distributions.yml`,
policy at `release/python-platform-policy.json`) already builds and qualifies
the full 5-platform x 5-interpreter (25-cell) matrix for the `sc-observability`
PyPI package's sdist/wheels. That pipeline is reused (not reinvented) by the
new `publish-python-wheel` job added to `.github/workflows/release.yml`.

TypeScript/Tauri platform-matrix evidence is pending: neither
`bindings/typescript/` nor `bindings/tauri/` exists in this branch's tree
yet, so there is no platform matrix to report for either.

## Adoption examples

- **Rust embedding**: `examples/rust-python-logging/` already exercises a
  real Rust host embedding `sc-observability-py`'s native module and
  `sc-observability-binding-runtime`'s shared backend end-to-end (installs
  the PyO3 module, attaches the host logger, logs through the Python-facing
  API, and reads back backend health). It currently depends on
  `sc-observability-binding-runtime` and `sc-observability-py` via **path**
  dependencies, clearly appropriate today since neither crate is on
  crates.io yet -- flip to a version requirement once published.
  `scripts/ci/validate_binding_registry_consumers.sh` reuses this example
  (via `cargo check -p rust-python-logging`) as its registry-shaped Rust
  embedding consumer proxy rather than fabricating new example code.
- **Python**: `bindings/python/sc-observability-py`'s existing package
  (`pyproject.toml`, `python/sc_observability/`) is the adoption surface;
  B.4a's installed-suite already exercises it against temporary logs across
  all 25 platform/interpreter cells.
- **TypeScript / Tauri**: pending. Both `bindings/typescript/` (the
  `@sc-observability/client` npm package) and `bindings/tauri/` (the
  `sc-observability-tauri` crate) exist only on `feature/phase-b-3a-typescript`
  at `202e4d0`, not yet merged forward onto this stack. No adoption example
  can be exercised here until that lands.

## Sprint status

**Readiness machinery is complete and passing for what exists in this
branch's tree today.** Specifically:

- `release/bindings-artifacts.toml` exists, parses, and correctly records 4
  crates.io entries (3 `ready`, 1 `pending` with a named reason) and 2
  package entries (1 `ready`, 1 `pending` with a named reason).
- `scripts/release_bindings_artifacts.py` (`validate-manifest`,
  `list-publish-plan`, `verify-versions`) runs clean against the real
  manifest and correctly refuses (`list-publish-plan --require-ready`) to
  produce a publish plan while any artifact is pending -- there is no
  three-crate-only acceptance path.
- `scripts/ci/validate_publish_order.sh` now runs both the existing 6-crate
  order check and the new bindings-manifest order/dependency check.
- `scripts/ci/validate_binding_registry_consumers.sh` correctly reports the
  2 pending entries as skipped-with-reason (not failures), reports the 4
  ready entries' structural state, and exercises the real Rust embedding
  consumer via `cargo check -p rust-python-logging`.
- `scripts/ci/tests/test_release_bindings_artifacts.py` (14 tests) proves the
  malformed/missing/duplicate/dependency-order/name/version negative paths
  against synthetic fixtures, not the real manifest.
- `.github/workflows/release.yml` gained `publish-binding-crates`,
  `publish-python-wheel`, and `publish-npm-client` jobs after the existing
  `publish` job, without modifying `gate-and-tag`, `publish`, or `release`.
  `publish-binding-crates` is intentionally left wired into the workflow
  (not gated out) and is expected to fail closed today because
  `sc-observability-tauri` is `pending`.

**Live publication and full 4-crate/npm coverage remain genuinely pending**,
blocked on:

(a) the `fix/phase-b-3a-completeness` parent-checkpoint merge landing on this
    stack (brings `bindings/tauri/` and `bindings/typescript/` into the tree,
    and requires flipping `bindings/typescript/package.json`'s
    `"private": true` before npm publish becomes possible even after that
    merge);
(b) npm (`NPM_TOKEN`) and PyPI (`PYPI_API_TOKEN`) registry credentials being
    provisioned (only `CARGO_REGISTRY_TOKEN` exists today);
(c) actual registry-name control verification -- today's preflight only
    proved absence of the 6 names on their registries, not ownership or
    reserved availability, and must be reverified at real release time.

This sprint is **not** closed by this readiness work alone. Per the sprint
doc's own AC3, missing publication access leaves B.7 pending; a staged
package or release-ready PR/workflow is not itself closure.
