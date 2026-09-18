# Publishing Guide

This repo uses a single source of truth for release artifacts:

- Manifest: `release/publish-artifacts.toml`
- Loader/validator: `scripts/release_artifacts.py`

Do not hardcode crate lists or publish order in docs or workflows. Update the
manifest instead.

## Distribution Channels

- **crates.io**: all ten intended publishable Rust crates in
  `release/publish-artifacts.toml`, in dependency order
  - [`sc-observability-types`](https://crates.io/crates/sc-observability-types)
  - [`sc-observability`](https://crates.io/crates/sc-observability)
  - [`sc-observe`](https://crates.io/crates/sc-observe)
  - [`sc-observability-otlp`](https://crates.io/crates/sc-observability-otlp)
  - [`sc-observability-log-macros`](https://crates.io/crates/sc-observability-log-macros)
  - [`sc-observability-log`](https://crates.io/crates/sc-observability-log)
  - `sc-observability-dto`
  - `sc-observability-binding-runtime`
  - `sc-observability-tauri` (standalone host workspace)
  - `sc-observability-py` (PyO3 support crate)
- **GitHub Releases**: <https://github.com/randlee/sc-observability/releases>

Phase B's binding and native artifacts are inventoried separately in
`release/bindings-artifacts.toml`: the DTO and native-runtime crates, the
standalone Tauri host crate, the PyO3 extension plus PyPI wheel/sdist, and the
generated TypeScript npm client. The manifest records dependency order,
platform-matrix references, and deliberate `pending` publication status; it
does not publish anything. Phase B records readiness only; after this phase
merges, the shared `sc-publish` migration/preflight is a Phase C task. Registry
publication is separately authorized after that migration, with BTIT adoption
following publication.

## Workflows

- Preflight: `.github/workflows/release-preflight.yml`
- Release: `.github/workflows/release.yml`

Both workflows are manual dispatch (`workflow_dispatch`).

## Standard Flow

1. Ensure `develop` contains the release version bump and all work is merged.
2. Run the preflight workflow from `develop` (or `main` post-merge):
   - `version=<X.Y.Z or vX.Y.Z>`
   - `run_by_agent=publisher`
3. Preflight validates formatting, clippy, tests, manifest completeness, publish
   order, repo boundaries, and version consistency. It runs `cargo publish
   --dry-run` for each crate in manifest order.
4. Merge `develop` to `main` once CI and preflight are green.
5. Do not dispatch the legacy release workflow for the ten-entry Phase-B
   inventory: its `cargo publish -p` path predates the standalone Tauri and
   binding channels and is not suitable or authorized for this surface.
6. After Phase C migrates and preflights `sc-publish`, that caller-owned
   contract reconciles the two readiness inventories; separate owner
   authorization is still required before registry/release actions.

## Initial Publish Note

For a first publish of any currently absent crate, preflight automatically
detects initial-release mode and uses `--no-verify` on the dry-run to skip path
dependency resolution. This is safe because correctness is already validated
by the preceding fmt/clippy/test steps.

## Publish Order

Crates must be published in dependency order (defined in the manifest):

| Order | Crate | Wait after publish |
|-------|-------|--------------------|
| 1 | `sc-observability-types` | 30s |
| 2 | `sc-observability` | 30s |
| 3 | `sc-observe` | 30s |
| 4 | `sc-observability-otlp` | — |
| 5 | `sc-observability-log-macros` | 30s |
| 6 | `sc-observability-log` | — |
| 7 | `sc-observability-dto` | 30s |
| 8 | `sc-observability-binding-runtime` | 30s |
| 9 | `sc-observability-tauri` | 30s |
| 10 | `sc-observability-py` | — |

## Local Validation Commands

```bash
# Show publish plan
python3 scripts/release_artifacts.py list-publish-plan \
  --manifest release/publish-artifacts.toml

# Validate manifest completeness against workspace
python3 scripts/release_artifacts.py validate-manifest \
  --manifest release/publish-artifacts.toml \
  --workspace-toml Cargo.toml

# Verify version matches workspace
python3 scripts/release_artifacts.py verify-version \
  --manifest release/publish-artifacts.toml \
  --workspace-toml Cargo.toml \
  --version 1.0.0
```

## Updating Release Artifacts

When adding or reordering crates:

1. Update `release/publish-artifacts.toml`.
2. Run `validate-manifest` locally to confirm consistency.
3. Run CI/Preflight to validate end-to-end.

No workflow edits are required for normal artifact-list changes when the
manifest is kept current.

## Binding packages (Rust DTO/native-runtime/Tauri/Python, npm, PyPI)

Language-binding artifacts (the DTO and native-runtime crates, the Tauri host
adapter crate, the Python PyPI package, and the TypeScript npm client) are
tracked separately in `release/bindings-artifacts.toml` and validated by
`scripts/release_bindings_artifacts.py` (`validate-manifest`, `verify-versions`,
`list-publish-plan`, `build-evidence`, `verify-evidence`) plus
`scripts/ci/validate_binding_registry_consumers.sh`. This is review/readiness
tooling only: `.github/workflows/release.yml` does not publish these
artifacts: Phase B records readiness only; publication is deferred until after
the phase merges and explicit owner authorization. Until then no publish
workflow is dispatched and no registry credentials are sought. Installing or
upgrading the intended shared publishing pipeline,
**`sc-publish`**, is a separate Phase C follow-up outside Phase B's
installation scope. See
[`docs/plans/phase-b/handoff-b-7.md`](./docs/plans/phase-b/handoff-b-7.md) for
the current review packet: readiness status per artifact, rebuildable
candidate evidence, and isolated consumer-matrix results.

The intended channels are crates.io for the DTO, native-runtime, and (after
its independent Tauri workspace qualification) host crate; PyPI for the
`sc-observability` wheel and sdist across the checked platform/interpreter
matrix; and npm for `@sc-observability/client`. Tauri/native binaries are
qualification artifacts consumed by the host and are not a second registry
channel. Phase B records readiness only; publication is separately authorized
after the Phase C `sc-publish` migration, and BTIT adoption follows publication.

Deliberate exclusions are `sc-observability-log-consumer-check` (CI-only
consumer proof), `bindings/schema-generator` (build tooling), and example
applications, including `examples/atm-adapter-example` (excluded from the
root workspace) and the Tauri/Python examples. They remain test or tooling
inputs and must not be published as library artifacts.
