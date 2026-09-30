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

The enumerated 2.0 break manifest is `release/public-api-major-breaks.toml`.
`release/public-api-policy.json` defines the crate inventory and frozen 1.4.1
baselines; it is not the break manifest.
Migration guidance is maintained in `docs/migration-guide.md`,
`docs/migration.md`, and `docs/migrate-error-api.md`; these documents describe
the accepted contract but do not authorize publication.

The manifest currently records the observed `from_core_health` return change
from `Result<LogHealthDto, Failure>` to direct `LogHealthDto`. Additional
canonical activation/removal breaks must be individually enumerated and
reviewed as D18 lands. Full semver qualification, final API approvals,
canonical migration fixture activation, and real composed binding tests remain
D18 obligations; this release-governance layer does not claim them complete.
