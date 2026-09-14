---
id: B.3
status: proposed
branch: feature/phase-b-3-typescript
base: develop
---

# B.3 — TypeScript bindings for the public logging API

## Goal and dependencies

Deliver a working Tauri-backed TypeScript logging client, with generated types
and explicit runtime validation. `must_follow` B.2 for the released Rust API.
B.4 `must_follow` B.3 because B.3 owns the shared DTO/schema/error contract.
The first user-selected runtime is Tauri; standalone Node.js requires a distinct
transport and is outside this sprint.

## Deliverables (authoritative)

1. Create `crates/sc-observability-dto/` containing checked conversions between
   wire DTOs and public core types, plus `bindings/schema/v1.json`,
   `bindings/schema/errors-v1.json`, and `bindings/conformance/v1/` fixtures.
   Add `bindings/API-COVERAGE.md` mapping each supported public operation to
   its DTO and runtime test, with explicit exclusions. Runtime-specific tools
   never enter this crate or the core dependency graph.
2. Create `bindings/typescript/` with a locked package/toolchain, a Rust exporter
   under `bindings/typescript/exporter/`, generated declarations, and a typed
   Promise-based client. Pin a compatible Specta/serde exporter combination
   there; validate the emitted JSON representation, not Rust type names alone.
   Export a transport interface and a Tauri invoke implementation. Publishable
   package name proposed: `@sc-observability/client`; availability/ownership is
   checked in B.6, not assumed here.
3. Create `bindings/tauri/` and `examples/tauri-logging/` as isolated adapter and
   consumer workspaces. A host installs command handlers over its existing
   logger/control API; the frontend cannot create or shut down the host logger.
   Host command registration, permissions, log root, targets, redaction and
   size policy are explicit. The example executes real Rust logging/query/
   health/flush through IPC; it is not a mock-only demonstration.
4. Create `scripts/ci/validate_typescript_bindings.sh` and a binding CI job that
   regenerate-and-diff schema/declarations, type-check, test package installation,
   exercise the real Tauri command boundary on supported desktop platforms,
   and run all conversion/error fixtures. Write usage and migration documentation
   plus `docs/plans/phase-b/handoff-b-3.md`. Document the DTO ownership exception
   to TYP-030: these are wire projections; core runtime health/errors stay owned
   by their existing crates. Add scoped public-API approval for the DTO package.

## Contract and operation signatures

The transport carries JSON values only. Frontend API (all methods reject with
an `ObservabilityError` exposing `code`, `message`, and `remediation` on failure):

```ts
export interface ObservabilityClient {
  tryLog(event: LogEventDto): Promise<AdmissionDto>;
  log(event: LogEventDto): Promise<AdmissionDto>;
  query(query: LogQueryDto): Promise<LogSnapshotDto>;
  health(): Promise<LogHealthDto>;
  flush(timeoutMs: number): Promise<CompletionDto>;
}
export type AdmissionDto = { status: "accepted" };
export type CompletionDto = { status: "completed" };
export type RemediationDto =
  | { kind: "retry"; after_ms: number | null }
  | { kind: "action"; steps: string[] }
  | { kind: "none" };
export interface ObservabilityErrorDto {
  schema_version: 1;
  code: string;
  message: string;
  remediation: RemediationDto;
}
export interface JsonTransport {
  request(operation: "try_log" | "log" | "query" | "health" | "flush",
          request: unknown): Promise<unknown>;
}
```

Admission means accepted by the Rust API, not persisted; level filtering follows
Rust behavior. `tryLog` reports queue-full immediately as a stable error;
`log` may wait for admission, so the host runs blocking work off the UI/async
executor thread. Flush timeout means the caller stopped waiting. Use the
accepted control API if available; otherwise a bounded, coalesced helper owns
the in-flight operation. Repeated timeouts must not create unbounded threads.

The published schema is authoritative for these value shapes:

| DTO | Fields and semantics |
| --- | --- |
| LogEventDto | `schema_version: 1`, `level` (trace/debug/info/warn/error), validated `target`, `action`, nullable `message`, nullable request_id/correlation_id/trace context, `fields` object; host supplies service, timestamp, identity and source provenance |
| LogQueryDto | `schema_version: 1`, nullable service/target/action/request_id/correlation_id/since/until filters, levels array, field_matches array, integer limit 1..1000 (default 100), order oldest_first/newest_first mapped to core ordering |
| LogSnapshotDto | `schema_version: 1`, full stored event projections and `truncated`; bounded snapshot, no continuation cursor or pagination promise |
| Stored event projection | Every public `LogEvent` field, including schema version, timestamp, service, identity, trace and outcome; explicit conversion inventory in API-COVERAGE |
| LogHealthDto | `schema_version: 1`, full public logging health projection, nullable bridge health projection if host uses the bridge; no mutable ownership handle |

Frontend and backend events use the same host-owned writer and health state.
The example logs one correlated operation from TypeScript and Rust, proving both
records are queryable together. Request/correlation/trace identifiers are validated
as diagnostic context; frontend claims never grant authority. The host annotates
source language/channel using protected metadata that frontend fields cannot
overwrite. Redaction and retention are applied by the same host policy.

All wire property names use snake_case. Missing optional input fields normalize
to null; output nullable fields are present. Unknown schema versions and unknown
input fields produce stable validation errors. Additive optional output fields
are accepted; changed required fields/tags/meaning require a new schema version.
Version DTOs independently of crate/package releases. Treat error codes as stable
identifiers; never parse display strings to reconstruct them.

All i64/u64 counters and values outside JavaScript's safe integer range use
canonical decimal strings in typed schema positions. Arbitrary event fields use
a recursively tagged value representation for integers: `{kind:"integer",
value:"18446744073709551615"}`; other values are explicitly tagged null, boolean,
string, finite float, array, or object, so user objects cannot collide with a
magic integer key. Public wrappers convert ergonomic inputs into this wire form.
Dates are canonical UTC RFC3339; query bounds retain core inclusive semantics.
Paths project to nullable UTF-8 text plus an explicit encoding-error flag for
unrepresentable paths; they are diagnostic output, never file-operation authority.

The host enforces a maximum serialized request size of 64 KiB and depth 32,
configured target allowlist, required redaction, bounded query limit and stable
error conversion before calling core. Frontend input cannot relax those policies.
These are adapter defaults, not changes to Rust core limits. Queries may return
pre-existing unredacted history; access remains host-authorized.

## Acceptance criteria (authoritative)

- AC1: A clean TypeScript consumer installs the packed package and uses all five
  operations through real Tauri IPC with expected JSONL/query/health results,
  including correlated frontend/Rust backend records in one application log.
- AC2: Rust JSON, generated declarations, runtime validators and fixture results
  agree, including max u64, negative large integers, nulls, UTC, invalid paths,
  same-timestamp query results, invalid versions and every error variant.
- AC3: Input policy cannot be bypassed by direct invoke calls; denied targets,
  oversized/deep payloads, invalid fields, redaction, queue-full and flush-timeout
  cases have boundary tests. The host stays responsive during blocked I/O.
- AC4: Generated drift fails CI; core dependency/API invariants remain intact.
  Packed TypeScript and packaged Rust adapter artifacts work outside the repo.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_typescript_bindings.sh
cargo test --locked -p sc-observability-dto
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_docs_consistency.sh
```

The new script owns exact locked exporter/package-manager commands, temporary
package-consumer installation, and Rust/IPC integration tests, and fails if any
stage is skipped. Record macOS/Linux/Windows results and the generated artifact
hashes. It must not overwrite drift and then report success.

## Paths to delete

None.

## Non-closure

No registry publication (B.6), actual BTIT migration, Node.js addon, Python, Go,
follow stream, custom callback sink/redactor, OTLP, log deletion, or frontend
lifecycle ownership. Preserve the narrow public logging subset explicitly.

## Technical references

[Tauri command boundary](https://v2.tauri.app/develop/calling-rust/) informs the
host/client split. [Specta integer export policy](https://docs.rs/specta/latest/specta/ts/enum.BigIntExportBehavior.html)
requires a wire encoding that agrees with generated types; merely exporting
`bigint` is insufficient for JSON.
