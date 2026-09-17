---
id: B
status: proposed
branch: plan/phase-b
base: develop
guideline_commit: 3d9ddbb0e8d9079a8efa0c471f5b5d8be549a878
---

# Phase B — Publish the log bridge and add language bindings

This proposal keeps B.1 as the first migration sprint: copy the corrected generic
BTIT code; publish the Rust crates so BTIT can consume them later; then add
TypeScript and Python bindings. Go is future scope. This document is a routing
index; each linked sprint is authoritative for its own deliverables, acceptance,
validation, and non-closure. No implementation or publication is authorized by
this proposal's status.

## Pre-copy prerequisite sprints

These are real implementation/release/integration work units, not invisible
entry assumptions. They must complete before the first migration sprint B.1.

| Sprint | Production deliverable | Authoritative plan |
| --- | --- | --- |
| B.P1 | Additive per-logger runtime level state, owner capability and admission outcomes | [Core runtime](sprint-b-p1-runtime-core.md) |
| B.P2 | Staged/qualified core capability with two-leg consumer proof; B.7 alone publishes | [Prerequisite qualification](sprint-b-p2-runtime-publish.md) |
| B.P3 | Accepted BTIT bridge integration and critical-review closure | [BTIT integration](sprint-b-p3-runtime-btit.md) |

## Migration, error evolution and binding sprints

| Sprint | Production deliverable | Authoritative plan |
| --- | --- | --- |
| B.1 | Traceable copy of corrected generic BTIT crates, building in this workspace | [Copy](sprint-b-1-copy.md) |
| B.1a | Neutral classified failures and explicit typed extension adapters | [Error model](sprint-b-1a-error-api.md) |
| B.1b | Additive logger methods and typed sink integration | [Logger errors](sprint-b-1b-logger-errors.md) |
| B.1c | Additive observation runtime methods | [Observation errors](sprint-b-1c-observation-errors.md) |
| B.1d | Additive telemetry runtime methods | [Telemetry errors](sprint-b-1d-telemetry-errors.md) |
| B.1e | Warning-only legacy migration and downstream adoption guidance | [Error adoption](sprint-b-1e-error-adoption.md) |
| B.2 | Published Rust bridge and macros, verified from crates.io | [Rust publication](sprint-b-2-publish-rust.md) |
| B.3 | Neutral DTO crate, checked conversions, schema and conformance fixtures | [Shared schema](sprint-b-3-schema.md) |
| B.3b | Shared native core/bridge backends, conversions and bounded operation coordinator | [Native runtime](sprint-b-3b-native-runtime.md) |
| B.3a | Generated TypeScript client, Tauri host adapter and real IPC consumer | [TypeScript](sprint-b-3a-typescript.md) |
| B.4 | Owned and host-attached Python runtime API through PyO3 | [Python runtime](sprint-b-4-python.md) |
| B.4a | Maturin wheels, sdist and full Python/platform qualification | [Python packaging](sprint-b-4a-python-packaging.md) |
| B.5 | Standard-library Python logging and context propagation in mixed applications | [Python integration](sprint-b-5-python-integration.md) |
| B.6 | Fire-and-forget Python submission with optional asynchronous confirmation | [Async Python](sprint-b-6-python-async.md) |
| B.7 | Published TypeScript and Python packages with registry-consumer proof | [Binding publication](sprint-b-7-publish-bindings.md) |

## Source and phase boundaries

Phase A in this repository is complete at v1.2.0; its accepted record is
[Phase A readiness](../phase-a/readiness.md). The abandoned A.5 migration
proposal in the separate `plan/phase-a-log-facade-bridge` branch is not a new
accepted Phase A sprint and is not this plan's authority.

sc-observability owns the target public API. The
[target bridge API proposal](target-bridge-api.md) is reviewed now so BTIT can
complete its initial design/implementation against the accepted contract. This
is an active contract-review gate, not a passive wait for an arbitrary BTIT API.
The proposal is not yet an accepted freeze. No backward-compatibility/semver
constraint from BTIT's unpublished API applies to this initial destination API.

The authoritative [B.1 entry gate](sprint-b-1-copy.md#goal-and-entry-gate)
defines contract approval, published runtime capability, accepted BTIT
implementation/review and immutable source provenance. B.1 is a mechanical
copy of that accepted implementation; all foreseeable bridge API changes are
implemented in BTIT before migration. B.2 publishes the companion pair.
Subsequent core error evolution and language bindings preserve the accepted
bridge public contract. Unforeseen future changes remain possible but are not
a planned redesign sprint.

The [BTIT handoff record](btit-api-handoff.md) routes that gate and records the division
of responsibility. Source behavior is preserved only where selected in the
target matrix; intentional revisions are explicit. Existing published core
crates retain their own release guarantees.

## Runtime level prerequisite

[Runtime level elevation](runtime-level-contract.md) implements issue #97 as an
explicit pre-copy core release and BTIT integration gate. Core owns the shared
effective level; the host retains mutation authority. B.1 remains the first
migration sprint. No bridge setter or filtering redesign is deferred until after
copy. #96 configuration loading is independent; LoggerConfig supplies baseline.

## Core error API migration

[B.1a](sprint-b-1a-error-api.md) through
[B.1e](sprint-b-1e-error-adoption.md) share the concrete
[error API contract](error-api-contract.md). They separately implement neutral
failures/adapters, logger, observation and telemetry entry points, then the
warning-only migration and adoption guide. Existing published signatures,
structs, traits, enum exhaustiveness, codes and serialized forms remain intact.
New methods and types coexist with old interfaces; no planned removal or 2.0
conversion is authorized. B.2 publishes the completed additive migration.

The plan contains 18 bounded sprint records including the three pre-copy
prerequisites. The identifiers retain existing B.1–B.7 references; scope is split
by production closure rather than hidden in prerequisites or implementation notes.

## Proposed binding architecture

Rust's public logging API remains the behavior authority. Companion packages
convert language inputs into public Rust values and expose stable errors;
neither language runtime becomes a dependency of the core crates.

- `crates/sc-observability-dto/`: language-neutral, versioned wire values and
  checked conversions, owned by B.3. It depends on public types, not Tauri or
  PyO3. Language exporters live outside this crate.
- `crates/sc-observability-binding-runtime/`: B.3b owns provided core/bridge
  backends, runtime-to-DTO conversion and bounded native operation coordination.
  It depends on core, DTOs and bridge; it has no Tauri/PyO3 dependency.
- `bindings/typescript/`: generated DTO declarations, transport abstraction,
  and Tauri client; `bindings/tauri/` owns host-side integration. The host owns
  initialization, filesystem policy, command registration, and shutdown.
- `bindings/python/sc-observability-py/`: a PyO3 extension with Python wrappers,
  stubs, and maturin packaging. Python supports both an independently owned
  logger and attachment to an existing Rust host logger. Attached Python, Rust backend, and Tauri frontend
  records use one host-owned writer, configuration, health and lifecycle.
  Importing the package never installs Rust's global log facade.
- Future Go bindings may reuse DTO schemas, fixtures, and error codes. A Go
  package, C ABI, cgo adapter, RPC transport, and Go release matrix are all
  deferred until there is a concrete Go consumer. JSON compatibility is not an
  FFI design and does not select a future transport.

The user-selected first TypeScript runtime is Tauri because BTIT is the concrete
consumer. A standalone Node.js runtime would require an additional native-addon
or service transport sprint; generated TypeScript alone does not execute Rust.

## Default logging behavior

Every new fallible public operation returns a discriminated result, including creation,
validation, emission, query, health, flush, shutdown, and asynchronous waiting.
Operational errors are values; neither language binding intentionally throws,
raises, rejects a Promise, or uses exceptions internally for expected failures.
Rust continues to use Result/enums. Existing infallible accessors and fixed
standard-library unit-return adapters retain their signatures; adapters record
ignored results in inspectable status, without changing published APIs. B.3 owns the shared result/error schema; B.3a implements TypeScript/Tauri;
Python generates equivalent tagged dataclasses and unions from that contract.
Accepted and filtered admissions remain distinct; neither implies persistence.

Normal emission is nonblocking. Calling code may ignore its returned result and
continue, including when an error record cannot be logged. Ignoring an error is
then the caller's choice; the library does not disguise a failed record as a
success or require error handling to keep the application running. Best-effort
bounded health accounting never recurses through logging. If accounting also
fails, preserve the original operation result and do not raise a second failure.
Foreign formatter/transport errors are converted at the boundary, not propagated.

Python B.6 adds immediate submission with an optional awaitable receipt and
async flush. sc-runtime's process/interpreter topology, IPC, supervision, worker
fairness and cross-worker ordering remain deferred. The in-process host example
is a supported mode, not a decision about the future runtime architecture.

## Initial public API coverage

The first language bindings cover structured logging, bounded historical query,
health, and flush. Python also exposes explicit creation/shutdown in owned mode,
attachment without lifecycle ownership in host mode, standard-library logging
integration, context propagation and owner-only level elevation/reset. Python is a first-class supported language
with the same release quality and conformance expectations as TypeScript. This is
an explicit logging subset of the public API, not a claim of whole-workspace
parity. B.3 owns shared DTOs and limits; B.3b owns native backends/coordination;
B.3a/B.4 own language runtime projections.

Deferred: `follow` sessions, custom callback sinks/redactors, Rust proc macros,
`sc-observe` generic observation routing, OTLP attachment/exporter setup,
frontend-owned process-global initialization/shutdown, log deletion, and Node.js.
BTIT's later switch to the published crates is a separate BTIT change.

## Dependencies and ownership

| Relation | Rationale |
| --- | --- |
| B.P1 must_follow the execution-authorized runtime contract (public acceptance owner-deferred to Phase B completion) | Implement a reviewed additive core surface without claiming runtime acceptance |
| B.P2 must_follow B.P1 | Qualify and stage the tested core capability; B.7 alone publishes |
| B.P3 must_follow B.P2 and accepted target bridge contract | BTIT integrates B.P2's staged core behavior before source acceptance |
| B.1 must_follow B.P3 | Copy only the accepted implementation of the sc-observability-owned locked target |
| B.1a must_follow B.1 | Verify additive core error evolution against the copied bridge contract |
| B.1b must_follow B.1a | Logger methods consume neutral failures and adapters |
| B.1c must_follow B.1b | Observation integration uses the completed logger compatibility path |
| B.1d must_follow B.1c | Telemetry composes the completed observation/runtime adapters |
| B.1e must_follow B.1d | Deprecate only after all replacements and upgrade fixtures work |
| B.2 must_follow B.1e | Publish the improved core API and warning-only compatibility path with the companion release |
| B.3 must_follow B.2 | Implement neutral DTO conversions against the published Rust baseline |
| B.3b must_follow B.3 | Implement native backends against the accepted DTO contract |
| B.3a must_follow B.3b | Consume shared native runtime in TypeScript/Tauri and real IPC tests |
| B.4 must_follow B.3a | Reuse schema and proven cross-language conformance evidence |
| B.4a must_follow B.4 | Qualify distributions of the completed Python runtime API |
| B.5 must_follow B.4a | Integrate Python logging/context using the qualified package baseline |
| B.6 must_follow B.5 | Add receipt/wait support to the established Python runtime/context behavior |
| B.7 must_follow B.6 | Publish the already-tested TypeScript and Python artifacts together |

No pair currently meets the guideline's full `parallel_safe` conditions: they
share public contracts, workspace/release metadata, or conformance fixtures.
For each internal `must_follow`, pushed parent development triggers merge-forward
before every child development/fix round; parent PR must merge before child PR
completion. A registry-dependent acceptance test additionally waits for the
parent's published artifact; a merged PR is not publication evidence. Cross-repo
B.P2→B.P3→B.1 edges use recorded release/source commits, not Git merges across
unrelated repositories.

Use feature branches/worktrees from `develop`; normal PRs target `develop`.
Release tags come from `main` under the existing release procedure. The team
lead runs plan hardening after this proposal is committed; no reviewer approval
or implementation readiness is claimed here.

## Planning validation and evidence

The plan uses the merged
[guideline](../../../.claude/skills/plan-hardening/sprint-planning-guidelines.md).
Each sprint writes its own execution evidence only when executed; no empty
handoff file is treated as evidence. Commands named as new validation scripts
in sprint deliverables must be implemented by that sprint before its acceptance.
The next step is review/acceptance of the complete target/runtime/error/binding
contracts and their proposed requirements/ADRs, followed by B.P1–B.P3. B.1 stays blocked until that full contract
and source-acceptance gate passes.

The [consistency review record](review-consistency.md) captures the iterative
documentation checks and resolved findings; it is not implementation or API
approval.

The [required-document map](document-coverage.md) links per-crate requirements,
architecture, ADRs and test guidance. [Boundary contracts](boundaries.json) are
machine-readable planning definitions; [issue dispositions](issues-inventory.md)
record inclusion and deferral without asserting implementation closure.
