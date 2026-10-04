# Phase E contract versioning

User-authorized addition: retain versioned public contracts and detect changes through fast unit tests. This is separate from the six integration suites; it adds no suite/OS cells or automatic CI workflows. Sc-lint is still unspecified.

## Version rules

Keep released snapshots immutable. A contract change requires a new versioned snapshot and an explicit current-version selection. Package/API versions, wire-format versions and SQLite migration versions have separate lifecycles. Keep existing wire identifiers and backwards-readable configuration. Historical snapshots are retained, not regenerated to silence a failure.

Generated snapshots must represent actual current code. A comparison of two checked-in files cannot prove the code matches, nor can a single-platform extraction prove parity. Reuse existing generators and serializers. The complete sprint API check must meet the user's under-one-minute requirement; measure generation and comparison, and expose compilation cost rather than hiding it. The Rust extraction approach remains an implementation feasibility question; do not quietly substitute stale evidence.

## Work items

| Sprint | Contract | Storage / existing source |
| --- | --- | --- |
| e-8 | Rust, Python and TypeScript public APIs | `schema/api/<surface>/<version>.json`; existing API tools |
| e-9 | Binding/IPC payloads and errors | `schema/bindings/`; existing binding schema generator and v1 contracts |
| e-10 | CLI command syntax and JSON responses | `schema/cli/sc-otel/commands/` and `results/`; clap and actual response types |
| e-11 | Public configuration/input formats | `schema/config/<format>/<version>.json`; existing parsers/defaults |
| e-12 | Durable SQLite storage | Immutable SQL/migrations alongside the existing durable-store implementation |

These do not depend on e-1 or each other. Resolve overlapping existing-source edits at publication rather than impose new prerequisite chains. Each has its normal dev/sanity/QA records. e-8 reuses `obs-phase-e-api-governance-replacement` rather than duplicating that placeholder.

## CLI tooling

Use clap's `CommandFactory` and built command introspection (`get_subcommands`, `get_arguments`, argument properties and relationships) to produce normalized input syntax. See [clap Command](https://docs.rs/clap/latest/clap/struct.Command.html). `clap_mangen` generates manpages; it is not a JSON response-schema generator. A small deterministic projection is sufficient; do not add a second CLI specification or unrelated documentation generator.

## Existing compatibility to preserve

Binding schemas already identify v1, CLI results use `sc-otel.result/v1`, and durable SQLite uses `PRAGMA user_version = 1` with newer-schema rejection. Preserve these baselines. Move binding schema ownership only with its consumer paths updated; retain compatibility references as required, never two editable sources. Do not copy upstream-owned OTLP schemas. Database work adds neither a cache nor seven-day retention and invents no migration merely to demonstrate machinery.

Implementation tests must detect a deliberate contract mismatch and retain valid current behavior. No new dashboards, approval certificates, standalone CI jobs, or exhaustive release matrix. Snapshot history is necessary contract data; the plan itself retires once implemented.
