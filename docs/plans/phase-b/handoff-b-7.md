---
id: B.7-publish-bindings-handoff
status: review_packet_prepared_publication_pending_owner_review
branch: feature/phase-b-7-publish-bindings
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-7-publish-bindings
base: develop
generated_at: 2026-09-18T16:21:23Z
---

# B.7 binding-release review packet (readiness handoff)

**Owner sequencing correction (via aobs): no mid-phase publication. B.P2/B.2
are reviewed immutable release candidates, B.3-B.6 (and this branch's
bindings) consume prepublication bundles, and B.7 is the sole phase-end
publication step for all of Phase B (core, bridge/macros, and bindings) --
but only after explicit owner review authorizes it.** Until that review
happens, no publication workflow is dispatched and no registry credentials
are sought, from this branch or any other. Installing or upgrading the
intended shared publishing pipeline, **`sc-publish`**, is a separate
follow-up outside Phase B; it is not installed on this branch or in
`.github/workflows/release.yml`. This is a sequencing/authorization
constraint, not a permanent scope removal: B.7 remains the eventual
publication authority the sprint doc already assigns it, deferred rather
than cancelled.

This document records what B.7's *review* machinery proves today, against
this branch's actual (partial) tree state: manifest structure, rebuildable
candidate evidence, and isolated consumer-matrix qualification. It is not a
publication record: nothing described here has been published to any
registry, no registry credentials have been sought, and no publish workflow
is dispatched or installed. See `release/bindings-artifacts.toml` for the
machine-readable source of truth this document summarizes.

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
files exist in this branch's tree and are pinned to `1.4.0`. Tauri's real
IPC/artifact qualification has passed in the recorded B.3a matrix, while
independent phase-end QA and formal API/ADR approval remain pending. The npm
entry remains pending because sc-publish credential provisioning and publication
approval are owner-deferred; `NPM_TOKEN` is only a workflow configuration key.

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

Existing approval records under `docs/api-approvals/` include Markdown records
with required `## Scope` / `## Approval` / `## Affected Artifacts` headings,
and machine-readable JSON records used by the API governance gate:

- `docs/api-approvals/phase-a-a3-writer-runtime.md`
- `docs/api-approvals/phase-b-log-import.md`
- `docs/api-approvals/phase-b-runtime-level.md` (owner-deferred; not a
  current approval)
- `docs/api-approvals/phase-b-integration-review-decimal.json` (scoped lead
  approval for the unpublished DTO DecimalDtoError API change; public API only)

The DecimalDtoError record specifically approves the `sc-observability-dto`
public surface at the exact recorded digest, limited to the enum, stable error
codes and the two constructor Result signature changes. It does not approve
the owner ADR/runtime contract, independent QA, publication, or any unrelated
crate. Other binding surfaces retain their separate review and owner-deferral
gates; this packet does not infer approval from the DTO record.

## Platform results

The B.4a Python wheel matrix (`.github/workflows/b4a-python-distributions.yml`,
policy at `release/python-platform-policy.json`) already builds and qualifies
the full 5-platform x 5-interpreter (25-cell) matrix for the `sc-observability`
PyPI package's sdist/wheels. That reusable workflow's `workflow_call` interface
is available for `sc-publish` (or any future publish pipeline) to consume
later; `.github/workflows/release.yml` does not call it and installs no
binding-publish jobs, per the owner scope correction above.

`bindings/typescript/` and `bindings/tauri/` now exist in this branch's tree.
`sc-observability-tauri` compiles standalone and its real IPC/artifact platform
matrix is recorded as passed in the B.3a qualification handoff. Independent
phase-end QA and formal API/ADR approval remain separate gates. TypeScript has its
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
  additionally runs `npm pack` and installs the real resulting
  tarball into an isolated npm project, then runs a `node -e` smoke check
  against it. This is a genuine forward-looking structural proof of the
  packaged file set, but it is explicitly labeled as such and never as
  publish-readiness -- the manifest entry stays `pending` and no summary
  line claims otherwise.

## Sprint status (updated after the owner sequencing correction)

**aobs rejected the first pass of this readiness machinery as too weak**:
`validate_binding_registry_consumers.sh` succeeded on missing packages,
skipped pending entries, and only `cargo check`ed local path dependencies
(C03); the manifest declared `ready` from file existence alone, with no
pinned source, artifact hashes, schema, or matrix proof (C05). Those two
items were genuinely reworked and are described below, and remain part of
this branch's review packet.

aobs's C04 direction additionally asked for `release.yml` to install real
crates.io/PyPI/npm publish jobs for the binding artifacts, gated on the
manifest, and to block the `release` job on all of them. That work was done,
verified, and then **reverted**: the actual owner corrected the sequencing
after the fact -- there is no mid-phase publication, and B.7 is the sole
phase-end publication step for all of Phase B (core, bridge/macros, and
bindings alike), gated on explicit owner review that has not happened yet.
Until that review authorizes it, no publication workflow is dispatched and
no registry credentials are sought; a shared publish pipeline ("sc-publish")
is a separate follow-up outside this phase's installation scope. This is a
deferral, not a cancellation of B.7's eventual publication role.
`.github/workflows/release.yml` no longer contains `publish-binding-crates`,
`precheck-python-wheel-build`, `qualify-python-wheel-matrix`,
`publish-python-wheel`, or `publish-npm-client`; the `release` job's `needs:`
is back to `[gate-and-tag, publish]` (the original 6-core-crate gate only).
The one narrow, independently-valid fix from that work that *was* kept is
the process-substitution exit-code bugfix in the pre-existing `publish` job
(described below) -- it corrects a real bug in a job that already existed
before B.7, and does not itself install or wire any new publish pipeline.

- `release/bindings-artifacts.toml` exists, parses, and correctly records 4
  crates.io entries (3 `ready`, 1 `pending` with a named reason) and 2
  package entries (1 `ready`, 1 `pending` with a named reason). Every entry
  now also carries `registry_secret`/`registry_secret_configured`
  (registry credential provisioning is owner-deferred; token names are workflow
  configuration keys), and the pypi entry carries `platform_policy_ref` pointing at
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
  packed TypeScript tarball; (4) an honest summary that
  never prints "registry consumer validation passed" unless
  `--live-registry-check` actually queried a live registry (it did not in
  this readiness work -- nothing has been published). Verified by actually
  running it end to end during this rework: exit 0, all sections passed.
- `.github/workflows/release.yml`: kept only the process-substitution
  exit-code bugfix in the pre-existing `publish` job (plan captured via
  `plan="$(...)"` then read via a here-string, not `done < <(...)`, so a
  broken manifest/script actually fails the job instead of silently
  publishing nothing). Every binding-publish job that was added for C04
  (`publish-binding-crates`, `precheck-python-wheel-build`,
  `qualify-python-wheel-matrix`, `publish-python-wheel`,
  `publish-npm-client`) has been removed, and the `release` job's `needs:`
  is back to `[gate-and-tag, publish]`. A comment in the workflow points at
  the manifest/script/validator tooling below as where B.7's review evidence
  actually lives, and names `sc-publish` as the intended future consumer of
  that evidence.

**This is a review packet, not a publication.** Nothing in this branch
uploads to a registry, creates a release tag beyond what the pre-existing
6-core-crate `gate-and-tag`/`publish` jobs already did before B.7, dispatches
a publish workflow, or seeks npm/PyPI credentials -- that stays true until
owner review authorizes B.7's actual phase-end publication pass. What B.7
hands off for that eventual, owner-reviewed pass (via `sc-publish` once it
is installed, or whatever mechanism the owner review settles on) to consume:

(a) `release/bindings-artifacts.toml`, a readiness manifest naming 4 crates.io
    entries (3 `ready`, 1 `pending`: `sc-observability-tauri`, pending
    independent phase-end QA/API approval) and 2 package entries (1
    ready-but-unpublished PyPI package, 1 pending npm client awaiting
    owner-deferred sc-publish credential provisioning and publication approval);
(b) `scripts/release_bindings_artifacts.py`'s `build-evidence`/`verify-evidence`,
    which produce and re-verify real, rebuildable candidate artifact hashes
    for every `ready` entry;
(c) `scripts/ci/validate_binding_registry_consumers.sh`'s real isolated
    Rust/Python/TypeScript consumer-matrix checks against those built
    artifacts (never a live registry);
(d) this document and `docs/api-approvals/phase-b-py.md`, recording the
    approvals and gaps a future publish pass will need.

None of (a)-(d) installs, dispatches, or authenticates against a publish
pipeline; they only prove structural/hash/consumer readiness for B.7's
eventual owner-reviewed publication pass. This sprint is **not** marked
complete by this review packet -- per the owner's explicit instruction, B.7
does not claim publication has happened or that it is authorized yet, and
its sprint doc's frontmatter `status` is not set to `complete`.
