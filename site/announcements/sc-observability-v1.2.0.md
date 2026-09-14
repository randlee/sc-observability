# sc-observability v1.2.0 — Queue-Admission Logging API and Public API Governance

**Released:** May 26, 2026 · **Install:** add `sc-observability = "1.2"` to `Cargo.toml` (plus `sc-observability-types`, `sc-observe`, and `sc-observability-otlp` from [crates.io](https://crates.io/search?q=sc-observability))

[Changelog](https://github.com/randlee/sc-observability/blob/main/CHANGELOG.md) · [Release notes](https://github.com/randlee/sc-observability/releases/tag/v1.2.0)

---

## Rust Developer

**As a Rust developer, I want reusable observability crates with standard consistent patterns, so that I can instrument my SC component without bespoke logging glue.**

v1.2.0 replaces fire-and-forget `emit()` with an explicit queue-admission contract. `Logger::log(event)` blocks to confirm admission to the writer queue; `Logger::try_log(event)` never blocks and returns `TryLogError::QueueFull(...)` on saturation, so each caller chooses its own backpressure policy. `Logger::shutdown()` drains the queue, joins the writer thread, and returns a `Logger<Stopped>` typestate for post-shutdown health inspection.

Behind the API, a single queue-backed writer thread now owns batching, sink writes, rotation, pruning, flush, and shutdown sequencing — the logging path is serialized in one place instead of scattered across call sites. Durability-sensitive callers use `flush()` or `shutdown()` to guarantee admitted records reach their sinks.

The health surface grows to match: `LoggingHealthReport` now exposes queue depth, capacity, high-water mark, queue-full drop count, writer state, last writer error, and maintenance health, so a component can answer "is my logger keeping up?" at runtime. Queue-depth accounting also rolls back on failed send paths, eliminating depth underflow and poisoned high-water metrics under hot scheduling.

---

## Observability Developer

**As an observability developer, I want shared contracts (identifiers, diagnostics, health reports) stable across components, so that telemetry from different SC products composes cleanly.**

Public API governance is now enforced in CI. `validate_public_api_diff.sh`, `validate_public_api_semver.py`, and `validate_public_api_docs.sh` gate every change against approved diffs, so the shared contracts in `sc-observability-types` can't drift silently. The release ships with docs.rs API references for all four crates and a new `CONSUMING.md` covering queue admission, durability, and migration off `emit()`.

---

## Skipped personas

- **Python Developer** — Python bindings are planned but not yet shipped; v1.2.0 contains no Python bindings. No section included this release.

---

## What's Next

`Logger::emit()` remains available as a compatibility path in 1.2.0 and is planned for removal in a future release. Python bindings are the next major surface on the roadmap.
