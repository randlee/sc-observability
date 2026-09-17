# B.1 exported API/impl inventory reconciliation

Fix-round evidence for aobs completeness finding B1-C02 (`phase-b-b1-copy`):
"Verify the complete exported API/impl inventory against the approved target
matrix, including additions made after the provisional source inspection"
(sprint-b-1-copy.md deliverable 3).

## Generation

```sh
cargo public-api --manifest-path crates/sc-observability-log/Cargo.toml \
  > docs/plans/phase-b/evidence/b1-log-export.txt
cargo public-api --manifest-path crates/sc-observability-log-macros/Cargo.toml \
  > docs/plans/phase-b/evidence/b1-log-macros-export.txt
```

Run at commit `c0a4cddaede02a7e3234bc48c421cd56be01660b` under this workspace's
pinned toolchain (rustc 1.94.1). `sc-observability-log` exports 562 public
items (structs/enums/consts/fns/impls, `#[doc(hidden)]` items excluded by the
tool as designed); `sc-observability-log-macros` exports 7 (the module plus
six proc macros and `#[instrument]`). Full raw listings are checked in at
`docs/plans/phase-b/evidence/b1-log-export.txt` and
`b1-log-macros-export.txt`.

## Reconciliation against `target-bridge-api.md`'s "Existing exported API" table

Every row was checked against the generated listing; the accepted
implementation matches the target's stated disposition in each case:

| Target row | Found in export | Notes |
| --- | --- | --- |
| `init` | yes (`fn sc_observability_log::init`) | signature matches `(LoggerConfig, BridgeOptions) -> Result<LogGuard, InitError>` |
| `BridgeOptions` | yes | |
| `LoggerConfig` re-export | yes (`pub use sc_observability_log::LoggerConfig`) | |
| `ActionName`, `ErrorCode`, `LevelFilter`, `ProcessIdentityPolicy`, `Remediation`, `ServiceName`, `TargetCategory` | yes, all re-exported | |
| `Level` + TRACE/DEBUG/INFO/WARN/ERROR consts, Clone/Eq/Debug, `From<Level>` into core `Level` | yes | no Ord/PartialOrd impl present, as required |
| `trace!`/`debug!`/`info!`/`warn!`/`error!`/`event!`/`#[instrument]` at bridge root and macros-crate root | yes, both places | `pub use sc_observability_log_macros::{debug, error, event, info, instrument, trace, warn}` in `src/lib.rs`; proc macros themselves listed in the macros-crate export |
| `DroppedEvents` get/total | yes | |
| `DropCause` ALL + 7 variants | yes, `pub const DropCause::ALL: [DropCause; 7]`, all 7 variants present | |
| `DEFAULT_DROP_SHUTDOWN_TIMEOUT` | yes | |
| `LogGuard` (Debug, must_use, !Clone) | yes | |
| `LogGuard::elevate_level` / `reset_level` | yes, both present | |
| `LogGuard::flush` / `shutdown` / `dropped_events` / `active_log_path` | yes, all four present | |
| `InitError` incl. `RuntimeStart`/`UnsupportedLevel` | yes, both new variants present alongside `AlreadyInitialized`/`ForeignLoggerInstalled`/`IdentityResolution`/`Logger` | |
| `FlushError` incl. `NotRunning`/`InProgress` | yes, both new variants present | |
| `ShutdownError`, four named cases | yes: `FinalFlush`, `HelperLost`, `HelperSpawn`, `TimedOut` | |
| `error_codes` module, ALL + registry | yes: `ALL` plus 16 named `SC_OBSERVABILITY_LOG_*` constants (the original 7 plus new cases added for the extended registry, per the target's "extend registry" instruction) | |
| `__private` module | present in source (`#[doc(hidden)] pub mod __private` at `src/lib.rs:525`), correctly absent from the public-facing export listing since the tool excludes `#[doc(hidden)]` items by design | matches "retain as hidden macro support," not a supported adapter API |
| CI-only consumer-check crate | `crates/sc-observability-log-consumer-check/Cargo.toml` depends only on `sc-observability-log` (path dependency), `publish = false` | matches "depends directly only on bridge, never publish" |

## Reconciliation against the "Snapshot reconciliation and disposition" table

The observed-source family names in that table describe BTIT's pre-redesign
source; the target's disposition for each family is "revise," and the
accepted implementation's actual shape was checked instead of the old names:

- Control/event surface (`JsonMap`/`JsonValue`/`LogControl`/`StructuredRecord`/`SubmitOutcome` family): revised to `BridgeEvent` (55 export lines) and `EmitOutcome`, matching "revise to the proposed direct-result `BridgeEvent`/`EmitOutcome` control surface."
- Error/drop-accounting family (`SubmitError`/`InvalidInputReason`/`DropCause`/`DroppedEvents`): revised to `EmitError` (with nested `FieldKeyError`, matching the target's explicit note that the source snapshot has no `FieldKeyError` and it is an intentional pre-copy addition) plus the retained `DropCause`/`DroppedEvents` family.
- Health/completion family (`BridgeHealthReport`/`BridgeHealthState`/`BridgeLifecycle`/`HelperHealth`/`LoggerHealth`/`QueueHealth`/`FileSinkHealth`/`SinkHealthSnapshot`/`SinkStatus`/`WriterStatus`/`HealthDiagnostic`/`BRIDGE_HEALTH_SCHEMA_VERSION`): consolidated into `BridgeHealthReport` and re-exported `LoggingHealthReport`/`BRIDGE_HEALTH_SCHEMA_VERSION`, matching "revise to the proposed bridge health/completion projection." The many granular old type names are gone by design (consolidation), not a missing-export gap.
- `FailureReport`/`Failure`/`InitFailure`/`FlushFailure`/`ShutdownFailure`/`CONTROL_SCHEMA_VERSION`: replaced by the native `InitError`/`FlushError`/`ShutdownError` result/error projection described above; no source report shape survives as a compatibility baseline, as required.
- `Timestamp` and neutral re-exports: `Timestamp`, `TraceContext`, `CorrelationId`, `AdmissionOutcome`, etc. all present as re-exports; runtime-level value/error re-exports (`LevelChange`, `LevelChangeError`, `LevelChangeSource`, `LevelState`) also present, matching "add only the target's explicitly listed runtime-level values/errors."
- Root macros: covered above.
- `Level`/`BridgeOptions`/`LogGuard`/`init`/`DEFAULT_DROP_SHUTDOWN_TIMEOUT`/`error_codes`: covered above.
- `__private` reachable support (callsite/context/mapping helpers): present under `sc_observability_log::callsite` (e.g. `FieldDebug` trait referenced throughout the export as a supertrait bound); hidden per design, inventoried here rather than treated as a public compatibility surface.

## Conclusion

No target-matrix row is unaccounted for in the accepted, copied
implementation; every observed-source family has an explicit disposition
realized in the actual export. This reconciliation is scoped to confirming
presence/shape against the target matrix, not a new API design review.
