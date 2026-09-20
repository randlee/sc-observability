# Changelog

## 1.4.0 — qualified candidate, publication pending separate authorization

The six-crate candidate includes additive typed error APIs and warning-only
compatibility paths from B.1a–B.1e, plus the first bridge/macros public baseline.
The bridge pins macros exactly at `=1.4.0`. Existing `emit` callers retain their
compatibility path; follow `docs/migration-guide.md` for typed replacements and
the queue-admission versus durability distinction. All package archives include
MIT license bytes. No candidate is published by B.2.

Versioned API URLs below become available only after separately authorized
publication:

- `sc-observability-types`: <https://docs.rs/sc-observability-types/1.4.0/sc_observability_types/>
- `sc-observability`: <https://docs.rs/sc-observability/1.4.0/sc_observability/>
- `sc-observe`: <https://docs.rs/sc-observe/1.4.0/sc_observe/>
- `sc-observability-otlp`: <https://docs.rs/sc-observability-otlp/1.4.0/sc_observability_otlp/>
- `sc-observability-log-macros`: <https://docs.rs/sc-observability-log-macros/1.4.0/sc_observability_log_macros/>
- `sc-observability-log`: <https://docs.rs/sc-observability-log/1.4.0/sc_observability_log/>

Phase-B binding qualification also covers the neutral DTO and native-runtime
crates, the Tauri host adapter, the PyO3 extension, the Python wheel/sdist
matrix, and the generated TypeScript client. Their intended channels and
dependency order are recorded in `release/bindings-artifacts.toml`; publication
remains deferred until after Phase B merge, the Phase C `sc-publish` migration,
and separate publication authorization.


All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Candidate `1.4.0` is staged only; it is not published or approved for release.

### Added

- Candidate-only `LevelOwner`, runtime threshold mutation, filtered admission,
  query, and shutdown behavior is qualified through the B.P2 extracted-package
  consumer. The release is pending the retained three-platform evidence and
  post-Phase-B publication authorization.

## [1.2.0] - 2026-05-26

This release includes approved public API changes relative to `1.1.0`; review
the migration notes below before upgrading.

Public API reference for this release:
- `sc-observability-types`: <https://docs.rs/sc-observability-types/1.2.0/sc_observability_types/>
- `sc-observability`: <https://docs.rs/sc-observability/1.2.0/sc_observability/>
- `sc-observe`: <https://docs.rs/sc-observe/1.2.0/sc_observe/>
- `sc-observability-otlp`: <https://docs.rs/sc-observability-otlp/1.2.0/sc_observability_otlp/>

### Added

- `Logger::log(event)` as the new blocking queue-admission logging API.
- `Logger::try_log(event)` as the new non-blocking queue-admission logging API, including explicit `TryLogError::QueueFull(...)` behavior on saturation.
- `Logger::shutdown(self) -> Logger<Stopped>` for draining queued events, joining the writer thread, and returning a stopped typestate logger for post-shutdown health inspection.
- `LogError` with `InvalidEvent(EventError)`, `WriterDegraded(#[source] Box<ErrorContext>)`, and `ShutdownTimedOut(#[source] Box<ErrorContext>)`.
- `TryLogError` with `InvalidEvent(EventError)`, `QueueFull(#[source] Box<ErrorContext>)`, `WriterDegraded(#[source] Box<ErrorContext>)`, and `ShutdownTimedOut(#[source] Box<ErrorContext>)`.
- `WriterState` re-export for live writer-thread health inspection.
- `MaintenanceHealthReport` and `MaintenanceWorkerState` re-exports from `sc-observability-types`.
- `LoggingHealthReport` queue/writer health fields, including queue depth, queue capacity, queue high-water mark, queue-full drop count, writer state, last writer error, and maintenance health.
- Explicit open-contract doc comments on the public `LogSink`, `LogFilter`, and `Redactor` traits.
- Queue-backed writer runtime with a single writer thread that owns batching, sink writes, rotation, pruning, flush, and shutdown sequencing.
- Public API governance CI gates: `validate_public_api_diff.sh`, `validate_public_api_semver.py`, and `validate_public_api_docs.sh`.
- Consumer documentation in `CONSUMING.md`, including dedicated `Queue Admission And Durability` and `Migrating From emit()` sections.
- `WriterShutdownTimeout` / `writer_shutdown_timeout` as the configurable writer-shutdown timeout threshold.

### Deprecated

- `Logger::emit()` in favor of `log()` and `try_log()`. The compatibility path remains available in `1.2.0` and is planned for removal in a future release.

### Changed

- Logging calls are now queue-admission operations by default. `log()` and `try_log()` confirm admission to the writer queue, not synchronous sink durability.
- Durability-sensitive callers must now use `flush()` or `shutdown()` to ensure admitted records are committed through the configured sinks.
- Logger shutdown now records timeout-threshold degradation in health/error reporting but still waits for definitive writer-thread completion before returning `Logger<Stopped>`.

### Fixed

- Queue-depth accounting now records enqueue intent before channel handoff and rolls back on failed send paths, preventing queue-depth underflow and poisoned high-water metrics under hot scheduling.
- Shutdown now returns `Logger<Stopped>` only after the writer thread has definitively joined, closing the detached-writer timeout gap from the initial phase-A implementation.
