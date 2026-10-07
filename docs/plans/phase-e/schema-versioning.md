# Phase E contract versioning

User-authorized addition: retain versioned public contracts and detect changes through fast unit tests. Public API comparison runs after the ordinary platform unit build in the existing macOS, Windows and Linux jobs; it creates no separate workflow or job. This is separate from the six integration suites and adds no integration cells. Sc-lint is still unspecified.

> Historical Rust-draft note: the accepted-history, `SC_API_ACCEPTED_BASE`,
> and no-rustdoc requirements in the retained Rust draft text below were
> superseded by the stock `cargo public-api` design in
> [PR #1013](https://github.com/randlee/sc-observability/pull/1013)
> (`ebde67eb`). The authoritative replacement is the Rust section, “API unit
> comparison and release-cut setup.” This pointer changes no separate
> Python/TypeScript, binding, configuration, or storage history rule.

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

Rust uses the pinned stock `cargo public-api` tool, not a compiler-metadata
reader or a project-defined row format. `setup` is the only build-producing
candidate step: it generates current rustdoc JSON for every `publish = true`
Rust crate named in `release/publish-artifacts.toml`, including
the standalone Tauri crate, and writes the tool's unmodified `-sss` text only
under its caller-owned target directory. It never writes the committed baseline.

```sh
python3 scripts/ci/stock_public_api.py setup --target-dir target/e-api-public-api
```

The ordinary follow-up check requires each current rustdoc JSON file and invokes
the stock tool with `--rustdoc-json`. It has no package or manifest argument and
does not run a Cargo build. Missing JSON or a missing committed native baseline
fails clearly when a committed stock-text line is removed or changed. Pure
additions pass the ordinary unit check; release-cut records them in a new
versioned baseline only after intentional release review.

`release-cut` is the separate, explicit command that refreshes a committed
versioned baseline after intentional release review; ordinary CI setup never
does so.

```sh
python3 scripts/ci/stock_public_api.py release-cut --target-dir target/e-api-public-api
```

```sh
python3 scripts/ci/stock_public_api.py check --target-dir target/e-api-public-api
```

Text equality alone cannot classify an addition as consumer-compatible. The
existing compatibility gate remains the semantic authority and now asks the
stock tool to deny only changed or removed public items:
`cargo public-api diff --deny changed --deny removed <published-version>`.
Therefore compatible additions pass the release compatibility gate; removals and
signature changes fail. No mapping from native text to another schema is used.

The existing `test` and `windows-test` CI jobs run capture, their ordinary Cargo
tests, then the zero-build check. They are normal PR jobs, not a new pipeline.
Local setup time is reported independently from comparison time; Windows and
Linux job timings are native CI evidence. Python and TypeScript keep their
existing independent history/producer checks and are not claimed by this Rust
check.

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
