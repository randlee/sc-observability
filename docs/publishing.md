# Publishing and Version Ownership

## Purpose

This repo becomes the publishing source of truth for:
- `sc-observability-types`
- `sc-observability`
- `sc-observe`
- `sc-observability-otlp`

These crates currently exist inside the `agent-team-mail` workspace. After
cutover, new releases of these crate names must come from this repo instead.

## Versioning

- The repo uses a single workspace version.
- All published crates in this repo must share that version.
- The initial standalone release must be strictly higher than the last version
  published from the ATM workspace for these crate names.
- The initial standalone release for this repo is `1.0.0`.
- Release workflows verify that the requested release version matches:
  - workspace version
  - each crate package version
- Release preflight uses two modes:
  - if `sc-observability-types` already exists on crates.io, preflight runs the
    normal dependency-aware `cargo package --locked --allow-dirty` checks in
    publish order
  - if `sc-observability-types` is not yet on crates.io, preflight treats the
    run as the initial standalone publish and uses
    `cargo publish --dry-run --locked --no-verify` for each crate in publish
    order so downstream path dependencies do not fail crates.io resolution

## Replacement/Cutover Rule

Before the ATM workspace switches to crates.io dependencies from this repo:
1. This repo must publish the target version of `sc-observability-types`.
2. This repo must publish the target version of `sc-observability`.
3. This repo must publish the target version of `sc-observe`.
4. This repo must publish the target version of `sc-observability-otlp`.
5. ATM must then replace its in-workspace path dependencies with version pins.

## Source of Truth

- Manifest: `release/publish-artifacts.toml`
- Preflight workflow: `.github/workflows/release-preflight.yml`
- Release workflow: `.github/workflows/release.yml`
- Release notes template: `release/RELEASE-NOTES-TEMPLATE.md`
- Release exit checklist:
  [docs/release-readiness-checklist.md](./release-readiness-checklist.md)
