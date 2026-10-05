# Published API history

The package manifest selects the versioned file for each language. These are
initial **1.5.0 implementation baselines for the next release cut**, not
reconstructed evidence for earlier releases. Once accepted, retain the files
unchanged; an intentional contract change selects a new package version.
ADR-020 compatibility requirements still apply.

Rust snapshots store a sorted row table and sorted row-index lists for each
manifest-derived Cargo feature family. The ten published packages account for
41 declared release families. The ordinary workspace test build currently
produces nine of those package/family combinations; separately workspaced Tauri
and disabled feature families are not covered by that workspace invocation.
Baselines are common across native targets because ADR-022 requires public API
parity. A local capture does not establish native Windows or Linux parity.

Current API inputs are actual completed Cargo metadata artifacts, installed
Python runtime objects, and emitted TypeScript declarations/runtime exports.
No checker builds those inputs. Python static-only stubs and dynamic attributes,
macro expansion behavior, and runtime semantics retain their independent tests.
Compiler metadata format is pinned to Rust 1.94.1. A compiler upgrade requires
review of the projection format and historical compatibility.

Run `just test`, the existing installed Python wheel tests, or
`npm test --prefix bindings/typescript` as appropriate for the producing package.
The Rust API comparison runs only through `scripts/api/run_unit_tests.py`.
Normal tests fail API mismatches and never refresh baselines. See the
[release-cut and version-update instructions](../../docs/plans/phase-e/schema-versioning.md#api-unit-comparison-and-release-cut-setup).

Development evidence on macOS arm64 (2026-10-04):

- Final 41-family Rust release setup: 18.460s with valid warm Cargo artifacts.
  This is explicit publishing setup, separate from ordinary comparison. Earlier
  cold/repeated prototypes took longer and are not subminute cold-build claims.
- Ordinary workspace build/test: 34.444s, followed by API comparison 0.837s
  (0.690s metadata inspection), nine actual built library families, zero builds
  in the API check. Runtime tests retained their known F14 trybuild mismatch;
  API comparison independently passed after Cargo completed.
- Installed wheel Python check: 0.060s on CPython 3.10 and 0.032s on CPython
  3.14, both against the same 1.5.0 runtime baseline. Wheel build setup was
  separate (5.04s Cargo build); these are native macOS wheel results.
- TypeScript ordinary npm test: existing build/runtime tests and compiled
  declaration mutation tests passed; API comparison took 0.156s with zero builds
  in the check.
- Native Windows and Linux timings have not yet been measured;
  the existing native unit/package jobs run the same applicable checks. No new
  workflow or job was added.
