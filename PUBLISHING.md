# Publishing Guide

This repo uses a single source of truth for release artifacts:

- Manifest: `release/publish-artifacts.toml`
- Loader/validator: `scripts/release_artifacts.py`

Do not hardcode crate lists or publish order in docs or workflows. Update the
manifest instead.

## Distribution Channels

- **crates.io**: all four crates in the workspace
  - [`sc-observability-types`](https://crates.io/crates/sc-observability-types)
  - [`sc-observability`](https://crates.io/crates/sc-observability)
  - [`sc-observe`](https://crates.io/crates/sc-observe)
  - [`sc-observability-otlp`](https://crates.io/crates/sc-observability-otlp)
- **GitHub Releases**: <https://github.com/randlee/sc-observability/releases>

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
5. Run the release workflow with `version=<X.Y.Z or vX.Y.Z>`.
6. Release workflow tags, publishes crates in manifest order (with propagation
   waits between crates), and creates the GitHub release.

## Initial Publish Note

For the first publish (all four crates absent from crates.io), preflight
automatically detects initial-release mode and uses `--no-verify` on the
dry-run to skip path dependency resolution. This is safe because correctness
is already validated by the preceding fmt/clippy/test steps.

## Publish Order

Crates must be published in dependency order (defined in the manifest):

| Order | Crate | Wait after publish |
|-------|-------|--------------------|
| 1 | `sc-observability-types` | 30s |
| 2 | `sc-observability` | 30s |
| 3 | `sc-observe` | 30s |
| 4 | `sc-observability-otlp` | — |

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
