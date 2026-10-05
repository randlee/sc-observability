# Phase E contract versioning

User-authorized addition: retain versioned public contracts and detect changes through fast unit tests. Public API comparison runs after the ordinary platform unit build in the existing macOS, Windows and Linux jobs; it creates no separate workflow or job. This is separate from the six integration suites and adds no integration cells. Sc-lint is still unspecified.

## Version rules

Keep accepted/merged snapshots immutable, including prerelease versions. A contract change requires a new versioned snapshot and an explicit current-version selection. Package/API versions, wire-format versions and SQLite migration versions have separate lifecycles. Keep existing wire identifiers and backwards-readable configuration. Historical snapshots are retained, not regenerated to silence a failure.

API checks read the actual current platform's already-built library metadata.
They must not build target crates, run rustdoc, build a renderer, or compare two
stored snapshots as evidence of current code. Setup at release-cut may build
configuration families to create the published baseline. Report that setup time
separately from the normal unit comparison, which targets approximately five
seconds and must finish in less than 60 seconds per platform. A build of one
configuration does not establish coverage for disabled configurations.
## Work items

| Sprint | Contract | Storage / existing source |
| --- | --- | --- |
| e-8 | Rust, Python and TypeScript public APIs | `schema/api/<surface>/<version>.json`; existing API tools |
| e-9 | Binding/IPC payloads and errors | Existing canonical `bindings/schema/`, retaining immutable versions there |
| e-10 | CLI command syntax and JSON responses | `schema/cli/sc-otel/commands/` and `results/`; clap and actual response types |
| e-11 | Public configuration/input formats | `schema/config/<format>/<version>.json`; existing parsers/defaults |
| e-12 | Durable SQLite storage | Immutable SQL/migrations alongside the existing durable-store implementation |

The contract-history sprints own independent paths; e-8 development resumed after the accepted e-1 sanity gate. Each has a disjoint file fence in its authoritative bead: e-11 owns public configuration parsers/tests; e-12 owns storage schema/opening/tests, not the configuration loader. No broad shared durable-directory ownership is permitted. Shared files have one owner, not publication-time conflict resolution as a substitute. Each has its normal dev/sanity/QA records. The existing e-8 placeholder is renamed to `obs-e-8`, not duplicated.

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

## API unit comparison and release-cut setup

The user-approved design is an ordinary platform unit check of actual compiled
public APIs against immutable versioned baselines. `scripts/api/run_unit_tests.py`
executes the existing Cargo test command once, retains its compiler-artifact
messages, waits for completion, and then runs the already-built metadata-reader
test executable. It does not run a second build. Cargo's test failure status is
preserved, and API comparison is reported independently after unrelated runtime
test failures. The existing unit jobs retain their failure aggregation.

The unpublished `sc-observability-api-test` workspace helper uses the repository's
pinned Rust 1.94.1 compiler metadata APIs. `rustc-dev` is installed during normal
toolchain setup; bootstrap is restricted to this helper crate. The reader parses
only an in-memory `extern crate` declaration and stops after expansion, before
local analysis or code generation. It reads signatures, fields, traits,
reexports and implementations from the already-built dependency metadata.
No target source is compiled by the check. The completed Cargo artifact stream
supplies exact paths and feature selections; the checker does not guess among
old files in `target`. Source and artifact hashes reject stale or replaced inputs.

Normal developer invocation remains `just test`. To run only the existing
workspace command and its API check:

```sh
python3 scripts/api/run_unit_tests.py --accepted-base <local-accepted-commit> -- cargo test --locked --workspace --all-targets --no-fail-fast
python3 -m unittest scripts.api.test_history
```

`SC_API_ACCEPTED_BASE` can supply the local accepted baseline. Local default
`HEAD` protects accepted files against working-tree edits; use the actual
accepted/release commit to audit an already committed change. CI supplies the
PR base or preceding push commit and fetches its local history. No registry or
network query is used by the check. A mismatch identifies package, selected
manifest version, feature family and changed rows. Keep accepted snapshots,
increment the owning package version, and capture its new baseline when a
contract change is intentional. Compatibility with released 1.x APIs still
follows ADR-020; a version bump alone does not authorize a breaking change.

At the initial or next release cut, build the ordinary workspace test artifacts
and capture the current families, then explicitly prepare the other declared
families. These are publishing setup commands, never invoked by the unit check:

```sh
python3 scripts/api/run_unit_tests.py --capture --accepted-base <local-accepted-commit> -- cargo test --locked --workspace --all-targets --no-run
python3 scripts/api/cut_baseline.py --accepted-base <local-accepted-commit>
```

The release-cut command reuses the existing manifest-derived feature inventory.
It accounts for the ten published Rust packages and 41 declared feature families
at the current revision, including separately workspaced Tauri. Each comparison
reports exactly which built families it covers; an unbuilt family is not a
passing result. Accepted snapshots cannot be extended or overwritten in place,
including accepted prereleases. The first capture establishes the implementation
baseline for the next cut; it does not invent historical release snapshots.

The metadata format is pinned to Rust 1.94.1. Compiler upgrades require explicit
format review. Macros are observed through their compiled exports and expanded
public items; macro expansion behavior is still covered by the existing macro
consumer tests. Crate-owned public implementations and compiler-evaluated
auto-trait capabilities are inspected through compiler metadata, not binary
symbol tables. Dependency blanket implementations and the generic/bound rows
created only while expanding them are deliberately excluded: they are
dependency internals, not declarations made by the supported package. Missing
or unsupported metadata fails rather than silently reducing the public surface.

### Rust draft snapshot v1-to-v2 migration

The initial unreleased Phase E Rust snapshots used
`rustc-1.94.1-metadata/v1`: each family stored a pretty-printed array of row
indices, and rows included dependency blanket implementation expansion. Version
2 keeps the canonical row table but stores each family as sorted inclusive index
ranges (`0-3,7,9-11`), and projects out exactly those `blanket` rows plus their
generated `generic` and `bound` rows. Item declarations, public signatures,
fields, variants, reexports, meaningful declared bounds, crate-owned trait
implementations and auto-trait capabilities remain rows in v2.

The one-time migration is deterministic and requires an explicit historical
draft revision; it reads no artifact and runs no build:

```sh
python3 -m scripts.api.history --migrate-unreleased-rust-snapshots \
  --accepted-base <phase-e-draft-revision>
```

It refuses a changed source snapshot and writes only the exact v1 projection.
Normal API comparison separately verifies the current already-built artifact
against v2, so a signature, declared-bound, field, variant, reexport, removal,
or public capability change still produces the ordinary readable row diff. The
migration allowance is limited to current `1.5.0` Rust Phase E draft snapshots;
released history and Python/TypeScript snapshots stay immutable.

Native release producers and `public_api_parity.py` remain unchanged and provide
separate target/feature parity evidence. A local check establishes only its
current target and built feature families. It cannot establish the other native
platforms or unbuilt configuration families. Python runtime exports and TypeScript
compiled declarations likewise require their producing package artifacts; a
Rust-only build must not be reported as their verification.

### Binding producer tests

`npm test --prefix bindings/typescript` retains its existing TypeScript build and
runtime tests, then reads the emitted `.d.ts` export/reference graph and runtime
exports without emitting code. Its existing native Tauri qualification invokes
this command on macOS, Windows and Linux. The graph includes public signatures,
fields, constraints and referenced declarations. Runtime implementation behavior
is outside the API shape comparison.

The existing native Python test-suite staging copies the runtime API test and
versioned history beside the relocated tests. Pytest reflects the installed
wheel, with its installed distribution version selecting the snapshot. The
existing local wheel validator and native distribution jobs share that staging
path. Runtime signatures, annotations, class members, dataclass fields and
public reexports are checked. Static-only `.pyi` declarations, dynamically
manufactured attributes and native implementation behavior retain their existing
independent typing/runtime tests; runtime reflection does not prove them.

Binding baseline capture also uses existing producer outputs, outside testing:

```sh
# Run using the interpreter where the built native wheel is installed.
python scripts/api/cut_python_baseline.py --accepted-base <local-accepted-commit>
# After the existing TypeScript package build.
SC_API_ACCEPTED_BASE=<local-accepted-commit> node scripts/ci/public_api_typescript.mjs --capture
```

No normal test command refreshes history. The Rust workspace job verifies Rust;
it does not claim Python or TypeScript coverage. Those languages run in their
existing producing package test paths. Local macOS timings are development
evidence; Windows/Linux measurements remain native phase-end evidence. API rows
are shared across native targets because ADR-022 requires identical public APIs;
legitimate target divergence requires its existing explicit ADR exception.

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
