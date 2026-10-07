# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.5.0] - Unreleased

This is a compatible 1.x release. Every public API released in 1.4.1 remains
available with its released signature, behavior, error variants and
serialization format. New canonical APIs live under each crate's `v2` module;
released 1.x paths stay behind the default-on `v1` feature and are deprecated.
Build with `default-features = false` to prove a consumer has migrated. See
`docs/migration/phase-f.md` and `release/RELEASE-NOTES-1.5.0.md`.

Public API reference for this release:
- `sc-observability-types`: <https://docs.rs/sc-observability-types/1.5.0/sc_observability_types/>
- `sc-observability`: <https://docs.rs/sc-observability/1.5.0/sc_observability/>
- `sc-observe`: <https://docs.rs/sc-observe/1.5.0/sc_observe/>
- `sc-observability-otlp`: <https://docs.rs/sc-observability-otlp/1.5.0/sc_observability_otlp/>
- `sc-observability-log-macros`: <https://docs.rs/sc-observability-log-macros/1.5.0/sc_observability_log_macros/>
- `sc-observability-log`: <https://docs.rs/sc-observability-log/1.5.0/sc_observability_log/>
- `sc-observability-dto`: <https://docs.rs/sc-observability-dto/1.5.0/sc_observability_dto/>
- `sc-observability-binding-runtime`: <https://docs.rs/sc-observability-binding-runtime/1.5.0/sc_observability_binding_runtime/>
- `sc-observability-tauri`: <https://docs.rs/sc-observability-tauri/1.5.0/sc_observability_tauri/>
- `sc-observability-py`: <https://docs.rs/sc-observability-py/1.5.0/sc_observability/>

### Added

- Canonical `v2` modules with typed error families (`InitError`, `EventError`,
  `FlushError`, `LogSinkError`, `IdentityError`, ...) and neutral telemetry
  signal records (spans, logs, metrics, profiles, state transitions) with
  validation in `sc-observability-types`.
- `LogSettings` / `LogSettingsInputs`: a single serde-stable, binding-friendly
  logging configuration value with documented settings error codes.
- Typed sink registration (`SinkRegistration::typed`) and a canonical
  `v2::Logger` with `flush_with_timeout` / `shutdown_with_timeout`.
- Host-owned logger attachment bridge (`attach_logger`) in
  `sc-observability-log`, routing `log` macros into an existing host logger.
- OTLP: a shared lifecycle core with two selectable backends: `otlp-sdk`
  (official OpenTelemetry SDK on Tokio, including OTLP/HTTP protobuf) and
  `sync-http` (synchronous OTLP/HTTP JSON), composed through one exporter
  factory, with bounded SDK transport retries.
- OTLP telemetry submission contract (`SubmissionEnvelope`) and
  `DurableTelemetryClient` (feature `durable-store`) with a durable local
  store, drain, and OTLP/JSON encoders for every signal, honoring collector
  partial-success rejections.
- Python bindings expose telemetry submission (release wheels enable the
  `otlp-telemetry` feature) and release the GIL for flush, shutdown and health.
- Windows ARM64 (`win_arm64`) Python wheel; the wheel matrix is now six platforms.
- Canonical v2 failure projection in the DTO schema and the Python/TypeScript/Tauri bindings.

### Changed

- Released 1.x facades now delegate to the canonical implementation; 1.x
  items are isolated behind the default-on `v1` feature in every library crate.
- Lifecycle handles use a shared-reference shutdown contract: no-argument
  `flush()` / `shutdown()` use the configured deadline; `_with_timeout`
  variants take an explicit one.
- Logging admission and queue capacity are bounded, and diagnostics are
  canonical; shutdown-time emits and flushes are classified consistently.
- Public API compatibility is checked with the stock `cargo public-api`
  baseline gate against 1.4.1.

### Deprecated

- `Logger::emit()`; use `Logger::log()` or `Logger::try_log()`.
- Legacy error wrappers `IdentityError`, `InitError`, `EventError`,
  `FlushError`, `ShutdownError`, `ProjectionError`, `SubscriberError`,
  `LogSinkError` and `ExportError`; use the `sc_observability_types::typed`
  / `v2` failures.
- `OtlpEndpoint::new()`, `AuthHeader::new()`, `SpanAssembler::push()` and
  `TelemetryConfigBuilder::build()`; use their `*_typed` counterparts.
- `RetentionPolicy::max_age_days` for logger-managed maintenance; use `RetainedLogPolicy`.
- Root (non-`v2`) facade items and the v1 `_typed` lifecycle methods listed in
  `docs/migration/phase-f.md`. They will be removed in a later release.

### Removed

- Nothing. All 1.4.1 public APIs remain available (deprecated where noted).

### Fixed

- OTLP: detached batches drain before shutdown; failed terminal shutdown is
  replayed to later callers; log parent-span correlation is preserved;
  admitted export failures surface at flush; SDK retries cancel after shutdown.
- Log bridge: drop-cause classification is shared across emit paths, and
  disabled records are skipped before guarded submission; a missing retained
  log directory is ignored; released `LogControl` auto traits are restored.
- `sc-observe`: released root configuration fields and registration signatures are restored.

## [1.4.1] - 2026-09-20

This maintenance release carries the qualified 1.4.0 public API surface into
the coordinated 1.4.1 publication train. Rust, Python, and TypeScript package
metadata and internal dependency pins are aligned at 1.4.1; the historical
1.4.0 qualification evidence and API baselines remain unchanged.

### Changed

- Regenerated release manifests and lockfiles for the 1.4.1 package train.
- Preserved the existing additive typed-error and binding contracts without
  introducing unrelated runtime or API changes.

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
- Public API history is checked with the stock `cargo public-api` baseline gate.
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
