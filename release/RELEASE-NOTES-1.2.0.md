# Release Notes — v1.2.0

## Summary

Phase A: writer-thread optimization and public API governance.

- Locked writer-thread architecture and public logging contract (A.1)
- CI-visible public API diff and semver governance gates (A.2)
- Queue-backed writer runtime; `Logger::log()` and `Logger::try_log()` replace caller-thread writes; `Logger::emit()` deprecated with compatibility path (A.3)
- Consumer documentation and `custom-sink-example` updated for new logging API and health inspection (A.4)

## Included Crates

- `sc-observability-types` v1.2.0
- `sc-observability` v1.2.0
- `sc-observe` v1.2.0
- `sc-observability-otlp` v1.2.0

## Compatibility Notes

- `Logger::emit()` is deprecated but remains callable; migrate to `log()` or `try_log()`
- `log()` blocks until the entry is enqueued; `try_log()` returns `Err(TryLogError::QueueFull)` on a saturated queue
- `LoggingHealthReport` now includes queue and writer health fields
- `LogSink`, `LogFilter`, and `Redactor` traits carry explicit open-contract doc comments
