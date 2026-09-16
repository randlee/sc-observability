---
id: B.3
status: proposed
branch: feature/phase-b-3-schema
base: develop
---

# B.3 — Shared binding DTOs, schema and checked conversions

## Goal and dependencies

Deliver a usable neutral Rust DTO crate and versioned JSON contract over the
published Rust API. `must_follow` B.2; B.3a `must_follow` this accepted schema.
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
   conversions stay in B.3a/B.4 adapters; the neutral crate never imports bridge,
   Tauri, Specta exporters or PyO3 runtime dependencies.
2. Generate `bindings/schema/v1.json` and `bindings/schema/errors-v1.json` from
   the crate's Serde shapes. Add `bindings/conformance/v1/` with valid and invalid
   input/output examples and expected results. Add `bindings/API-COVERAGE.md`
   mapping each supported operation/type to its conversion fixture and explicit
   exclusions; B.3a and B.4 append their runtime evidence later.
3. Implement `scripts/ci/validate_binding_schema.sh` and a schema CI job for
   Rust conversion tests, schema/Serde agreement, regenerate-and-diff checks,
   and an external packaged-crate consumer. Record hashes, scoped DTO public-API
   approval and TYP-030 wire-only ownership exception in
   `docs/plans/phase-b/handoff-b-3.md`. Generation must use a locked toolchain
   and report drift without overwriting it into a passing result.

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
  size/depth/query limits, oversized diagnostics, maximum level revision and
  unsuccessful change diagnostics. Old/new schema compatibility rules are tested.
- AC4: Generated drift fails CI, crate API approval names the DTO crate, and no
  native ErrorContext/source/backtrace or ownership capability enters the wire.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_binding_schema.sh
cargo test --locked -p sc-observability-dto
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_docs_consistency.sh
```

The new validator runs all conformance cases, packages and compiles the external
consumer, checks schema generation deterministically and fails on skipped stages.
Negative fixtures assert the exact failure tag/code, not merely rejection.

## Paths to delete

None.

## Non-closure

No generated TypeScript package/exporter, Tauri host/IPC example (B.3a), Python
runtime (B.4), platform wheels (B.4a), or registry publication (B.7). This sprint
closes working neutral conversions and schema; it does not claim language
runtime behavior from schema-only tests.
