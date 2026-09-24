---
phase: D
status: draft
branch: plan/phase-d
base: develop
---

# Phase D — Host logging ergonomics, configuration, and distribution completion

Phase D closes ten bounded, user-visible gaps in the logging, OTLP, and Python
distribution surface.  The linked sprint documents are authoritative for
deliverables, acceptance criteria, validation, and explicit non-closure.

| Sprint | Production deliverable | Source |
| --- | --- | --- |
| D.1 | [Host-owned macro bridge and host policy hook](sprint-d-1-host-logger-bridge.md) | [#204](https://github.com/randlee/sc-observability/issues/204) |
| D.2 | [Serde-stable startup `LogSettings`](sprint-d-2-log-settings.md) | [#96](https://github.com/randlee/sc-observability/issues/96) |
| D.3 | [Typed sink registration ergonomics](sprint-d-3-typed-sink-registration.md) | [#203](https://github.com/randlee/sc-observability/issues/203) |
| D.4 | [2.0 discriminated error enums and migration](sprint-d-4-error-enums-2-0.md) | [#92](https://github.com/randlee/sc-observability/issues/92) |
| D.7a | [OTLP 2.0 signal model](sprint-d-7a-otlp-signal-model.md) | Split regression prerequisite |
| D.7b | [Official SDK/Tokio exporter](sprint-d-7b-otlp-sdk-tokio.md) | Restored primary transport |
| D.7c | [Legacy HTTP/JSON source transplant](sprint-d-7c-otlp-http-json-transplant.md) | Restored synchronous transport |
| D.7d | [Cross-path qualification and observability docs](sprint-d-7d-otlp-conformance.md) | Regression closure |
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
| D.7a `must_follow` D.4 | The OTLP data-model contract is a 2.0 change and must use D.4's accepted error/version/migration baseline. |
| D.7b `must_follow` D.7a | The SDK exporter consumes the accepted spec-correct neutral signal model. |
| D.7c `must_follow` D.7b | Both transports change `sc-observability-otlp`, `OtelConfig`, dependencies, and exporter ownership; D.7b freezes the backend selector and lifecycle contract before the source transplant. |
| D.7d `must_follow` D.7c | Cross-path qualification requires both production transports; D.7b must also be complete before D.7d begins. |
| D.5 `must_follow` D.7d | The release/version and public API baseline, including the restored telemetry package, must be settled before adding a new published wheel platform. |
| D.6 `must_follow` D.5 | The guard validates the final six-platform distribution configuration, including the ARM64 Windows entry. |

For every `must_follow` edge, merge the parent's pushed development into the
child before every development or fix round; the parent PR must merge before
the child PR completes. These are deliberately not `parallel_safe`: all touch
the same published logger/Python distribution contract, release metadata, or
consumer fixtures.

## Phase-wide constraints

- #88 (Python OTEL/structured logging) is explicitly out of scope. Do not add
  a new Python OTEL/structured logging surface. D.7a–D.7d restore the existing
  Rust OTLP exporter regression only; they are not #88 work.
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
- D.7a–D.7d repair OTLP data-model prerequisites and both real exporters:
  `SpanKind`, trace sampled flag and links, plus a histogram representation
  with bucket boundaries/counts/sum/count. It scopes both the official SDK
  integration (for Tokio-hosted consumers) and the restored synchronous legacy
  HTTP/JSON path. The old single-`f64` histogram placeholder
  is not spec-correct and cannot be exported as one.

## Planning deliverables

This index and the ten sprint docs, plus requirements/ADR/migration and CI
updates explicitly owned by those docs. Each sprint requires direct QA against
its authoritative acceptance list and retains exact command/evidence output.
