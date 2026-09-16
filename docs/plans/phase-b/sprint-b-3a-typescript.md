---
id: B.3a
status: proposed
branch: feature/phase-b-3a-typescript
base: develop
---

# B.3a — TypeScript bindings for the public logging API

## Goal and dependencies

Deliver a working Tauri-backed TypeScript logging client, with generated types
and explicit runtime validation. `must_follow` B.3 for its completed DTO/schema/error contract.
B.4 `must_follow` B.3a for the proven cross-language transport/conformance baseline.
Shared schema consumers and fixtures preclude parallel_safe execution. Parent
pushes trigger merge-forward before each child dev/fix round; parent PR merges first.
The first user-selected runtime is Tauri; standalone Node.js requires a distinct
transport and is outside this sprint.

The result-returning direct-submit/control/health contract is owned and accepted
by sc-observability before B.1, then implemented/reviewed in BTIT and copied.
Use [the accepted target](target-bridge-api.md); this sprint projects that public
API into the binding contract without a hidden-module call or a second writer.
The binding proposals below are reviewed against the accepted target revision.

The complete new DTO declarations, conversion boundaries, validation defaults
and error mapping are incorporated from [the binding contract](binding-contract.md).
They are part of this sprint's reviewable contract, not future design work.

## Deliverables (authoritative)

1. Create `bindings/typescript/` with a locked package/toolchain, a Rust exporter
   under `bindings/typescript/exporter/`, generated declarations, and a typed
   client with nonblocking logging and discriminated results for every operation. Pin a compatible Specta/serde exporter combination
   there; validate the emitted JSON representation, not Rust type names alone.
   Export the exact encodeValue/encodeEvent helpers in the
   [binding contract](binding-contract.md#typescript-ergonomic-conversion-api),
   a transport interface and a Tauri invoke implementation. Publishable
   package name proposed: `@sc-observability/client`; availability/ownership is
   checked in B.7, not assumed here.
2. Create `bindings/tauri/` and `examples/tauri-logging/` as isolated adapter and
   consumer workspaces. A host installs command handlers over its existing
   logger/control API; the frontend cannot create or shut down the host logger.
   Host command registration, permissions, log root, targets, redaction and
   size policy are explicit. The example executes real Rust logging/query/
   health/flush through IPC; it is not a mock-only demonstration. Add the
   application-owned level-request handler specified below, retaining LogGuard
   authority in Rust and returning ordinary tagged envelopes.
3. Create `scripts/ci/validate_typescript_bindings.sh` and a binding CI job that
   consume and verify B.3 schema fixtures, regenerate-and-diff declarations, type-check, test package installation,
   exercise the real Tauri command boundary on supported desktop platforms,
   and run all conversion/error fixtures. Write usage and migration documentation
   plus `docs/plans/phase-b/handoff-b-3a.md`. Extend B.3 API-COVERAGE with
   concrete client/IPC runtime test references. DTO definitions, schema generation
   and crate API approval remain B.3 artifacts; this sprint consumes them.

## Contract and operation signatures

The transport carries JSON values only. Every public factory, validator and
operation returns Result; async operations resolve Result and never reject for
operational errors. Do not throw then catch the library's own expected failures.
Convert foreign transport/formatter errors into Failure at the boundary. Error
objects are data, not Error subclasses. No convenience unwrap-or-throw API ships.

```ts
// Result, Failure and DTOs are generated from B.3; see binding-contract.md.
export interface ObservabilityClient {
  log(event: LogEventDto): Result<DispatchDto>;
  tryLog(event: LogEventDto): Promise<Result<AdmissionDto>>;
  query(query: LogQueryDto): Promise<Result<LogSnapshotDto>>;
  health(): Promise<Result<LogHealthDto>>;
  client_status(): Result<ClientStatus>;
  flush(timeoutMs: number): Promise<Result<CompletionDto>>;
}
export type ClientOutcome =
  | { kind: "idle" }
  | { kind: "scheduled"; operation: "log" }
  | { kind: "accepted"; operation: "log" | "try_log" }
  | { kind: "filtered"; operation: "log" | "try_log" }
  | { kind: "completed"; operation: "query" | "health" | "flush" };
export interface ClientStatus {
  in_flight: number; // integer 0..256
  failures_by_kind: Record<Failure["kind"], string>; // saturating u64 decimal
  last_result: Result<ClientOutcome>;
  last_failure: Failure | null;
}
export interface JsonTransport {
  request(operation: "try_log" | "query" | "health" | "flush",
          request: unknown): Promise<Result<unknown>>;
}
export declare function createClient(transport: JsonTransport): Result<ObservabilityClient>;
```

Shared level values are generated from B.3 and detailed in the binding contract.
The application-level helper has this signature:

```ts
// Example application API, not a method on ObservabilityClient/LogControl:
export declare function requestLevelChange(request: LevelRequestDto): Promise<Result<LevelChangeDto>>;
```

The example registers `sc_observability_try_log`, `sc_observability_query`,
`sc_observability_health`, and `sc_observability_flush`. Each takes one argument
`request` carrying schema_version 1 plus respectively `event`, `query`, no
operation fields, or `timeout_ms`, and returns WireEnvelope of AdmissionDto,
LogSnapshotDto, LogHealthDto, or CompletionDto. JsonTransport maps its operation
names to those exact commands. `log` schedules the same try_log command.

The example-only `app_observability_level_change` takes `request` containing
schema_version 1 and LevelRequestDto under `change`, and returns
WireEnvelope<LevelChangeDto>. The registered main application window is the only
authorized caller; direct calls from other windows return permission_denied.
The host supplies source=user_request (the caller cannot forge a source), uses
one mutex over its LogGuard owner for level/shutdown commands, and executes the
short mutation on its bounded host worker. No lock is held while waiting on sink
I/O. Unknown fields, bad tags and malformed levels return validation results.
Stopping/Stopped map to closed, BelowBaseline/UnsupportedLevel preserve their
payloads, Unavailable maps to unavailable; diagnostic failure remains an ok
changed value with not_accepted. The sample helper uses the same transport error
containment as the library and never grants frontend ownership. Its fixed window
policy is an example default; production authorization remains application-owned.


Tauri command wrappers return an ordinary serializable WireEnvelope<T>, with
schema_version and a flattened tagged WireResult<T>. They do not expose a native
Result<T, E> command return that Tauri routes into Promise rejection. Convert
native Rust Result into this value at the command boundary:

```rust
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireResult<T> {
    Ok { value: T },
    Error { error: Failure },
}
```

Transport-level invoke failures are still converted by the client boundary;
expected application failures travel as resolved error envelopes end-to-end.

Result envelopes carry `schema_version: 1` at the wire boundary; generated
language Result wrappers project that envelope without losing its discriminator.
The error registry fixes each code's Failure variant and remediation mapping.
A valid but unknown remote failure tag maps to unknown_remote with its original
kind/code/message retained in bounded diagnostic data; malformed envelopes produce
a validation result. Unknown codes within a known variant remain available in
code; callers must not parse messages. Native remediation is projected faithfully:
recoverable actions do not imply an automatic retry or a retry delay. Invalid union tags become a
validation result. Serialize only the active variant's fields: never use a
success flag plus nullable value/error combinations that permit invalid states.

Normal `log` validates and schedules the nonblocking host try_log request and
returns `ok/scheduled` or an immediate error; scheduled does not mean accepted
by the host. Later transport/admission errors update bounded client health, with
no unhandled Promise rejection. client_status() reads that local status without
IPC, including after host disconnection. Initially all failure counters/in_flight
are zero, last_result is ok/idle and last_failure is null. Each completed
operation replaces last_result with its payload-free ClientOutcome or Failure;
a failure also increments its fixed kind counter and retains last_failure.
Dispatch starts as scheduled and later becomes accepted/filtered/error; this is
completion order, not a per-call receipt. Successful later calls do not erase
last_failure. The status accessor itself does not change these counters/status
or recurse; unavailable local state returns internal. Callers wanting host confirmation use tryLog and
inspect its Result. AdmissionDto preserves Accepted versus Filtered: filtered means valid but
excluded by the effective threshold, accepted means queued. Neither means
writing or persistence. The API explicitly distinguishes local dispatch
from host acknowledgement, and never upgrades dispatch into a persistence claim.

All error handling is nonrecursive. If best-effort health accounting fails,
preserve the original result and do not attempt another log/fallback sink.
No retry queue may grow without bound. The host runs blocking query/flush work
off the UI/async executor thread. Flush timeout is an error result indicating
that the caller stopped waiting; it does not stop or retry the underlying flush.
Use the accepted control API with one in-flight flush per logger. Overlapping
requests return queue_full/SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS; a later request never shares an
earlier flush barrier. The slot remains owned until actual completion, including
after caller timeout. Repeated timeouts cannot create unbounded threads.

The published schema is authoritative for these value shapes:

| DTO | Fields and semantics |
| --- | --- |
| LogEventDto | `schema_version: 1`, `level` (trace/debug/info/warn/error), validated `target`, `action`, nullable `message`, nullable request_id/correlation_id/trace context, `fields` object; host supplies service, timestamp, identity and source provenance |
| LogQueryDto | `schema_version: 1`, nullable service/target/action/request_id/correlation_id/since/until filters, levels array, field_matches array, integer limit 1..1000 (default 100), order oldest_first/newest_first mapped to core ordering |
| LogSnapshotDto | `schema_version: 1`, full stored event projections and `truncated`; bounded snapshot, no continuation cursor or pagination promise |
| Stored event projection | Every public `LogEvent` field, including schema version, timestamp, service, identity, trace and outcome; explicit conversion inventory in API-COVERAGE |
| LogHealthDto | `schema_version: 1`, full public logging health projection, nullable bridge health projection if host uses the bridge; required `level_state: LevelStateDto`, no mutable ownership handle |

Frontend and backend events use the same host-owned writer and health state.
The example logs one correlated operation from TypeScript and Rust, proving both
records are queryable together. Request/correlation/trace identifiers are validated
as diagnostic context; frontend claims never grant authority. The host annotates
source language/channel using protected metadata that frontend fields cannot
overwrite. Redaction and retention are applied by the same host policy.

All wire property names use snake_case. Missing optional input fields normalize
to null; output nullable fields are present. Unknown schema versions produce unsupported_version; unknown
input fields produce stable validation errors. Additive optional output fields
are accepted; changed required fields/tags/meaning require a new schema version.
Adding or changing a Result/Failure/Remediation variant can break exhaustive
callers: require a new DTO schema and an appropriate breaking package version,
with old/new consumer fixtures. The predeclared unknown_remote variant handles
forward-compatibility explicitly. Version DTOs independently of crate/package
releases. Treat error codes as stable
identifiers; never parse display strings to reconstruct them.

All i64/u64 counters and values outside JavaScript's safe integer range use
canonical decimal strings in typed schema positions. Arbitrary event fields use
a recursively tagged value representation for integers: `{kind:"integer",
value:"18446744073709551615"}`; other values are explicitly tagged null, boolean,
string, finite float, array, or object, so user objects cannot collide with a
magic integer key. Public wrappers convert ergonomic inputs into this wire form.
Dates are canonical UTC RFC3339; query bounds retain core inclusive semantics.
Paths project to a tagged union: `{kind:"utf8", value:string}`,
`{kind:"unrepresentable"}`, or `{kind:"absent"}`; they are diagnostic output, never file-operation authority.

The host enforces a maximum serialized request size of 64 KiB and depth 32,
configured target allowlist, required redaction, bounded query limit and stable
error conversion before calling core. Frontend input cannot relax those policies.
These are adapter defaults, not changes to Rust core limits. Queries may return
pre-existing unredacted history; access remains host-authorized.

## Acceptance criteria (authoritative)

- AC1: A clean TypeScript consumer installs the packed package and uses all five
  remote operations through real Tauri IPC with expected JSONL/query/health results,
  including correlated frontend/Rust backend records in one application log.
- AC2: Rust JSON, generated declarations, runtime validators and fixture results
  agree, including every Result/Failure variant, exhaustive TypeScript narrowing,
  max u64, negative large integers, nulls, UTC, invalid paths,
  same-timestamp query results, invalid versions, every error/remediation variant,
  unknown remote failures, and old/new schema consumer compatibility.
- AC3: Input policy cannot be bypassed by direct invoke calls; denied targets,
  oversized/deep payloads, invalid fields, redaction, queue-full and flush-timeout
  cases have boundary tests. Default log failures, including a disconnected host
  and failed diagnostic accounting, do not throw or trigger unhandled Promise
  rejections. Factory, validation, query, health and lifecycle errors also return
  Result rather than throwing/rejecting; tests fail on a hidden exception path.
  The host stays responsive during blocked I/O. After disconnected-host and
  delayed admission failures, client_status exposes the retained local error
  without IPC, bounded fixed-kind counts and correct in_flight recovery; remote
  health cannot substitute for this local evidence.
- AC4: Baseline/elevation/reduce/reset/repeat/Off scenarios produce coherent
  core/bridge/frontend health; maximum revision survives round-trip. Unauthorized
  window/direct invoke and malformed requests return tagged failures. Queue-full
  change diagnostics preserve successful change outcomes; capped/lifecycle
  mutation failures preserve prior state. Admission accepted/filtered fixtures
  and diagnostic message and remediation steps round-trip without information loss.
- AC5: Generated drift fails CI; core dependency/API invariants remain intact.
  Packed TypeScript and packaged Rust adapter artifacts work outside the repo.
  The packed consumer exercises ergonomic integer conversion through real
  logging/query; invalid/cyclic/getter-failing inputs return typed errors.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_typescript_bindings.sh
bash scripts/ci/validate_binding_schema.sh
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_docs_consistency.sh
```

The new script checks authored TypeScript/Rust adapter paths for throw/panic/
unwrap-based operational control flow, runs fault injection on every public
Result-returning path, and compiles exhaustive Result/Failure narrowing fixtures.
Foreign transport failures must be converted without escaping. Include real
level-command authorization, owner/shutdown races, health coherence,
accepted/filtered distinction and every level-result/error fixture; a mock-only
endpoint test does not satisfy these checks.

The new script owns exact locked exporter/package-manager commands, temporary
package-consumer installation, and Rust/IPC integration tests, and fails if any
stage is skipped. Record macOS/Linux/Windows results and the generated artifact
hashes. It must not overwrite drift and then report success.

## Paths to delete

None.

## Non-closure

No DTO/schema redesign (B.3 owns the accepted definitions), registry publication (B.7), actual BTIT migration, Node.js addon, Python, Go,
follow stream, custom callback sink/redactor, OTLP, log deletion, or frontend
lifecycle ownership. Preserve the narrow public logging subset explicitly.

## Technical references

[Tauri command boundary](https://v2.tauri.app/develop/calling-rust/) informs the
host/client split. [Specta integer export policy](https://docs.rs/specta/latest/specta/ts/enum.BigIntExportBehavior.html)
requires a wire encoding that agrees with generated types; merely exporting
`bigint` is insufficient for JSON.
