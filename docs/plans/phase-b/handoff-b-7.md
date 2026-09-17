---
id: B.7-publish-bindings-handoff
status: readiness_machinery_complete_publication_pending
branch: feature/phase-b-7-publish-bindings
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-7-publish-bindings
base: develop
generated_at: 2026-09-17T11:14:42Z
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
in the manifest; their versions are not checked by `verify-versions`. Both
files now exist in this branch's tree (merged forward from
`feature/phase-b-6-python-async` after this readiness work was first
written) and both are already pinned to `1.4.0` -- `bindings/tauri/Cargo.toml`
literally, `bindings/typescript/package.json` literally -- but neither is
`status = "ready"`: `sc-observability-tauri` still needs real qualification
on `feature/phase-b-tauri-qualification` (a separate layer being prepared
above B.7), and `bindings/typescript/package.json` still sets
`"private": true`.

## Source commit / tag

The **release** source commit/tag is still pending: this will be an immutable
`main` release commit/tag once B.7's `gate-and-tag` job runs for a real
release, and no tag has been cut for this readiness work.

A **candidate** source commit is no longer pending, though: `scripts/release_
bindings_artifacts.py build-evidence` records the exact `git rev-parse HEAD`
each candidate artifact was built from, and `verify-evidence` fails closed
(wrong-source rejection) if that recorded commit ever stops matching the
commit being verified. This is real, but it is a *candidate* proof (this
branch's current tip), not the release proof above -- do not conflate the two.

## Hashes

**Candidate hashes now exist and are real** -- this is a change from earlier
readiness work, which had none. `scripts/release_bindings_artifacts.py
build-evidence` actually builds every `status = "ready"` artifact (`cargo
package --locked --no-verify --exclude-lockfile` for the 3 ready crates;
`maturin sdist` + `maturin build` for the PyPI package) and records a sha256
of each output file plus a canonical content-fingerprint (`content_sha256`,
tolerant of archive-container timestamp jitter -- see
`content_fingerprint`'s docstring in the script) at
`release/evidence/bindings-candidate.json`. `verify-evidence` rebuilds each
artifact fresh and fails closed on wrong-source, changed-byte, or
missing-evidence-for-a-ready-artifact. `scripts/ci/
validate_binding_registry_consumers.sh` runs both end to end every time it
runs (see its "candidate evidence" section) -- this is not a one-off,
hand-run result.

What is still genuinely pending: **release-level** hashes tied to an
immutable, tagged `main` commit (the table in this sprint's own
`sprint-b-7-publish-bindings.md` "Release compatibility record"). Candidate
hashes prove the build/verify machinery and this branch's current bytes;
they are not a substitute for hashing the actual tagged release commit once
one exists, and `release/evidence/` is gitignored build output (rebuilt on
demand), not a committed release record.

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

`bindings/typescript/` and `bindings/tauri/` now exist in this branch's tree
(merged forward from `feature/phase-b-6-python-async`). `sc-observability-tauri`
compiles standalone (`cargo check --locked --no-default-features` inside
`bindings/tauri/`, verified during this readiness work) but has no IPC/artifact
platform matrix yet -- that is the explicit scope of `feature/phase-b-tauri-
qualification`, a separate layer being prepared above B.7. TypeScript has its
own generation/build/pack validation (`scripts/ci/validate_typescript_bindings.sh`,
`scripts/ci/validate_binding_schema.sh`, both merged in alongside the package)
which is a different concern from this document's registry-publish-readiness
scope; this readiness work does not re-run or duplicate those checks.

## Adoption examples

- **Rust embedding**: `examples/rust-python-logging/` already exercises a
  real Rust host embedding `sc-observability-py`'s native module and
  `sc-observability-binding-runtime`'s shared backend end-to-end (installs
  the PyO3 module, attaches the host logger, logs through the Python-facing
  API, and reads back backend health). It currently depends on
  `sc-observability-binding-runtime` and `sc-observability-py` via **path**
  dependencies, clearly appropriate today since neither crate is on
  crates.io yet -- flip to a version requirement once published.
  `scripts/ci/validate_binding_registry_consumers.sh` no longer reuses this
  example's live-source path dependencies as its registry-consumer proof
  (that was C03's rejected weak proxy). It now extracts the real `.crate`
  tarballs `build-evidence` produced, builds a throwaway consumer workspace
  whose `[dependencies]` point only at those extracted directories (never
  the live source tree), and `cargo check`s it -- proving the actual shipped
  file set (respecting each crate's `Cargo.toml` include/exclude) compiles
  as an external dependency. The 4 core crates those packaged, registry-
  normalized manifests still need (`sc-observability`,
  `sc-observability-types`, `sc-observability-log`,
  `sc-observability-log-macros` -- not yet published at this workspace
  version) are satisfied via a `[patch.crates-io]` table pointing at their
  real local source, scoped to that throwaway workspace only, for the same
  documented reason `examples/rust-python-logging/Cargo.toml` already gives
  for its own path overrides.
- **Python**: `bindings/python/sc-observability-py`'s existing package
  (`pyproject.toml`, `python/sc_observability/`) is the adoption surface;
  B.4a's installed-suite already exercises it against temporary logs across
  all 25 platform/interpreter cells. `scripts/ci/
  validate_binding_registry_consumers.sh` additionally installs the exact
  wheel `build-evidence` built into a fresh isolated venv and runs a
  representative subset of `bindings/python/sc-observability-py/tests/`
  against the installed package (not the live source tree).
- **Tauri**: `examples/tauri-logging/` now exists in this branch's tree
  (merged forward alongside `bindings/tauri/`); a registry-only consumer
  example for it is blocked on the same qualification gate as the crate
  itself and is not exercised by this readiness work.
- **TypeScript**: `bindings/typescript/src/test.ts` exercises the client
  today via `npm test`. `scripts/ci/validate_binding_registry_consumers.sh`
  additionally runs `npm pack` (this works even with `"private": true` --
  that flag only blocks `npm publish`) and installs the real resulting
  tarball into an isolated npm project, then runs a `node -e` smoke check
  against it. This is a genuine forward-looking structural proof of the
  packaged file set, but it is explicitly labeled as such and never as
  publish-readiness -- the manifest entry stays `pending` and no summary
  line claims otherwise.

## Sprint status (updated after aobs's C03/C04/C05 rejection and rework)

**aobs rejected the first pass of this readiness machinery as too weak**:
`validate_binding_registry_consumers.sh` succeeded on missing packages,
skipped pending entries, and only `cargo check`ed local path dependencies
(C03); `release.yml` tagged/published the 6 core crates before binding
readiness, silently ignored plan failures via a process-substitution
exit-code bug, and left PyPI upload literally unimplemented (C04); the
manifest declared `ready` from file existence alone, with no pinned source,
artifact hashes, schema, or matrix proof (C05). This section describes the
reworked state, not the original one.

- `release/bindings-artifacts.toml` exists, parses, and correctly records 4
  crates.io entries (3 `ready`, 1 `pending` with a named reason) and 2
  package entries (1 `ready`, 1 `pending` with a named reason). Every entry
  now also carries `registry_secret`/`registry_secret_configured`
  (CARGO_REGISTRY_TOKEN is real and configured; PYPI_API_TOKEN/NPM_TOKEN are
  not), and the pypi entry carries `platform_policy_ref` pointing at
  `release/python-platform-policy.json`.
- `scripts/release_bindings_artifacts.py` gained `build-evidence` and
  `verify-evidence` (C05: real cargo package/maturin builds, real sha256 +
  canonical content hashing, real wrong-source/changed-byte/missing-evidence
  fail-closed checks), a `check_platform_matrix` gate wired into
  `validate-manifest` (missing-platform rejection), and `--require-secrets`
  / `--only-kind` on `list-publish-plan` (unavailable-auth rejection, scoped
  per publish-job kind so an unrelated package's missing secret can't block
  a different kind's publish). `list-publish-plan`'s row format gained
  `workspace_member` and `manifest_path` columns so the release workflow can
  pick the correct `cargo publish` invocation per entry (C04).
- `scripts/ci/tests/test_release_bindings_artifacts.py` (29 tests, up from
  14) and the new `scripts/ci/tests/test_bindings_evidence.py` (6 tests)
  cover the original malformed/missing/duplicate/dependency-order/name/
  version negatives plus the new missing-platform, unavailable-auth,
  wrong-source, changed-byte, and missing-evidence-for-ready-artifact
  negatives -- against synthetic fixtures, not the real manifest.
- `scripts/ci/validate_binding_registry_consumers.sh` was rewritten (C03),
  keeping its filename. It now runs, every time, in order: (1) an offline
  structural preflight explicitly labeled as such, never as registry-
  consumer PASS; (2) `build-evidence` + `verify-evidence` against real
  freshly-built bytes; (3) a real isolated Rust consumer of the extracted
  `.crate` tarballs, a real isolated Python venv install + test subset of
  the built wheel, and a real isolated npm install + smoke check of the
  packed (still-private) TypeScript tarball; (4) an honest summary that
  never prints "registry consumer validation passed" unless
  `--live-registry-check` actually queried a live registry (it did not in
  this readiness work -- nothing has been published). Verified by actually
  running it end to end during this rework: exit 0, all sections passed.
- `.github/workflows/release.yml` (C04): fixed the process-substitution
  exit-code bug in both the pre-existing `publish` job and the new
  `publish-binding-crates` job (plan captured via `plan="$(...)"` then read
  via a here-string, not `done < <(...)`); added manifest-path-aware
  `cargo publish` invocation for non-workspace-member crates (needed for
  `bindings/tauri`, a standalone Cargo workspace); wired `--require-secrets`
  into `publish-binding-crates`/`publish-python-wheel`/`publish-npm-client`;
  wired `publish-python-wheel` to `build-evidence`/`verify-evidence` for
  real hash-verified upload bytes instead of a placeholder; and added all
  three binding jobs to the `release` job's `needs:` list. **Consequence,
  stated plainly**: today this means the `release` job (and therefore any
  GitHub Release, for the 6 core crates too) cannot run at all, because
  `publish-binding-crates`/`publish-python-wheel`/`publish-npm-client` are
  all designed to fail closed right now (tauri pending, npm pending,
  PyPI/npm auth unconfigured). This is intentional per aobs's explicit C04
  instruction and this sprint's own AC3 ("a staged package or release-ready
  PR/workflow is not closure") -- it is not a defect to be quietly reverted.

**Live publication and full 4-crate/npm coverage remain genuinely pending**,
blocked on:

(a) `feature/phase-b-tauri-qualification` (real Tauri artifact/IPC matrix
    work, a separate layer being prepared above B.7) landing on this stack
    before `sc-observability-tauri` can flip to `ready` -- the crate itself
    already exists in this branch's tree and compiles standalone;
(b) `bindings/typescript/package.json`'s `"private": true` being cleared
    upstream before npm publish becomes possible -- the package itself
    already exists in this branch's tree;
(c) npm (`NPM_TOKEN`) and PyPI (`PYPI_API_TOKEN`) registry credentials being
    provisioned (only `CARGO_REGISTRY_TOKEN` exists today);
(d) actual registry-name control verification -- today's preflight only
    proved absence of the 6 names on their registries, not ownership or
    reserved availability, and must be reverified at real release time.

This sprint is **not** closed by this readiness work alone. Per the sprint
doc's own AC3, missing publication access leaves B.7 pending; a staged
package or release-ready PR/workflow is not itself closure.

As of C04's rework, (a)-(c) above are no longer just documentation gaps --
they now literally block `.github/workflows/release.yml`'s `release` job
(and therefore every GitHub Release, including the unrelated 6 core crates)
from running at all, since `release` now needs
`publish-binding-crates`/`publish-python-wheel`/`publish-npm-client` and all
three fail closed today. Resolving (a)-(c) is required not only to publish
the binding artifacts, but to cut any release through this workflow again.
