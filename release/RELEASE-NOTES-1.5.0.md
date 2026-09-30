# Release Notes — v1.5.0

This is the next compatible 1.x release candidate. It preserves the published
1.4.1 API and adds the compatible APIs described below. Publication and tagging
remain separately authorized.

## Compatibility

- All public APIs available in 1.4.1 remain available with their released
  signatures, behavior, error variants, and serialization formats.
- Deprecated APIs remain supported for this release. Removal requires a
  separately authorized major release.
- Additive APIs may be used alongside the retained 1.4.1 APIs.

## Deprecated APIs

The deprecated 1.4.x compatibility APIs remain available in 1.5.0:

- `Logger::emit()` remains available; use `Logger::log()` or
  `Logger::try_log()` for queue admission.
- Legacy error wrappers such as `IdentityError`, `InitError`, `EventError`,
  `FlushError`, `ShutdownError`, `ProjectionError`, `SubscriberError`,
  `LogSinkError`, and `ExportError` remain available; use their corresponding
  `sc_observability_types::typed` failures in new code.
- `OtlpEndpoint::new()`, `AuthHeader::new()`, `SpanAssembler::push()`, and
  `TelemetryConfigBuilder::build()` remain available; use their `*_typed`
  counterparts.
- `RetentionPolicy::max_age_days` remains available for direct file-sink
  configuration; logger-managed maintenance uses `RetainedLogPolicy`.

## Validation baseline

The nine candidate workspace API packages are checked against their published
1.4.1 packages. The separately published `sc-observability-tauri` crate is
excluded from this workspace API check; its recorded 1.4.1 baseline is not a
qualification result. Its standalone API/publication qualification remains
deferred in `release/bindings-artifacts.toml`. The blocking validator asserts
that the candidate and exactly approved deferred package sets cover every
published crate. No breaking API exceptions are accepted for this 1.x
candidate.
