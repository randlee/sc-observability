# Phase E contract versioning

User-authorized addition: retain versioned public contracts and detect changes through fast unit tests. This is separate from the six integration suites; it adds no suite/OS cells or automatic CI workflows. Sc-lint is still unspecified.

## Version rules

Keep accepted/merged snapshots immutable, including prerelease versions. A contract change requires a new versioned snapshot and an explicit current-version selection. Package/API versions, wire-format versions and SQLite migration versions have separate lifecycles. Keep existing wire identifiers and backwards-readable configuration. Historical snapshots are retained, not regenerated to silence a failure.

Generated snapshots must represent actual current code. A comparison of two checked-in files cannot prove the code matches, nor can a single-platform extraction prove parity. Reuse existing generators and serializers. The complete sprint API check must meet the user's under-one-minute requirement; measure generation and comparison, and expose compilation cost rather than hiding it. The Rust extraction approach remains an implementation feasibility question; do not quietly substitute stale evidence.

## Work items

| Sprint | Contract | Storage / existing source |
| --- | --- | --- |
| e-8 | Rust, Python and TypeScript public APIs | `schema/api/<surface>/<version>.json`; existing API tools |
| e-9 | Binding/IPC payloads and errors | Existing canonical `bindings/schema/`, retaining immutable versions there |
| e-10 | CLI command syntax and JSON responses | `schema/cli/sc-otel/commands/` and `results/`; clap and actual response types |
| e-11 | Public configuration/input formats | `schema/config/<format>/<version>.json`; existing parsers/defaults |
| e-12 | Durable SQLite storage | Immutable SQL/migrations alongside the existing durable-store implementation |

These do not depend on e-1 or each other. Each has a disjoint file fence in its authoritative bead: e-11 owns public configuration parsers/tests; e-12 owns storage schema/opening/tests, not the configuration loader. No broad shared durable-directory ownership is permitted. Shared files have one owner, not publication-time conflict resolution as a substitute. Each has its normal dev/sanity/QA records. The existing e-8 placeholder is renamed to `obs-e-8`, not duplicated.

## CLI tooling

Use clap's `CommandFactory` and built command introspection (`get_subcommands`, `get_arguments`, argument properties and relationships) to produce normalized input syntax. See [clap Command](https://docs.rs/clap/latest/clap/struct.Command.html). `clap_mangen` generates manpages; it is not a JSON response-schema generator. A small deterministic projection is sufficient; do not add a second CLI specification or unrelated documentation generator.

## Existing compatibility to preserve

Binding schemas already identify v1, CLI results use `sc-otel.result/v1`, and durable SQLite uses `PRAGMA user_version = 1` with newer-schema rejection. Preserve these baselines. Keep canonical binding schema paths unchanged, including existing generation-manifest and caller contracts. The active binding v1 files ARE the initial version-1 snapshots; do not copy them to an additional baseline. Likewise the existing durable `schema.sql` IS its initial v1 snapshot; retain that original when a real later version is added. Keep one authoritative file per version, without relocating current callers or creating a second editable source. Do not copy upstream-owned OTLP schemas. Database work adds neither a cache nor seven-day retention and invents no migration merely to demonstrate machinery.

Implementation tests must detect a deliberate contract mismatch and retain valid current behavior. No new dashboards, approval certificates, standalone CI jobs, or exhaustive release matrix. Snapshot history is necessary contract data; the plan itself retires once implemented.

## Selected versions and history checks

Public API snapshots use the owning published package version from its existing
package manifest, separately per language surface. Wire/config schemas use their
existing identifier where present; otherwise the owning parser's test selects a
literal contract version (initially 1), without adding a field to user files.
SQLite continues to select through `PRAGMA user_version`.

Supply an existing local accepted/merged baseline commit (the release tag/commit
where applicable): its historical snapshot files cannot be edited or deleted. A changed API under an
accepted package version must bump that package version and add its snapshot;
independent wire/config changes must select a new contract version and retain
prior files. Only an initial draft that has not been accepted/merged may be corrected in place.
An accepted prerelease snapshot is immutable too; its next contract change
requires a new version.
The first snapshot capture establishes the current implementation baseline,
not invented historical snapshots. Tests compare generated current code to the
selected snapshot and check retained accepted files against that explicit local git
accepted baseline. Implement the history assertion inline in each existing
per-surface unit test using `git show <supplied-local-base>:<path>`; its consumer
is the user-required rejection of changes without a new version. The sprint
owning that surface owns the assertion. No shared history helper, network fetch,
service, new workflow or approval mechanism is added. The existing manifest
version is the API selection; do not create a redundant selection artifact.
Mismatch output names the contract, version and difference; adding a new version
is the remedy for an actual contract change, not overwriting accepted history.

## API verification feasibility and ADR-022

e-8 owns the ADR-022 verification amendment for the user's snapshot decision,
including the API layout, Rust/Python/TypeScript surfaces, version selection and
immutability. Reconcile the verification wording without changing the rule that
platform-dependent public APIs require an explicit ADR amendment.
It retains uniform public interfaces for the same release target/features and
released compatibility. The fast unit comparison must obtain its current
surface from current source/compiled exports, with deterministic normalization;
comparing two stored snapshots is insufficient. Its first implementation step is a bounded prototype using the existing
`public_api_parity.py` extractor and local build artifacts on current source;
e-8 measures the complete path on this repository: total fresh
verification must be under one minute, including any compilation it needs.
Include source invalidation in that measurement. Separately record extraction, compilation and comparison durations; cached
results cannot conceal a stale source revision. If no authoritative method
meets the budget, e-8 acceptance fails: report the concrete result for a user
decision before replacing it with a weaker test. Independent sprints continue;
this plan does not claim the prototype has already demonstrated feasibility. Do not add another automatic extraction job.

Existing native release producers own release-target/feature parity proof; e-8
consumes that evidence read-only and does not add another cross-platform job or
edit release workflows. The fast local unit comparison cannot claim native
parity. The amendment distinguishes these proofs without reducing the common-API
contract or adding an exhaustive new matrix.

## Configuration compatibility

e-11 covers telemetry YAML (`TelemetryFileConfig`, loaded by
`durable/config_file.rs`) and the importer YAML supplied through `--config`
(`scripts/sanity-telemetry/import_sanity.py`, `load_config`/`sources`).
Capture actual accepted fields, defaults and unknown-key rules for both.
Importer invalid source kinds and duplicate paths remain rejected. In-memory
configuration types are observed read-only by e-8 extraction; e-11 owns the
configuration parser/test edits. CLI result schemas belong to e-10. In particular,
`durable/config_file.rs` intentionally ignores consumer-specific YAML keys;
its versioning tests must preserve that behavior, not turn it into strict
unknown-key rejection. Invalid-value cases follow existing parser rejection.
Internal temporary files and upstream-owned OTLP wire schemas stay excluded.
