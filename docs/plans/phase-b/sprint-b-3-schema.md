---
id: B.3
status: complete
branch: feature/phase-b-3-schema
base: feature/phase-b-2-qualification
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-3-schema
---

# B.3 — Shared binding DTOs, schema and checked conversions

## Goal and dependencies

Deliver a usable neutral Rust DTO crate and versioned JSON contract over the
qualified staged Rust API. `must_follow` B.2; B.3b `must_follow` this accepted schema for shared runtime adapters;
B.3a/B.4 consume its generated language projections.
Shared public contracts and conformance artifacts prevent parallel_safe work.
Parent pushes trigger merge-forward before every child dev/fix round; parent PR
must merge before child completion. No language transport runtime is required
for this crate's production closure.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Create `crates/sc-observability-dto/` implementing every wire type and checked
   core conversion in [binding-contract.md](binding-contract.md), including the
   declarations below, decimal integers, paths, full health/event projections,
   runtime levels and diagnostic-preserving Failure mappings. Runtime-dependent
   conversions belong to B.3b shared backends; the neutral crate never imports bridge,
   Tauri or PyO3 runtime dependencies. The optional `schema-gen` feature enables
   Schemars only for tooling; default DTO builds retain types/serde/serde_json.
2. Create the isolated unpublished `bindings/schema-generator/` Cargo package
   and CLI `sc-observability-schema`, generating canonical
   `bindings/schema/v1.json` from Rust DTO Serde shapes with Schemars =1.2.2.
   Generate `bindings/schema/errors-v1.json` as a projection of its embedded
   error registry, never a second handwritten type authority. Add `bindings/conformance/v1/` with valid and invalid
   input/output examples and expected results. Add `bindings/API-COVERAGE.md`
   mapping each supported operation/type to its conversion fixture and explicit
   exclusions; B.3a and B.4 append their runtime evidence later.
3. Implement `scripts/ci/validate_binding_schema.sh` and a schema CI job for
   Rust conversion tests, schema/Serde agreement, regenerate-and-diff checks,
   and an external packaged-crate consumer. Implement reusable
   `scripts/ci/build_binding_source_bundle.py --root-manifest PATH --output DIR`
   for the prepublication source-bundle procedure specified in B.4a; B.3 owns
   this helper now so B.3b/B.3a/B.4 do not depend on future B.4a code. Inputs
   are a manifest path and fresh staging directory; output is manifest.json,
   checksummed archives, extracted unpublished roots, published vendor tree,
   staged root patches/config and a frozen layout-specific lock. Reject missing
   or escaping dependencies and stale locks. B.4a reuses it for sdists/matrix. Record hashes, scoped DTO public-API
   approval and TYP-030 wire-only ownership exception in
   `docs/plans/phase-b/handoff-b-3.md`. Generation must use a locked toolchain
   and report drift without overwriting it into a passing result. Implement
   repository-owned `scripts/generate_typescript_bindings.py` and
   `scripts/generate_python_bindings.py` as deterministic schema-only generators;
   their generated declarations/tagged data and conversion scaffolding are
   consumed by later runtime sprints, not alternate Rust exporters.

## Generation contract and locked tooling

The selected source chain is Rust DTO declarations/Serde annotations ->
Schemars -> `bindings/schema/v1.json` -> repository-owned TypeScript/Python
scripts. There is no Specta dependency/exporter and neither language generator
reads Rust source, errors-v1.json, or a handwritten parallel interface inventory.
Serde-conformance tests remain necessary: schema generation is not a substitute
for checked conversion behavior.

DTO manifest has optional `schemars = { version = "=1.2.2", optional = true }`
and `schema-gen = ["dep:schemars"]`; only wire DTOs derive JsonSchema behind
that feature. Core crates acquire no Schemars dependency. The generator is an
isolated workspace with committed `Cargo.lock`, DTO `schema-gen` enabled, and
Schemars exactly =1.2.2. It uses the repository's pinned Rust toolchain. This
Schemars release declares Rust 1.74 MSRV, below this repo's Rust 1.94.1 baseline;
lockfile and compile tests still gate actual compatibility. Schemars explicitly
uses Serde attributes, and its generated structure can change between versions,
which is why the exact pin is required. [Schemars 1.2.2 documentation](https://docs.rs/schemars/1.2.2/schemars/)

Use `SchemaSettings::draft2020_12().for_serialize()` for output definitions and
`.for_deserialize()` for input definitions, retaining separate `Input...` and
`Output...` names where required/default/unknown-field rules differ. Both sets
live in the single canonical file's `$defs`; `x-sc-entrypoints` maps each
operation/type to the correct local `$ref`. Never validate all operations through
an ambiguous generic root union. Every referenced type and concrete generic
Result/envelope instantiation is registered explicitly in the Rust generator.
[Schemars settings](https://docs.rs/schemars/1.2.2/schemars/generate/struct.SchemaSettings.html)

The same artifact embeds `x-sc-error-registry` from the Rust diagnostic registry
and `x-sc-bindings` hints for canonical integer domains, normalization/defaults,
public type names and operation mappings. These are emitted from Rust DTO/tooling
metadata and checked against conformance fixtures, not patched into generated
JSON. Semantic checks JSON Schema cannot express (for example since <= until)
remain the binding contract's checked converter behavior and generated tests.
Canonicalization sorts object keys, preserves array order, emits UTF-8 with LF
and one final newline, and excludes timestamps/absolute paths. All $refs are
local; duplicated names or unsupported schema keywords fail generation.

Concrete CLI surface (each tool supports `--check`, which compares temporary
output without rewriting committed files and exits nonzero on any drift):

```sh
cargo run --locked --manifest-path bindings/schema-generator/Cargo.toml --bin sc-observability-schema -- --output bindings/schema/v1.json --errors-output bindings/schema/errors-v1.json --check
python3 scripts/generate_typescript_bindings.py --schema bindings/schema/v1.json --output-dir bindings/typescript/src/generated --check
python3 scripts/generate_python_bindings.py --schema bindings/schema/v1.json --output-dir bindings/python/sc-observability-py/python/sc_observability/generated --check
```

B.3 owns both scripts and their golden outputs/tests. B.3a/B.4 import those
outputs and add runtime implementations. Each script uses Python 3.12.10 and
stdlib only; record that exact generation interpreter in
`bindings/generation-toolchain.toml`. This pin is independent of the supported
Python runtime matrix. CI verifies generator source revision, toolchain/lockfile
hashes, canonical schema hash and generated output hashes, then runs twice from
clean temporary directories and requires byte-identical outputs. An intentional
schema/tooling change updates reviewed artifacts explicitly; check mode cannot
regenerate its own expected baseline. Generated Python stubs and runtime tagged
data originate from the same script/schema; TypeScript declarations and runtime
validators likewise share one schema traversal.

The external DTO fixture uses its packaged .crate outside the checkout with
only its already-published core dependencies. It does not publish DTO to satisfy
B.3; subsequent unpublished dependent-crate fixtures follow B.4a's bundled-source
strategy. No registry-only DTO claim is made until B.7.

## Shared signatures

These TypeScript-shaped declarations specify JSON data, not a shipped client.
The binding contract supplies all remaining declarations and exact Rust
conversion signatures, including validation/resource limits. Rust types use
its explicit Serde rules; existing public core representations remain unchanged.

```ts
export type DispatchDto = { kind: "scheduled" };
export type AdmissionDto = { kind: "accepted" } | { kind: "filtered" };
export type CompletionDto = { kind: "completed" };
export type Result<T> =
  | { kind: "ok"; value: T }
  | { kind: "error"; error: Failure };
export type Failure =
  | (Diagnostic & { kind: "validation"; field: string })
  | (Diagnostic & { kind: "queue_full" })
  | (Diagnostic & { kind: "below_baseline"; requested: LevelFilterDto; configured: LevelFilterDto })
  | (Diagnostic & { kind: "unsupported_level"; requested: LevelFilterDto; available: LevelFilterDto })
  | (Diagnostic & { kind: "permission_denied" })
  | (Diagnostic & { kind: "closed" })
  | (Diagnostic & { kind: "unavailable" })
  | (Diagnostic & { kind: "io" })
  | (Diagnostic & { kind: "timeout"; operation: string })
  | (Diagnostic & { kind: "cancelled"; operation: string })
  | (Diagnostic & { kind: "unsupported_version"; received: number })
  | (Diagnostic & { kind: "internal" })
  | (Diagnostic & { kind: "unknown_remote"; remote_kind: string });
export interface Diagnostic {
  at: string; // original diagnostic timestamp, or boundary capture time for foreign failures
  code: string;
  message: string;
  remediation: RemediationDto;
}
export type RemediationDto =
  | { kind: "recoverable"; steps: string[] }
  | { kind: "not_recoverable"; justification: string };

export type LevelFilterDto = "off" | "error" | "warn" | "info" | "debug" | "trace";
export type LevelChangeSourceDto = "application" | "user_request" | "diagnostic_session";
export interface LevelStateDto {
  configured_level: LevelFilterDto;
  effective_level: LevelFilterDto;
  level_revision: string; // canonical unsigned u64 decimal
}
export interface DiagnosticSummaryDto {
  code: string | null;
  message: string;
  at: string; // canonical UTC RFC3339
}
export interface OperationDiagnosticDto extends Diagnostic {}
export type ChangeDiagnosticDto =
  | { kind: "accepted" }
  | { kind: "not_accepted"; diagnostic: OperationDiagnosticDto };
export type LevelChangeDto =
  | { kind: "changed"; previous: LevelStateDto; current: LevelStateDto;
      source: LevelChangeSourceDto; diagnostic: ChangeDiagnosticDto }
  | { kind: "unchanged"; state: LevelStateDto };
export type LevelRequestDto =
  | { kind: "elevate"; level: LevelFilterDto }
  | { kind: "reset" };
```

## Acceptance criteria (authoritative)

- AC1: A consumer outside the checkout can build the packaged DTO crate and
  execute checked event/query/health/diagnostic conversions using public APIs.
  No language runtime dependency enters the crate or existing core graph.
- AC2: Schema, Rust Serde output and conformance expectations agree for every
  Result/Failure/remediation/level variant, accepted/filtered outcomes, complete
  stored-event/health fields and original diagnostic data. Unknown codes remain
  codes; unknown remote variants, invalid tags/versions and malformed envelopes
  follow the contract rather than being coerced to success.
- AC3: Fixtures cover min/max/overflow signed and unsigned integers, decimal
  canonicalization, finite/nonfinite floats, null/missing/unknown fields, UTC
  timestamps and equal/inclusive query bounds, unrepresentable paths, request
  size/depth/query limits, every protected-provenance spoofing case from the
  binding contract, oversized diagnostics, maximum level revision and
  unsuccessful change diagnostics. Old/new schema compatibility rules are tested.
- AC4: Both schema-only language generators reproduce complete declarations,
  unions/defaults/integer mappings from the canonical file with no Rust parser,
  Specta or independent language schema. Input/output Serde differences and
  unsupported-keyword failures have fixtures; two clean runs match byte-for-byte.
- AC5: The actual `scripts/ci/build_binding_source_bundle.py` helper produces
  a self-contained bundle for an external DTO consumer. From a fresh directory
  outside every checkout, with fresh CARGO_HOME/CARGO_TARGET_DIR and network and
  checkout access disabled, `cargo metadata --locked --offline` and
  `cargo run --locked --offline` resolve only bundled dependencies and execute
  the public conversion fixture. Missing bundle members, a stale frozen lock
  and escaping manifest/archive paths each fail explicitly; cache or checkout
  fallback cannot satisfy this gate.
- AC6: Generated drift fails CI, crate API approval names the DTO crate, and no
  native ErrorContext/source/backtrace or ownership capability enters the wire.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_binding_schema.sh
cargo test --locked -p sc-observability-dto
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_docs_consistency.sh
```

The new validator runs all conformance cases and checks schema generation
and both language generators deterministically. Its external-consumer stage
calls `python3 scripts/ci/build_binding_source_bundle.py --root-manifest
crates/sc-observability-dto/Cargo.toml --output "$BINDING_BUNDLE_DIR"` against the
actual DTO package. It creates a minimal conversion consumer within that artifact
with versioned dependencies and the helper's artifact-local root patches/source
replacement. It transfers only that self-contained artifact to a fresh
external directory, provisions the pinned toolchain before isolation, and runs
`cargo metadata --locked --offline` followed by `cargo run --locked --offline`
with fresh CARGO_HOME/CARGO_TARGET_DIR, no network and no checkout access. Assert
metadata manifest paths are confined to the artifact; record helper command,
bundle hashes, dependency provenance and fixture output in the B.3 handoff.

Run separate negative copies with a missing unpublished bundle member, stale
Cargo.lock and an escaping path (manifest dependency or archive entry). Each
must fail validation/build before any external read or fallback resolution.
These exercise the actual helper/output, not a mocked packaging path. The
validator fails if any positive or negative stage is skipped; B.4a later reuses
this same helper with its expanded Python/runtime dependency closure.
Negative fixtures assert the exact failure tag/code, not merely rejection.

## Paths to delete

None.

## Non-closure

No TypeScript runtime package, Tauri host/IPC example (B.3a), Python
runtime (B.4), platform wheels (B.4a), or registry publication (B.7). This sprint
closes working neutral conversions and schema; it does not claim language
runtime behavior from schema-only tests.

## Implementation handoff

The complete implementation, scoped lead API approval, generation hashes and
actual isolated prepublication package proof are recorded in
[handoff-b-3.md](handoff-b-3.md). Status denotes implementation completion;
consolidated Phase B QA, ordered merges and B.7 publication remain separate.
