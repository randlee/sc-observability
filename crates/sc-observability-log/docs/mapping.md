# Bridge mapping and native control contract

This is the current B.P3 bridge contract. Historical Phase A documents retain
the evidence for the superseded unpublished adapter surface; they are not a
description of exported API.

## Producers and one writer

`log` facade records, `trace!`/`debug!`/`info!`/`warn!`/`error!`/`event!`
macros, `#[instrument]`, and `LogControl::try_log(BridgeEvent)` enter the same
guarded core and staged `Logger`. Facade and macro entry points intentionally
discard their result only after exact-once drop accounting. `try_log` returns
`Result<AdmissionOutcome, EmitError>`.

| Producer input | Bridge result |
| --- | --- |
| `log::Record` | Sanitized target/action/message/fields, then bridge-owned envelope |
| macro/instrument call | Same mapping plus current trace context and completion fields |
| `BridgeEvent` | Typed level/target/action/message/outcome/fields/request/correlation/trace; bridge fills version, timestamp, service, identity, routing and default action |

The selected level is checked before queue admission. `Filtered` is neither
queued nor counted; `Accepted` means successful nonblocking queue admission,
not durable persistence.

## Field keys

Every producer uses `field_key_label`: `::` becomes `.`, invalid label
characters become `_`, and `sc_observability_log.` is reserved. Literal invalid
macro keys are compile errors. Invalid dynamic facade/macro keys are omitted and
the record is counted as `InvalidEvent`. A direct empty or reserved key, or a
normalization collision, rejects the whole `BridgeEvent` with
`EmitError::InvalidField` and one `InvalidEvent` drop. `code.*` bridge fields
remain authoritative; displaced facade values are recorded under
`sc_observability_log.shadowed_fields`.

## Control and lifecycle

`LogGuard` owns final shutdown. `LogControl` is non-owning and exposes:

```text
try_log, query, flush, health, active_log_path, dropped_events, wait_stopped
```

It cannot mutate levels or shut down the bridge. `LogGuard` alone owns
`elevate_level` and `reset_level`.

Lifecycle is `running`, `stopping`, `stopped`, or `failed`. Owner shutdown
starts exactly one reserved worker. A timeout returns `ShutdownError::TimedOut`
to that caller but leaves the original operation running; controls may call
`wait_stopped` repeatedly and receive its one saved report after completion.
Confirmed final-flush failure is distinct from unconfirmed worker failure.
Health, path, counters, and runtime level state remain inspectable after stop.
Query and flush return a typed `NotRunning` error outside `running`.

At most one native flush is in flight. A concurrent request returns
`FlushError::InProgress`; a timed-out flush remains the same in-flight operation
until it ends.

## Health

`BridgeHealthReport` is serde data with schema version 1:

| Field | Meaning |
| --- | --- |
| `logging` | Unmodified staged-core `LoggingHealthReport` |
| `dropped` | Bridge exact-once `DroppedEvents` counters |
| `lifecycle` | Native bridge lifecycle phase |
| `active_log_path` | Init-cached JSONL path, if the file sink is enabled |
| `configured_level`, `effective_level`, `level_revision` | Coherent staged-core runtime level state, retained after shutdown |

## Errors and registry

The public operation error families are `InitError`, `FlushError`,
`ShutdownError`, `EmitError`, `ControlError`, and `WaitError`. Each is tagged
serde data, implements `Display` and `Error` with no error source, and exposes
`code()` and `remediation()`. Wrapped core failures copy the underlying
`OperationDiagnostic` code, message, remediation, and timestamp exactly.

`error_codes::ALL` contains only the accepted bridge registry: the seven
baseline install/timeout/helper codes plus `UNSUPPORTED_LEVEL`,
`RUNTIME_START_FAILED`, `NOT_RUNNING`, `INVALID_FIELD`, `REENTRANT_EMIT`,
`LOGGER_PANICKED`, `STATUS_UNAVAILABLE`, `SHUTDOWN_NOT_STARTED`, and
`FLUSH_IN_PROGRESS`. Retired `SUBMIT_*` and `FLUSH_AFTER_SHUTDOWN` codes are not
part of the native contract.

## Evidence

`tests/one_writer.rs` proves cross-producer guard and accounting behavior.
`tests/shutdown_timeout.rs` proves timeout then late completion and repeated
waiters. `tests/reinstall_subprocess.rs` proves install-once, foreign logger,
and post-stop direct rejection. `tests/runtime_level_bridge.rs`,
`tests/health_snapshot.rs`, `tests/flush_single_flight.rs`, consumer-check,
compile-fail macro fixtures, and native-error fixtures cover the remaining
public contract boundaries.
