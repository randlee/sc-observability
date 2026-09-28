# 2.0.0 qualification candidate

This document records the D18 release-policy and inventory alignment only.
Publication, tagging, and final semver approval remain separately authorized
steps.

## Publishable inventory

The candidate inventory contains ten publishable Rust crates in dependency
order, the PyPI `sc-observability` wheel/sdist channel, and the existing shared
npm channel `@synaptic-canvas/sc-observability`. See
[`release/release-inventory.json`](release-inventory.json) and
[`release/bindings-artifacts.toml`](bindings-artifacts.toml).

## Python compatibility policy

The PyO3 extension publishes `abi3-py310` wheels tagged `cp310-abi3` with
`Requires-Python >=3.10` and no upper or exclusion cap. The immutable
qualification matrix covers six platforms and 29 native installed-suite cells:
all five existing platforms run Python 3.10–3.14, while Windows ARM64 runs
3.11–3.14 because native CPython 3.10 is unavailable. Changing the floor,
adding an upper bound, or changing the interpreter matrix requires a separately
approved compatibility decision and corresponding validator expectations.

## API and migration evidence

The canonical 2.0 break manifest remains `release/public-api-policy.json`.
Migration guidance is maintained in `docs/migration-guide.md`,
`docs/migration.md`, and `docs/migrate-error-api.md`; these documents describe
the accepted contract but do not authorize publication.
