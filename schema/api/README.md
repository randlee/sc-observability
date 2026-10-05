# Published API history

The package manifest selects the versioned file for each language. These are
initial **1.5.0 implementation baselines for the next release cut**, not
reconstructed evidence for earlier releases. Once accepted, retain the files
unchanged; an intentional contract change selects a new package version.
ADR-020 compatibility requirements still apply.

Rust compatibility baselines are the ten native text files under `rust-stock/`.
`scripts/ci/stock_public_api.py setup` prepares the current native text in its
caller-owned target directory; its `check` mode reads supplied rustdoc JSON with
the pinned stock tool and compares it with those committed text baselines. The
check does not build a candidate crate. The retired Rust JSON draft snapshots
are not accepted API history.

Python and TypeScript retain their versioned JSON baselines. Their checks use
the supplied accepted base to ensure existing baselines remain byte-for-byte
unchanged. Python runtime behavior and TypeScript declarations/runtime exports
retain their independent tests.

Run `just test`, the existing installed Python wheel tests, or
`npm test --prefix bindings/typescript` as appropriate for the producing package.
The Rust API comparison runs through `scripts/ci/stock_public_api.py`: CI
performs its candidate setup first, then runs the no-build `check` mode.
Normal tests fail API mismatches and never refresh baselines. See the
[release-cut and version-update instructions](../../docs/plans/phase-e/schema-versioning.md#api-unit-comparison-and-release-cut-setup).

Development evidence on macOS arm64 (2026-10-04):

- Installed wheel Python check: 0.060s on CPython 3.10 and 0.032s on CPython
  3.14, both against the same 1.5.0 runtime baseline. Wheel build setup was
  separate (5.04s Cargo build); these are native macOS wheel results.
- TypeScript ordinary npm test: existing build/runtime tests and compiled
  declaration mutation tests passed; API comparison took 0.156s with zero builds
  in the check.
