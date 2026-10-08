# sc-observability v1.5.0 — Canonical v2 APIs, Durable OTLP Telemetry, and Six-Platform Python Bindings

**Released:** October 7, 2026 · **Install:** add `sc-observability = "1.5"` to `Cargo.toml` (ten crates on [crates.io](https://crates.io/search?q=sc-observability)); Python bindings via `pip install sc-observability` from [PyPI](https://pypi.org/project/sc-observability/)

[Changelog](https://github.com/randlee/sc-observability/blob/main/CHANGELOG.md) · [Release notes](https://github.com/randlee/sc-observability/releases/tag/v1.5.0)

---

## Rust Developer

**As a Rust developer, I want reusable observability crates with standard consistent patterns, so that I can instrument my SC component without bespoke logging glue.**

v1.5.0 is a compatible 1.x release — every public API released in 1.4.1 keeps its signature, behavior, error variants, and serialization format, so upgrading is non-breaking. What changes is that each library crate now ships a canonical `v2` module: typed error families (`InitError`, `EventError`, `FlushError`, `LogSinkError`, `IdentityError`, and more) plus neutral telemetry signal records (spans, logs, metrics, profiles, state transitions) with validation in `sc-observability-types`. New code gets one consistent error-and-signal contract instead of the per-crate 1.x wrappers.

The logging surface gets the same treatment. A single serde-stable, binding-friendly `LogSettings` / `LogSettingsInputs` value now configures logging everywhere, with documented settings error codes. Typed sink registration (`SinkRegistration::typed`) and a canonical `v2::Logger` with `flush_with_timeout` / `shutdown_with_timeout` replace the ad-hoc 1.x lifecycle calls. There's also a host-owned logger attachment bridge (`attach_logger`) in `sc-observability-log` that routes `log` macros into an existing host logger, so a runtime that already owns a logger can adopt sc-observability's macros without spinning up a second runtime.

The 1.x facades are not going anywhere yet: they now delegate to the canonical implementation, isolated behind the default-on `v1` feature, and `cargo public-api` gates the candidate against the published 1.4.1 baseline. Deprecations are explicit and additive — `Logger::emit()` (use `log()`/`try_log()`), the legacy error wrappers, `OtlpEndpoint::new()`, `AuthHeader::new()`, `SpanAssembler::push()`, `TelemetryConfigBuilder::build()`, and `RetentionPolicy::max_age_days` are all still present but annotated. Build with `default-features = false` to prove a consumer has fully migrated off the `v1` surface.

---

## Python Developer

**As a Python developer, I want Python bindings for the same observability crates, so that Python-side SC tooling instruments consistently with the Rust side.**

The Python bindings now expose telemetry submission, not just logging. Release wheels enable the `otlp-telemetry` feature, so `pip install sc-observability` gives you a path to submit telemetry from Python, and flush, shutdown, and health calls release the GIL so they don't block your event loop. The wheel matrix also grows to six platforms with the new Windows ARM64 (`win_arm64`) wheel alongside the existing macOS, x86_64 Linux, and Windows targets — Python-side SC tooling on ARM64 Windows now installs a compiled wheel instead of building from source.

The same canonical v2 failure projection lands in the Python (and TypeScript/Tauri) bindings, so a failure that starts on the Rust side deserializes into the same shape on the Python side rather than a hand-mapped variant. The `otlp-telemetry`-enabled wheels are the release path for `SubmissionEnvelope` and `DurableTelemetryClient` from Python.

---

## Observability Developer

**As an observability developer, I want shared contracts (identifiers, diagnostics, health reports) stable across components, so that telemetry from different SC products composes cleanly.**

OTLP export gets a real backend story. A shared lifecycle core now composes two selectable backends through one exporter factory: `otlp-sdk` (the official OpenTelemetry SDK on Tokio, OTLP/HTTP protobuf) and `sync-http` (synchronous OTLP/HTTP JSON), with bounded SDK transport retries. Pick the backend that fits your runtime; the lifecycle contract stays the same. On top of that, the `SubmissionEnvelope` contract and a `DurableTelemetryClient` (feature `durable-store`) add a durable local store, drain, and OTLP/JSON encoders for every signal, honoring collector partial-success rejections so an export that partially succeeds is tracked accurately instead of dropped or double-sent.

The transport got a hardening pass in the same release: detached OTLP batches drain before shutdown, a failed terminal shutdown replays to later callers, log parent-span correlation is preserved, admitted export failures surface at flush, and SDK retries cancel after shutdown. `sc-observe` restores its released root configuration fields and registration signatures, and the canonical v2 failure projection reaches the DTO schema and every language binding — so telemetry and diagnostics compose cleanly across the whole SC ecosystem rather than drifting per product.

---

## Skipped / noted

- **TypeScript / Tauri (`sc-observability-tauri`, `@sc-observability/client`)** — the `sc-observability-tauri` adapter crate is published to crates.io and receives the canonical v2 failure projection, but the SSOT (`user-stories.md`) has no dedicated TypeScript/Tauri persona, so no section is included this release.

---

## What's Next

The 1.x `v1` surface (`Logger::emit()`, the legacy error wrappers, the `*_typed`-less constructors) remains behind the default-on feature and is slated for removal in a later release after this deprecation window. OTLP durable-store and the `sync-http` backend continue to mature alongside the `otlp-sdk` path, and the TypeScript client's npm publication is still pending the Phase C `sc-publish` migration.
