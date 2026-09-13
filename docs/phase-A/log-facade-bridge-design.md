---
id: A.5-design
title: Log Facade Bridge Design Review And Language-Adapter Direction
status: proposed
reviewed_source: randlee/beads-task-issue-tracker@f6f69dc
reviewed_at: 2026-09-13
reviewers:
  - Codex
  - aobs
---

# Log Facade Bridge Design Review

## Decision

Add a published companion layer; do not fold Rust's process-global `log`
facade, proc macros, Tauri, or Python dependencies into the core logger.

```text
sc-observability-types
  └── sc-observability
       └── sc-observability-log
            └── sc-observability-log-macros

optional, later adapters
  ├── sc-observability-tauri
  └── bindings/python/sc-observability-py
```

This preserves a minimal core for consumers that use `Logger` directly while
giving facade users an idiomatic, supported integration. The traceable baseline
is the BTIT bridge and macro code at `f6f69dc`; copy the generic crates and
their tests, not BTIT's Tauri commands, Vue UI, app paths, or application
lifecycle policy.

## Blocking design findings

| ID | Finding | Required resolution |
| --- | --- | --- |
| A5-D01 | `Bridge::log` maps/formats a `log::Record` before `submit_guarded` begins. A custom `Display` or key-value formatter can panic through `log!`; logging from it bypasses `ReentrantEmit`. | Establish one outer guard per record covering lookup, formatting, mapping, event assembly, redaction, and `try_log`. Retain one private unguarded submit core used by both facade and macro paths. Never nest the existing guarded emit function inside another guard. |
| A5-D02 | A sole-owner `LogGuard::shutdown(self)` conflicts with consumer use of `Arc<LogGuard>` for background flushes. A timed shutdown can remove the slot while a detached helper retains the logger, and guard drop can issue a second shutdown. | Split ownership: non-cloneable `LogGuard` is the sole lifecycle owner; cloneable read-only `LogControl`/`LogStatusHandle` may flush/query path/health but cannot shut down on drop. Model serialized shutdown explicitly. A timeout means the caller stopped waiting, not that the logger has stopped. |

## Important design findings

| ID | Finding | Required resolution |
| --- | --- | --- |
| A5-D03 | Drop counters alone do not tell an operator why a bridge is degraded. | Provide a serializable bridge-owned `BridgeHealthReport` containing `LoggingHealthReport`, bridge counters, lifecycle state, and active path. Do not expose mutable `Logger` ownership. |
| A5-D04 | Macro fields enforce `field_key_label`, whereas `log::kv` fields are inserted raw. Reserved prefixes, invalid key handling, and sanitized-key collisions therefore vary by source. | Freeze one field-key policy: reject/count invalid or reserved keys and define deterministic collision behavior for all input paths. |
| A5-D05 | The bridge resolves `LoggerConfig.process_identity`, but the core logger stores rather than consumes it; BTIT currently documents `Auto` with pid but no hostname. That conflicts with the core API promise of automatic hostname and pid. | Decide and document whether core or producer owns identity. If `Auto` is supported, resolve/cache hostname and pid once at initialization cross-platform; align bridge, core, docs, and tests. |
| A5-D06 | `log::set_boxed_logger` is process-global and irrevocable. | Document install-once and no-normal-reinitialization as a public lifecycle rule. Test it in subprocesses, not ordinary same-process tests. |

## Contract to lock before implementation

### Lifecycle

`LogGuard` owns the final transition. `LogControl` is cloneable but has no
shutdown method. Lifecycle states are `Running`, `Stopping`, `Stopped`, and
`ShutdownTimedOut`; only one transition to `Stopping` is permitted. Calls made
after `Stopping` return a typed stable outcome. Bounded flush and shutdown
methods must say whether the operation completed, timed out while still
running, or could not start; an implicit `Drop` path is fallback-only and must
not be the normal observable shutdown mechanism.

### Health DTO

`BridgeHealthReport` is a versioned, serde-serializable projection suitable
for application diagnostics and future language adapters. It includes the
underlying `LoggingHealthReport`, bridge drop counts by cause, lifecycle state,
and `active_log_path`. Diagnostic values retain stable code, message, and
remediation; adapters must not parse display strings.

### TypeScript / Tauri direction

Keep BTIT's commands, permissions, retention/delete policy, and UI rendering
in BTIT. If a generic adapter proves useful, create optional
`sc-observability-tauri`, exposing only versioned serializable DTOs:

- `LogEventDto`, `LogQueryDto`, and paged query results;
- `BridgeHealthDto` and `LogHealthDto`;
- `FrontendLogRequest` with explicit target, size, and redaction policy;
- tagged `ObservabilityErrorDto { code, message, remediation }`.

Use a Rust type exporter such as Specta behind the optional adapter to generate
TypeScript types and, if selected, Tauri command declarations. Generated files
must be checked in or generated-and-diffed in CI. Rust serde derives alone are
not a frontend compatibility contract; version the DTO schema separately. The
core crates must not gain Tauri or TypeScript tooling dependencies.

### Python / maturin direction

Only after the Rust lifecycle and health contracts stabilize, add a thin
PyO3/maturin package under `bindings/python/sc-observability-py/`. Expose a
Python-owned `Observability` object with `try_log`, `log`, `query`, `health`,
`flush`, and `shutdown`; use JSON-compatible DTOs or generated Python models
and map failures through stable error codes/remediations. Release the GIL for
blocking flush, query, and shutdown. Do not accept Python callback sinks or
redactors in v1, and do not silently install the Rust global `log` facade.

The package requires a `pyproject.toml`, a PyO3 `cdylib`, typed Python stubs,
and a macOS/Linux/Windows wheel matrix. Select minimum Python version and
whether `abi3` is worth its compatibility constraints before implementation.

## Validation required for the design

1. Enabled `log!` calls with panicking `Display` and key-value formatters do
   not unwind; each produces exactly one `LoggerPanicked` count.
2. Nested logging from a formatter is not written and produces exactly one
   `ReentrantEmit` count; the outer record writes once.
3. Subprocess tests prove the install-once rule before and after shutdown.
4. Tests cover field-key policy, collisions, lifecycle transitions, timeout
   observation, health transitions, JSONL output, macro compatibility,
   compile-fail UI fixtures, and external-consumer compilation.
5. Adapter tests serialize the DTO schema, verify generated TypeScript is
   current, and exercise Python wheel/import behavior on supported platforms.

## Open decisions

- Does `Auto` identity guarantee hostname, or is that public promise reduced?
- Is a post-timeout shutdown retry/observation interface required, or is
  terminal `Stopping` sufficient?
- Should `BridgeHealthReport` be bridge-specific (recommended) or a shared
  generic health envelope?
- Which raw `log::kv` keys are accepted, and how are collisions resolved?
- Should BTIT prove its DTO command boundary first (recommended), followed by
  extraction only of demonstrated generic Tauri operations?
- Which Python versions and platforms are supported, and is `abi3` required?

