---
id: A.5
title: Log Facade Bridge Migration And Adapter Foundations
status: planned
branch: plan/phase-a-log-facade-bridge
target: integrate/phase-a
---

# Sprint A.5 — Log Facade Bridge Migration And Adapter Foundations

```yaml
plan_type: sprint_plan
phase: A
sprint: A.5
status: planned
estimated_scope: extra_large
```

## Goal

Migrate the generic BTIT `sc-observability-log` facade bridge and macros into
this workspace as published companion crates, while correcting its lifecycle,
panic containment, key-value, identity, and health contracts. Establish a
versioned adapter boundary that can later support BTIT TypeScript and Python
frontends without coupling either language runtime to the core logger.

This is a proposed follow-on workstream. It does not revise the accepted A.1–A.4
Phase-A closure record until its API lock and implementation have been accepted.

## Hard Dependencies

- [`log-facade-bridge-design.md`](./log-facade-bridge-design.md)
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`
- `docs/public-api-checklist.md`
- BTIT baseline: `randlee/beads-task-issue-tracker@f6f69dc`

## Scope Boundary

Copy only the generic source baseline:

```text
BTIT crates/sc-observability-log/
BTIT crates/sc-observability-log-macros/
BTIT crates/sc-observability-log-consumer-check/
```

Do not copy `src-tauri`, frontend components, Tauri commands, BTIT app paths,
or BTIT deletion/permission policy. The core `sc-observability` and
`sc-observability-types` crates remain independent of `log`, proc macros,
Tauri, PyO3, and maturin.

## Staged Plan

### A5.0 — Contract and API lock

Freeze A5-D01 through A5-D06 in the design record and normative docs. Define
the guard/control lifecycle state machine, timeout semantics, `BridgeHealthReport`,
identity ownership, `log::kv` key/collision rules, global-install rule, and
supported facade/macro compatibility surface. Add every proposed public type,
error, and semver decision to `public-api-checklist.md` before implementation.

Exit: no unresolved behavior ambiguity remains for A5.1–A5.5.

### A5.1 — Traceable baseline import

Create and wire:

```text
crates/sc-observability-log/
crates/sc-observability-log-macros/
crates/sc-observability-log-consumer-check/   # CI-only; publish = false
```

Copy the generic source, rustdoc, integration fixtures, compile-fail UI tests,
and consumer compile-check from the pinned BTIT baseline. Add workspace
dependencies for `log`, macro tooling (`syn`, `quote`, `proc-macro2`), and
test tooling (`trybuild`); retain the exact lockstep macro-version pin because
macro expansions address the bridge's hidden support module.

Exit: baseline behavior is reproducible and every copied file has an explicit
source/variation record.

### A5.2 — Correct bridge internals

Implement the one-outer-guard emission path, a private unguarded submit core,
the `LogGuard`/`LogControl` ownership split, idempotent serialized shutdown,
typed timeout outcomes, `BridgeHealthReport`, the identity decision, and the
unified field-key policy. Keep the compatibility facade non-blocking on its
emit path and keep `log::Log::flush` bounded/no-op according to the locked
contract.

Exit: all A5-D01 through A5-D05 implementation changes have focused tests.

### A5.3 — Compatibility and failure validation

Add subprocess tests for process-global install behavior; enabled-level tests
for panicking and recursive formatters; raw key-value policy/collision tests;
bounded flush/shutdown and post-timeout state tests; health-transition tests;
JSONL and tracing-compatible macro fixtures; UI compile-fail coverage; and an
external consumer compilation test depending only on `sc-observability-log`.

Exit: bridge claims are proven in isolation rather than inferred from BTIT.

### A5.4 — BTIT proving migration

Replace BTIT's local bridge crates with the migrated package (initially a
pinned workspace/path dependency, later a released version). BTIT owns one
`LogGuard`; clear/export/health operations use a read-only control handle.
Re-run its clear-during-exit, log-clear, frontend, and manual-quit evidence.

Exit: BTIT proves the generic Rust contract without importing its app policy
into the shared crates.

### A5.5 — Adapter contract and TypeScript proof

First prove the DTO boundary in BTIT. If two consumers need the same operations,
extract optional `sc-observability-tauri` with feature-gated Tauri and Specta
support. Generate/version TypeScript DTOs for event submission, query,
health, and typed errors; check their output in CI. Application command names,
permissions, log clearing, and UI remain outside the generic adapter.

Exit: a TypeScript frontend can consume stable observability DTOs without
access to Rust lifecycle ownership or mutable logger state.

### A5.6 — Python feasibility and binding implementation

After A5.2–A5.3 stabilize, implement a separate PyO3/maturin package. Publish
a Python-owned object and typed DTO/error surface; release the GIL around
blocking calls; exclude callback sinks/redactors and implicit facade install.
Build/test wheels across approved platforms and Python versions.

Exit: a Python frontend can consume the same stable observability concepts
without sharing Tauri-specific APIs or the Rust `log` global.

### A5.7 — Release gate

Run formatter, full workspace tests, strict Clippy, public API/semver gates,
docs consistency, consumer compile checks, generated-binding drift checks, and
macOS/Linux/Windows CI. Publish macros and bridge in lockstep, then publish
adapter packages only after their DTO compatibility policy is accepted.

## Exact Targets

- workspace `Cargo.toml` / `Cargo.lock`
- `crates/sc-observability-log/**`
- `crates/sc-observability-log-macros/**`
- `crates/sc-observability-log-consumer-check/**`
- later only: `crates/sc-observability-tauri/**`
- later only: `bindings/python/sc-observability-py/**`
- `docs/phase-A/log-facade-bridge-design.md`
- `docs/public-api-checklist.md`, `docs/api-design.md`, and `docs/migration-guide.md`
- BTIT migration documentation and its a5 review evidence

## Acceptance Criteria

- the BTIT baseline is copied traceably and corrected before release;
- no enabled facade record can unwind through caller code or bypass reentrancy
  accounting;
- exactly one final shutdown is owned and observed;
- bridge health is a stable serializable projection;
- facade and macro field-key behavior is identical and documented;
- core crates have no Tauri, Specta, PyO3, maturin, or `log` dependency;
- generated TypeScript and Python surfaces use versioned DTOs, stable error
  codes, and no raw Rust ownership handles;
- BTIT proves the Rust bridge before generic UI extraction;
- every published crate passes semver/API and cross-platform release gates.

## Non-Closure

- A5 does not move BTIT's Tauri command authorization, filesystem deletion,
  retention policy, or Vue rendering into this workspace.
- A5 does not install a Rust global logger from Python automatically.
- A5 does not permit Python callback sinks/redactors in its initial release.
- A5 does not add a Tauri or language-binding dependency to core crates.

