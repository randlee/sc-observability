---
id: B
status: proposed
branch: plan/phase-b
base: develop
guideline_commit: 3d9ddbb0e8d9079a8efa0c471f5b5d8be549a878
---

# Phase B — Publish the log bridge and add language bindings

This proposal follows the user's sequence: first copy the corrected generic
BTIT code; publish the Rust crates so BTIT can consume them later; then add
TypeScript and Python bindings. Go is future scope. This document is a routing
index; each linked sprint is authoritative for its own deliverables, acceptance,
validation, and non-closure. No implementation or publication is authorized by
this proposal's status.

## Sprint sequence

| Sprint | Production deliverable | Authoritative plan |
| --- | --- | --- |
| B.1 | Traceable copy of corrected generic BTIT crates, building in this workspace | [Copy](sprint-b-1-copy.md) |
| B.2 | Published Rust bridge and macros, verified from crates.io | [Rust publication](sprint-b-2-publish-rust.md) |
| B.3 | Generated TypeScript contract and functioning Tauri frontend client | [TypeScript](sprint-b-3-typescript.md) |
| B.4 | Python logging API implemented through PyO3, with tested maturin wheels | [Python](sprint-b-4-python.md) |
| B.5 | Standard-library Python logging and context propagation in mixed applications | [Python integration](sprint-b-5-python-integration.md) |
| B.6 | Published TypeScript and Python packages with registry-consumer proof | [Binding publication](sprint-b-6-publish-bindings.md) |

## Source and phase boundaries

Phase A in this repository is complete at v1.2.0; its accepted record is
[Phase A readiness](../phase-a/readiness.md). The abandoned A.5 migration
proposal in the separate `plan/phase-a-log-facade-bridge` branch is not a new
accepted Phase A sprint and is not this plan's authority.

BTIT's critical review is maintained separately in its repository at
`docs/plans/phase-a/review-a-5.md`. At planning time it reports open findings
against `f6f69dc`. That SHA is an audit reference, **not an approved import
source**. B.1 starts only after BTIT's fixes and re-review produce an accepted,
immutable source commit. The copy sprint preserves the accepted lifecycle,
formatting, identity, and health behavior. New correctness findings return to BTIT before
import; a destination-only redesign requires a separately scoped sprint.

## Proposed binding architecture

Rust's public logging API remains the behavior authority. Companion packages
convert language inputs into public Rust values and expose stable errors;
neither language runtime becomes a dependency of the core crates.

- `crates/sc-observability-dto/`: language-neutral, versioned wire values and
  checked conversions, owned by B.3. It depends on public types, not Tauri or
  PyO3. Language exporters live outside this crate.
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

## Initial public API coverage

The first language bindings cover structured logging, bounded historical query,
health, and flush. Python also exposes explicit creation/shutdown in owned mode,
attachment without lifecycle ownership in host mode, standard-library logging
integration and context propagation. Python is a first-class supported language
with the same release quality and conformance expectations as TypeScript. This is
an explicit logging subset of the public API, not a claim of whole-workspace
parity. The B.3 sprint owns the detailed operation/DTO contract and its limits.

Deferred: `follow` sessions, custom callback sinks/redactors, Rust proc macros,
`sc-observe` generic observation routing, OTLP attachment/exporter setup,
frontend-owned process-global initialization/shutdown, log deletion, and Node.js.
BTIT's later switch to the published crates is a separate BTIT change.

## Dependencies and ownership

| Relation | Rationale |
| --- | --- |
| B.1 must_follow BTIT accepted review/fix handoff | Import only corrected, independently accepted source |
| B.2 must_follow B.1 | Packages and provenance must exist before publication |
| B.3 must_follow B.2 | Bind against the published Rust baseline; own shared DTO schema once |
| B.4 must_follow B.3 | Reuse the accepted DTO conversions/error registry and conformance fixtures |
| B.5 must_follow B.4 | Integrate Python logging/context with the implemented owned/attached runtime |
| B.6 must_follow B.5 | Publish the already-tested TypeScript and Python artifacts together |

No pair currently meets the guideline's full `parallel_safe` conditions: they
share public contracts, workspace/release metadata, or conformance fixtures.
For each internal `must_follow`, pushed parent development triggers merge-forward
before every child development/fix round; parent PR must merge before child PR
completion. A registry-dependent acceptance test additionally waits for the
parent's published artifact; a merged PR is not publication evidence.

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
The next step is plan review, then source-handoff readiness for B.1.
