# Publishing and Version Ownership

## Phase C migration (C.1 preparation; installation held)

This document currently describes the repository-specific publishing
implementation. [Phase C](plans/phase-c/plan-phase-c.md) proposes replacing
that implementation with the shared `../sc-publish` package, expanding
coverage to Phase B's full ten-crate release surface (Python wheels/sdist,
the npm client, the standalone Tauri workspace), and treating the shared
package's missing npm channel and stale pinned action versions as named
upstream prerequisites that `../sc-publish` must satisfy at a reviewed pin —
not a repository-local publisher fork and not an accepted regression.
The reviewed caller contract is `install.json`. Installation is intentionally
held until `sc-publish` lands both an npm channel and current action-major pins;
the sibling revision inspected for C.1 (`3a57926b3e939835644aad944a614cbb48e4d5fc`) satisfies neither prerequisite. No
publication, tag, or release dispatch is authorized. See
`docs/plans/phase-c/sprint-c-1-shared-pipeline-migration.md` for the exact
asset replacement/removal/retention table and
`docs/plans/phase-c/sprint-c-2-release-surface-preflight.md` for the
preflight this document will grow to reflect after C.1 lands.

## Purpose

This repo becomes the publishing source of truth for:
- `sc-observability-types`
- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`
- `sc-observability-log-macros`
- `sc-observability-log`
- `sc-observability-dto`
- `sc-observability-binding-runtime`
- `sc-observability-tauri`
- `sc-observability-py`

Binding and native artifacts are tracked in `release/bindings-artifacts.toml`
(DTO, native runtime, Tauri host, PyO3 extension, PyPI wheel/sdist, and npm
client). The ten Rust crates above use `release/publish-artifacts.toml`; the
binding manifest is a readiness inventory only and all publication remains
deferred until after Phase B merge and separately authorized following the
Phase C `sc-publish` migration; BTIT adopts the published artifacts afterward.

These crates currently exist inside the `agent-team-mail` workspace. After
cutover, new releases of these crate names must come from this repo instead.

## Versioning

- The repo uses a single workspace version.
- All published crates in this repo must share that version.
- The initial standalone release must be strictly higher than the last version
  published from the ATM workspace for these crate names.
- Verification uses the workspace candidate version (currently `1.4.0`),
  rather than a hard-coded historical `1.0.0` example.
- Release workflows verify that the requested release version matches:
  - workspace version
  - each crate package version
- Prepublication qualification is available from a candidate branch and does
  not require publisher impersonation, registry credentials, tags or `main`.
- `prepare_log_staged_packages.py` invokes Cargo's full workspace packaging
  and verification, excluding the private consumer fixture. Cargo uses its
  temporary package registry to verify unpublished chained dependencies.
  Every package is verified; none is skipped and `--no-verify` is forbidden.
- The release manifest preserves dependency order (core types/runtime,
  observation/OTLP, macros/bridge, then binding crates). A future Phase-C
  `sc-publish` workflow consumes this manifest. `wait_for_registry_version.py` checks the sparse index after
  each real publication with 12 bounded attempts and fails the sequence visibly
  if the exact non-yanked version does not appear. B.2 tests this gate with
  mocked responses and never invokes live publication.
- `.github/workflows/b2-staged-consumer.yml` distributes a single immutable
  stage to macOS, Linux and Windows; all three must attest the same candidate
  source SHA and archive checksums. Third-party dependencies can use crates.io;
  all first-party dependencies resolve only from freshly verified extractions.
- B.P2's historical four-package stage uses `release/bp2-publish-artifacts.toml`.
  Its existing artifacts and evidence are not regenerated as B.2 evidence.

### B.2 candidate workflow (no publication)

```sh
python3 scripts/ci/prepare_log_staged_packages.py --version 1.4.0 --output target/b2-stage/<source-sha>
python3 scripts/ci/validate_log_staged_consumer.py --version 1.4.0 --stage target/b2-stage/<source-sha> --source-commit <source-sha> --result-file target/b2-evidence/macos.json
```

The source must be clean and committed. Existing stage directories are immutable
and never overwritten. `stage-manifest.json` records each real Cargo archive,
its normalized manifest, all file hashes, its checksum and the source SHA.
The shared consumer checks an enabled macro, explicit flush and shutdown, and
persisted JSONL fields. `handoff-b-2.md` records results and adoption instructions.
B.7 records the final readiness source and evidence. Phase C installs and
qualifies the shared `sc-publish` pipeline; live publication is separately
authorized afterward, followed by BTIT's published-artifact adoption and
registry-only consumer verification.

## Replacement/Cutover Rule

Before the ATM workspace switches to crates.io dependencies from this repo:
1. This repo must publish the six core/bridge crates in
   `release/publish-artifacts.toml` in manifest order.
2. After Phase B merges, Phase C must migrate and preflight the shared
   `sc-publish` pipeline; publication then requires separate owner
   authorization and registry-consumer proof.
3. ATM must then replace its in-workspace path dependencies with version pins.

## Source of Truth

- Manifest: `release/publish-artifacts.toml`
- Preflight workflow: `.github/workflows/release-preflight.yml`
- Release workflow: `.github/workflows/release.yml`
- Release notes template: `release/RELEASE-NOTES-TEMPLATE.md`
- Release exit checklist:
  [docs/release-readiness-checklist.md](./release-readiness-checklist.md)

### Shared-pipeline credential scopes (planned)

The shared contract requires repository-scoped `CARGO_REGISTRY_TOKEN` for
`crates_io`. PyPI uses environment-scoped `PYPI_API_TOKEN` in the `pypi`
environment and `TEST_PYPI_API_TOKEN` in `testpypi`; no values are documented
here. npm secret names remain an upstream contract decision and are not
invented locally while the npm channel prerequisite is unmet.

## Public API Visibility

- CI publishes the generated public API governance reports from
  `target/public-api/` as a GitHub Actions artifact named
  `public-api-governance-<github.run_id>-<github.sha>`.
- Release notes should include versioned docs.rs links for every published crate
  so consumers can inspect the exact shipped API surface:
  - `sc-observability-types`
  - `sc-observability`
  - `sc-observe`
  - `sc-observability-otlp`
  - `sc-observability-log-macros`
  - `sc-observability-log`
- The changelog entry for each release should include the same versioned docs.rs
  links so the release record points directly at the published API docs.
