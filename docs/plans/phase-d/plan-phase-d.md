---
phase: D
status: draft
branch: plan/phase-d
base: develop
---

# Phase D — Host logging ergonomics, configuration, and distribution completion

Phase D closes six bounded, user-visible gaps in the logging and Python
distribution surface.  The linked sprint documents are authoritative for
deliverables, acceptance criteria, validation, and explicit non-closure.

| Sprint | Production deliverable | Source |
| --- | --- | --- |
| D.1 | [Host-owned macro bridge and host policy hook](sprint-d-1-host-logger-bridge.md) | [#204](https://github.com/randlee/sc-observability/issues/204) |
| D.2 | [Serde-stable startup `LogSettings`](sprint-d-2-log-settings.md) | [#96](https://github.com/randlee/sc-observability/issues/96) |
| D.3 | [Typed sink registration ergonomics](sprint-d-3-typed-sink-registration.md) | [#203](https://github.com/randlee/sc-observability/issues/203) |
| D.4 | [2.0 discriminated error enums and migration](sprint-d-4-error-enums-2-0.md) | [#92](https://github.com/randlee/sc-observability/issues/92) |
| D.5 | [Windows ARM64 Python wheel qualification](sprint-d-5-windows-arm64-wheel.md) | New platform gap |
| D.6 | [Open-ended Python ABI/metadata CI guard](sprint-d-6-python-open-ended-guard.md) | New regression guard |

## Sprint order

All implementation branches target `develop`; this plan does not authorize a
release, tag, registry publication, or a change to the supported Python floor.

| Relation | Rationale |
| --- | --- |
| D.1 `must_follow` D.2 | Both alter public logger construction/configuration semantics; D.2 establishes the one resolved startup configuration input before D.1 attaches a host-owned logger. |
| D.3 `must_follow` D.1 | Both change public logger integration surfaces; merge-forward D.1 before the typed registration API is finalized. |
| D.4 `must_follow` D.3 | D.4 removes the legacy `LogSinkError` boundary that D.3 makes avoidable in 1.x; its migration guide must name D.3's ergonomic replacement. |
| D.5 `must_follow` D.4 | The release/version and public API baseline must be settled before adding a new published wheel platform. |
| D.6 `must_follow` D.5 | The guard validates the final six-platform distribution configuration, including the ARM64 Windows entry. |

For every `must_follow` edge, merge the parent's pushed development into the
child before every development or fix round; the parent PR must merge before
the child PR completes. These are deliberately not `parallel_safe`: all touch
the same published logger/Python distribution contract, release metadata, or
consumer fixtures.

## Phase-wide constraints

- #88 (Python OTEL/structured logging) is explicitly out of scope. Do not add
  OTEL exporter work, a new Python structured logging surface, or a plan for it
  under this phase.
- D.4 is a breaking **2.0** sprint. It requires a versioning decision,
  migration guide, public-API approval, semver fixture updates, and consumer
  migration evidence; it may not masquerade as a 1.x additive release.
- Existing Python distribution compatibility remains open-ended: CPython
  stable ABI `abi3-py310`, `requires-python = ">=3.10"`, no upper bound, and
  all supported current interpreters. D.6 protects this; it does not add an
  upper cap.
- D.5 adds `aarch64-pc-windows-msvc` / `win_arm64` to the existing matrix. A
  cross-compiled artifact alone is insufficient: the wheel must be built and
  installed/executed on an ARM64 Windows runner with the existing offline,
  embedding, and full-suite evidence shape.

## Risk and open questions

- `log::set_boxed_logger` remains process-global. D.1 must prove foreign
  facade ownership is rejected without shutting down a host-owned logger, and
  document coexistence with a tracing bridge.
- The event policy must be bounded, deterministic and non-panicking at the
  bridge boundary; policy rejection must be observable without leaking fields
  that the policy redacts.
- Windows ARM64 runner availability and maturin/PyO3 target support are an
  execution prerequisite for D.5; if hosted ARM64 execution is unavailable,
  keep D.5 open rather than claiming x86 cross-build evidence is equivalent.
- Informational only, not a Phase D deliverable: a later OTLP-export sprint
  must address `sc-observability-otlp` model gaps against OTel—no `SpanKind`,
  no trace sampled flag or links, and `MetricRecord`'s single `f64` cannot
  represent histogram bucket boundaries, counts, sum, and count.

## Planning deliverables

This index and the six sprint docs, plus requirements/ADR/migration and CI
updates explicitly owned by those docs. Each sprint requires direct QA against
its authoritative acceptance list and retains exact command/evidence output.
