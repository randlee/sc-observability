# CI scope and retirement policy

Effective 2026-10-02, lint, unit and integration checks run on every pull
request, without branch or path filters, and on pushes to `develop`, `main`
and `integrate/*`. Each platform test matrix is fixed to `ubuntu-latest`,
`macos-latest`, `windows-latest`, with `fail-fast: false`. A PR's base never
reduces coverage or makes a failed gate advisory. Public API diff exit 1 means
an API change to assess; tool failures, semver failures and missing required
approvals fail the job.

## Categories

| Category | Jobs (existing IDs retained) |
| --- | --- |
| Lint | CI `fmt`, `clippy`, `docs-consistency`, `version-literals`, `public-api-governance`; Python packaging `boundaries` |
| Unit | CI `test`, `manifest-validation` (including CI script tests) |
| Integration | CI `integration` (all OTLP integration targets/features); `native-contract`, `packaged-consumer`, `binding-schema`, `schema-and-contract`, `real-ipc-artifacts`, `collector-conformance`, `desktop-viewer-factory-conformance`, `telemetry-e2e`, `python-source-runtime`, `source-consumer`, sanity telemetry `importer` |

`scripts/ci/cargo_workspaces.py` discovers Cargo workspace roots from Git's
tracked and non-ignored file inventory. Root-excluded packages are included.
The script's `EXCLUDED` constant is the sole list of content exclusions.
Formatting, all-target/all-feature clippy and unit tests cover every discovered
workspace without a workflow edit when another workspace is added. Failures
are collected across workspaces; multi-binary tests use `--no-fail-fast`.
The retained feature-specific tests and workspace doctests also run.
Lint failures do not suppress the unit or integration category.

## Actual platform limits

- The pinned desktop viewer manifest carries `darwin_arm64`, `linux_amd64` and
  `windows_amd64` entries. The `desktop-viewer-factory-conformance` and
  installed-artifact `telemetry-e2e` jobs run on ubuntu, macOS and Windows on
  every PR, each selecting its host entry. The hermetic OTLP collector tests
  run on the same three platforms.
- `validate_binding_bundle.py` isolates the packaged consumer and source-bundle
  checks on all three OSes (`sandbox-exec`, `bubblewrap`, supervised Windows identity sandbox).
- `complete-gate` and `all-platforms` only aggregate the required platform
  evidence, on Ubuntu. They cannot pass on an incomplete platform inventory.
  Tauri builds on all hosts; consumers share the one Linux-produced immutable
  npm/Rust artifact pair to compare the same input across hosts.

## Expensive checks retained unconditionally

These are rough **cold-cache estimates per runner**, not measured timings or
permission to add filters. Only the user decides future path gating.

| Work | Approximate minutes | Inputs that affect it |
| --- | --- | --- |
| All workspace lint/unit, including native Tauri builds | 5–20 | Cargo manifests/locks, Rust sources, native build configuration |
| Python source wheel plus installed runtime/typing suite; importer wheel | 5–15 each | bindings/python, core/OTLP/DTO/runtime crates, scripts/ci, scripts/sanity-telemetry, toolchain/locks |
| Packaged Rust consumer and schema source bundle | 5–15 | crate sources/manifests/locks, bindings/schema, generators, bundle/isolation scripts |
| Tauri npm/Rust packaging and installed real IPC | 10–30 | bindings/typescript, bindings/tauri, shared crates, Tauri qualification scripts/fixtures |
| Installed wheel and CLI telemetry end-to-end / wheel API checks | 10–25 | Python bindings, sc-otel-cli, OTLP/types crates, telemetry tests, pinned viewer, public API and migration policies |
| Public API/semver reports | 5–15 | exported crate APIs, Cargo/toolchain, API approvals and baselines |

Thus an intermediate PR gains two platform executions for previously
Ubuntu-only work plus previously integration-only packaging/IPC jobs. These
run concurrently; summing their runner minutes is not a wall-clock estimate.
Release-only sdist/wheel distribution builds remain in their existing explicit
release/preflight workflows; this change does not turn publication jobs into
PR jobs or alter their triggers.

## Local Windows cross-checks (cargo-xwin)

Bootstrap on macOS (observed with cargo-xwin 0.23.0, rustc 1.94.1, Homebrew
LLVM 22):

```sh
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
brew install llvm   # clang-cl and llvm-lib; Apple clang has no clang-cl
```

On first use cargo-xwin downloads the MSVC CRT and Windows SDK via xwin into
`~/Library/Caches/cargo-xwin` and accepts the Microsoft licence itself; no
environment variable is needed. It links with the toolchain's `rust-lld` as
`lld-link`. Run, per discovered workspace (`--manifest-path`):

```sh
cargo xwin clippy --locked --workspace --all-targets --all-features --target x86_64-pc-windows-msvc -- -D warnings
cargo xwin test --locked --workspace --all-features --target x86_64-pc-windows-msvc --no-run
```

This proves the code compiles, passes clippy and links its test binaries for
Windows. It does not run any test; native runtime behaviour (viewer process,
file locks, paths, DLL loading) is checked only on the Windows CI runners.

## Release preflight

B.2 (`b2-staged-consumer.yml`) and B.P2 (`bp2-staged-consumer.yml`) staged
qualification are release preflight run on demand through `workflow_dispatch`
by the publisher before publishing, never on sprint or integration PRs or push
events. Their original 1.4.x release qualification is satisfied and those
packages are in use by BTIT; running their published-package checks on Phase D
sprint changes is inappropriate because the published 1.4.x dependencies lack
the new API. The package-stage, staged-consumer and complete-platform evidence
jobs, scripts, fixtures and tests remain available for release qualification.
The checked-in B.P2 authority is
`release/runtime-level-qualification.toml`, whose historical pair is candidate
`1.3.0` against baseline `1.2.0`. That pair is the recorded B.P2 qualification
used for the 1.4.x release; it is not a qualification for a later candidate.
Before a later release, the publisher must update and requalify the authority,
the frozen baseline fixture, the handoff, and the retained evidence before
dispatching B.P2 for that candidate. The publisher then dispatches the
applicable retained workflows on the candidate ref before publishing and
verifies successful completion: B.2 for the current six-package release train,
and B.P2 only when its candidate/baseline authority matches the release plan.
B.2 runs
`test_log_staging.py`; B.P2 runs
`test_validate_runtime_level_qualification_metadata.py`,
`test_validate_runtime_level_platform_evidence.py`,
`validate_runtime_level_qualification_metadata.py` and the `rustfmt --check`
check on `scripts/ci/fixtures/runtime-level-consumer/*.rs`. Release qualification runs only on dispatch. PR unit tests also exercise the
CI helper implementations, including their rejection fixtures; they do not
publish packages or replace the candidate's installed release qualification.
The preflight aggregators always require successful platform consumers.

The binding source bundle uses B.2 qualified archives only when the B.2 candidate
version matches the workspace release train; otherwise it builds unpublished
Cargo packages from source (obs-ci-stage-bundle). A corrupt or unreadable B.2
stage still fails the bundle build; only a verified release-train mismatch
falls back to source.

## Retired historical gates

`log-bridge-import-integrity`, the BTIT clone, import/adaptation validators,
Phase B/C snapshot JSON and their dedicated regression suites are retired.
The import was accepted and the crates are now maintained here; source edits
are intentional. The source-revision pin and history prerequisites for binding
generation are also retired because stack rebases rewrite commits. The
committed generated-binding source/output hash inventory is also retired.
`validate_binding_generators.py` regenerates and compares the committed schema
and language bindings, checks determinism, and rejects stale generated output.
Pinned generator toolchains, typing, runtime tests, public API/semver checks and
Cargo package integrity checks remain functional gates. Tests unrelated to these
retired provenance checks are unchanged.

Retired 2026-09-26: `validate_log_import.py`, `_log_metadata_adaptations.py`,
`_log_release_adaptations.py`, Phase B `import-provenance.json`,
`post-import-adaptations.json`, `release-adaptations-b-2.json` and Phase C
`manifest-metadata-adaptations.json`. References in historical sprint plans,
approvals and architecture records describe the acceptance gates at that time;
they do not require restoring these retired gates. The Phase D OTLP
transplant provenance manifest and its validator were retired with the
`sync-http` rename; OTLP-023 no longer requires them.
