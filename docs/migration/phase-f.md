# Phase F consumer migration guide

Phase F retires the 1.x facade in favour of the canonical 2.0 surface. This
guide is the consumer migration record for deferred landing-ledger work. A
listed 1.x item is removed only after every consumer line named for it has
migrated.

## Release and feature rule

The next release keeps released 1.x paths behind each crate's default-on `v1`
feature and marks them deprecated. A consumer proves that it has completed its
migration by building with `default-features = false` (and therefore without
`v1`). A later release deletes the `v1` modules and features. Deprecated paths
are not a compatibility promise: move to the canonical replacement below.

## Lifecycle consolidation

The canonical lifecycle handles use a shared-reference shutdown contract. The
no-argument operations use the configured lifecycle deadline; callers that
need a distinct deadline use the corresponding `_with_timeout` method. The
retained v1 items below are deprecated compatibility aliases.

| Changed item | Canonical replacement |
| --- | --- |
| v1 `LogGuard::flush(Duration)` | `sc_observability_log::v2::LogGuard::flush()` or `flush_with_timeout(Duration)` |
| v1 `LogGuard::shutdown(self, Duration)` | `sc_observability_log::v2::LogGuard::shutdown()` or `shutdown_with_timeout(Duration)` |
| v1 `LogControl::flush(Duration)` | `sc_observability_log::v2::LogControl::flush()` or `flush_with_timeout(Duration)` |
| v2 `LogGuard::flush(Duration)` | `LogGuard::flush()` or `LogGuard::flush_with_timeout(Duration)` |
| v2 `LogGuard::shutdown(self, Duration)` | `LogGuard::shutdown()` or `LogGuard::shutdown_with_timeout(Duration)` |
| v2 `LogControl::flush(Duration)` | `LogControl::flush()` or `LogControl::flush_with_timeout(Duration)` |
| v1 `Logger::shutdown(self) -> Logger<Stopped>` | `sc_observability::v2::Logger::shutdown()` or `shutdown_with_timeout(Duration)` |
| v2 `Logger<State>` (`Running`/`Stopped` typestate) | `Logger` (no type parameter) |
| v2 `Logger::shutdown(self) -> Logger<Stopped>` | `Logger::shutdown()` or `Logger::shutdown_with_timeout(Duration)` |
| v2 `Logger::flush()` (configured deadline only) | `Logger::flush()` or `Logger::flush_with_timeout(Duration)` |
| v2 `sc_observe::v2::Observability::flush()` (configured deadline only) | `Observability::flush()` or `Observability::flush_with_timeout(Duration)` |
| v2 `sc_observe::v2::Observability::shutdown()` (configured deadline only) | `Observability::shutdown()` or `Observability::shutdown_with_timeout(Duration)` |

## Deferred ledger rows

The 68 entries below are the deferred consumer-migration entries from the
landing ledger. `Location` is the consumer source line recorded by the ledger
at its checked revision; it is a review aid, not a promise that downstream
repositories have stayed still.

### atm-core

| Entry | 1.x item | Canonical replacement | Consumer location |
| --- | --- | --- | --- |
| F7-01 | `typed::InitFailure` | `sc_observability_types::v2::InitError` | `crates/atm/src/main.rs:343,347` |
| F7-02 | `typed::EventFailure` | `sc_observability_types::v2::EventError` | `crates/atm/src/main.rs:1129` |
| F7-03 | `typed::FlushFailure` | `sc_observability_types::v2::FlushError` | `crates/atm/src/main.rs:592` |
| F7-04 | `LogFailure` | `sc_observability_types::v2::EventError` | `crates/atm/src/main.rs:573-577` |
| F7-05 | `TryLogFailure` | `sc_observability_types::v2::EventError` | `crates/atm-observability/src/lib.rs:117-121,300-305` |
| F7-06 | `LogSinkError` | `sc_observability_types::v2::LogSinkError` | `crates/atm/src/main.rs:459,467` |
| F7-07 | root `LogSink` | `sc_observability::v2::LogSink` | `crates/atm/src/main.rs:43,432-468` |
| F7-08 | `SinkRegistration::new` | `SinkRegistration::typed` | `crates/atm/src/main.rs:337,425` |
| F7-09 | root `LoggerBuilder` | `sc_observability::v2::LoggerBuilder` | `crates/atm/src/main.rs:45,335-343` |
| F7-10 | `Logger::builder_typed` | `v2::LoggerBuilder` | `crates/atm/src/main.rs:335` |
| F7-11 | `LoggerBuilder::register_sink` | `v2::LoggerBuilder::register_sink` | `crates/atm/src/main.rs:337,425` |
| F7-12 | `LoggerBuilder::build_typed` | `v2::LoggerBuilder::build` | `crates/atm/src/main.rs:343` |
| F7-13 | root `Logger` | `sc_observability::v2::Logger` | `crates/atm/src/main.rs:46,323,479-544` |
| F7-14 | `Logger::log_typed` | `v2::Logger::log` | `crates/atm/src/main.rs:519` |
| F7-15 | `Logger::try_log_typed` | `v2::Logger::try_log_with_outcome` | `crates/atm-observability/src/lib.rs:117` |
| F7-16 | `Logger::flush_typed` | `v2::Logger::flush` | `crates/atm/src/main.rs:507,520` |
| F7-17 | `Logger::query` | `v2::Logger::query` | `crates/atm/src/main.rs:525` |
| F7-18 | `Logger::follow` | `v2::Logger::follow` | `crates/atm/src/main.rs:533,537` |
| F7-19 | `Logger::health` | `v2::Logger::health` | `crates/atm/src/main.rs:544` |
| F7-20 | `Logger::shutdown` | `v2::Logger::shutdown` | `crates/atm/src/main.rs:502` |
| F7-21 | `RotationPolicy` | `RetainedLogPolicy::{rotation_max_bytes,rotation_max_files}` | `crates/atm/src/main.rs:48,420-424` |
| F7-22 | `RetentionPolicy` | `RetainedLogPolicy::retention_max_age` | `crates/atm/src/main.rs:48,423` |
| F7-23 | `JsonlFileSink::new` | `JsonlFileSink::open(path, RetainedLogPolicy)` | `crates/atm/src/main.rs:420-424` |
| F7-24 | legacy sink maintenance policy | writer-owned `RetainedLogPolicy` maintenance | `crates/atm/src/main.rs:420` |
| F7-25 | `fault-injection` feature enable | remove it; atm-core has its own test sink | `crates/atm/Cargo.toml:24,92` |
| F7-26 | `sc_observability_log::InitError` | `sc_observability_log::v2::InitError` | `crates/atm-observability/tests/macro_qualification.rs:110-117` |
| F7-27 | `sc_observability_log::FlushError` | `sc_observability_log::v2::FlushError` | `crates/atm-observability/tests/macro_qualification.rs:141` |
| F7-28 | `sc_observability_log::ShutdownError` | `sc_observability_log::v2::ShutdownError` | `crates/atm-observability/tests/macro_qualification.rs:258-260` |
| F7-29 | root `sc_observability_log::init` | `sc_observability_log::v2::init` | `crates/atm-observability/tests/macro_qualification.rs:110` |
| F7-30 | `LogGuard::flush` | `v2::LogGuard::flush` | `crates/atm-observability/tests/macro_qualification.rs:141` |
| F7-31 | `LogGuard::shutdown` | `v2::LogGuard::shutdown` | `crates/atm-observability/tests/macro_qualification.rs:258-260` |
| F7-32 | `LogGuard::dropped_events` | `v2::LogGuard::dropped_events` | `crates/atm-observability/tests/macro_qualification.rs:257` |
| F7-33 | root `OBSERVATION_ENVELOPE_VERSION` construction | `constants::OBSERVATION_SCHEMA_VERSION.clone()` | `crates/atm/src/main.rs:696` |
| F7-34 | root `LoggerConfig` construction path | canonical logger config via `v2::LoggerBuilder` | `crates/atm-observability/src/lib.rs:226-251` |
| F7-35 | released root facade imports | explicit `v2` imports only | `crates/atm/src/main.rs:43-55` |

### sc-compose

| Entry | 1.x item | Canonical replacement | Consumer location |
| --- | --- | --- | --- |
| F7-36 | `InitError` wrapper | `sc_observability_types::v2::InitError` | `crates/sc-compose/src/observability/logger.rs:22` |
| F7-37 | `EventError` wrapper | `sc_observability_types::v2::EventError` | `crates/sc-compose/src/observer_impl.rs:607,731` |
| F7-38 | `LogSinkError` wrapper | `sc_observability_types::v2::LogSinkError` | `crates/sc-compose/src/observer_impl.rs:923,970` |
| F7-39 | root `LogSink` | `sc_observability::v2::LogSink` | `crates/sc-compose/src/observer_impl.rs:583,923,970` |
| F7-40 | `SinkRegistration::new` | `SinkRegistration::typed` | `crates/sc-compose/src/observability/logger.rs:26` |
| F7-41 | root `LoggerBuilder` | `sc_observability::v2::LoggerBuilder` | `crates/sc-compose/src/observability/logger.rs:22-28` |
| F7-42 | root `Logger` | `sc_observability::v2::Logger` | `crates/sc-compose/src/main.rs:25` |
| F7-43 | `Logger::log` facade entry | `v2::Logger::log` | `crates/sc-compose/src/observer_impl.rs:129` |
| F7-44 | `Logger::try_log` facade entry | `v2::Logger::try_log_with_outcome` | `crates/sc-compose/src/observer_impl.rs:731` |
| F7-45 | 1.x logging error matching | `v2::EventError` diagnostic matching | `crates/sc-compose/src/observer_impl.rs:607,731` |
| F7-46 | 1.x sink error matching | `v2::LogSinkError` diagnostic matching | `crates/sc-compose/src/observer_impl.rs:923,970` |
| F7-47 | facade builder conversions | canonical builder construction | `crates/sc-compose/src/observability/logger.rs:22-28` |
| F7-48 | facade logger health path | `v2::Logger::health` | `crates/sc-compose/src/observer_impl.rs:63-128` |
| F7-49 | facade logger shutdown path | `v2::Logger::shutdown` | `crates/sc-compose/src/observer_impl.rs:63-128` |
| F7-50 | root `sc-observability` imports | explicit `sc_observability::v2` imports | `crates/sc-compose/src/main.rs:25` |
| F7-51 | root types error imports | explicit `sc_observability_types::v2` imports | `crates/sc-compose/src/observability/logger.rs:22` |

### beads-task-issue-tracker

| Entry | 1.x item | Canonical replacement | Consumer location |
| --- | --- | --- | --- |
| F7-52 | root `sc_observability_log::init` | `sc_observability_log::v2::init` | `crates/btit-app/src/logging.rs:19` |
| F7-53 | root `InitError` | `sc_observability_log::v2::InitError` | `crates/btit-app/src/logging.rs:73` |
| F7-54 | root `LogControl` | `sc_observability_log::v2::LogControl` | `crates/sc-observability-log-consumer-check/src/lib.rs:14,22` |
| F7-55 | root `BridgeHealthReport` | `sc_observability_log::v2::BridgeHealthReport` | `crates/sc-observability-log-consumer-check/src/lib.rs:22` |
| F7-56 | root `FailureReport` | `sc_observability_log::v2::BridgeHealthReport.helpers` | `crates/sc-observability-log-consumer-check/src/lib.rs:22` |
| F7-57 | root `LoggerConfig` | `sc_observability_log::v2::LoggerConfig` | `crates/sc-observability-log/tests/compat_events.rs:15` |
| F7-58 | root `BridgeOptions` | `sc_observability_log::v2::BridgeOptions` | `crates/sc-observability-log/tests/compat_events.rs:15` |
| F7-59 | root `LevelFilter` | `sc_observability_log::v2::LevelFilter` | `crates/sc-observability-log/tests/compat_events.rs:15` |
| F7-60 | root `ServiceName` | `sc_observability_log::v2::ServiceName` | `crates/sc-observability-log/tests/compat_events.rs:15` |
| F7-61 | root `ActionName` | `sc_observability_log::v2::ActionName` | `crates/sc-observability-log/tests/compat_events.rs:15` |
| F7-62 | root `Level` | `sc_observability_log::v2::Level` | `crates/sc-observability-log-consumer-check/src/lib.rs:6` |
| F7-63 | root event macros | `sc_observability_log::v2::{debug,error,event,info,instrument,trace,warn}` | `crates/sc-observability-log-consumer-check/src/lib.rs:6` |
| F7-64 | direct `sc_observability::Logger` | `sc_observability::v2::Logger` | `crates/sc-observability-log/src/health.rs:271` |
| F7-65 | direct `sc_observability_types::InitError` | `sc_observability_types::v2::InitError` | `crates/sc-observability-log/src/error.rs:42` |
| F7-66 | direct `sc_observability_types::FlushError` | `sc_observability_types::v2::FlushError` | `crates/sc-observability-log/src/error.rs:60,95` |
| F7-67 | direct `sc_observability_types::IdentityError` | `sc_observability_types::v2::IdentityError` | `crates/sc-observability-log/src/error.rs:35` |
| F7-68 | root `sc-observability-log` facade imports | explicit `v2` imports only | `crates/btit-app/src/logging.rs:19` |

Once the locations named for an entry have migrated, that consumer must prove
the result with its dependency on the corresponding crate configured with
`default-features = false`. The final feature removal belongs to the later
release, not to an individual consumer migration.
