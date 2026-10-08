# Adopting the compatible 1.x release

This release follows [ADR-020](../architecture.md#adr-020-compatible-1x-adoption-of-phase-d).
It is the next 1.x version of all nine crates. The released 1.4.1 public API is
retained, and the new implementation sits behind the released entry points;
the public-API and semver gates enforce that compatibility. Clean canonical APIs are opt-in under each
crate's `v2` module. Nothing is removed in 1.x.

Every Rust block in this guide runs under `rustdoc --test` against the
built workspace crates (see [Checking the examples](#checking-the-examples)).
The examples show API usage. They do not prove delivery to a live collector,
and they do not prove semver compatibility; the public-API and semver gates
own that.

## Stay on the released API

No source change is needed. The released root types, their error wrappers
and the `*_typed` methods added in 1.4.0 are all retained:

```rust
use std::path::PathBuf;

use sc_observability::{Logger, LoggerConfig, ServiceName};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let log_root = std::env::temp_dir().join(format!("sc-obs-mig-root-{}", std::process::id()));
let config = LoggerConfig::default_for(ServiceName::new("billing")?, log_root.clone());
let logger = Logger::new(config)?;
logger.flush()?;
let _stopped = logger.shutdown();
# std::fs::remove_dir_all(&log_root).ok();
# Ok(())
# }
```

The deprecation warnings this code prints are the ones 1.4.0 already
emitted. This release adds no new `#[deprecated]` item: the released
deprecated items kept their original notes, and most of them moved into
each crate's `compat` module. The deprecated error wrappers stay in
`sc-observability-types/src/errors.rs`, and
`sc_observability::RetentionPolicy::max_age_days` stays in
`sc-observability/src/lib.rs`. Code that builds at default lint settings keeps building.

## Opt in to `v2`

Four crates have a `v2` module: `sc-observability-types`, `sc-observability`,
`sc-observe` and `sc-observability-log`. The OTLP, DTO, binding-runtime, log-macros and log-consumer-check crates have no `v2`
module.

`v2` is the same runtime with canonical signatures, not a second
implementation. `v2` facades wrap the root state, and `v2` errors are the
ADR-017 named enums from `sc_observability_types::v2`. A migrated crate can
deny deprecated usage to prove it no longer touches the compatibility layer:

```rust
#![deny(deprecated)]

use sc_observability::v2::{EventError, Logger, LoggerConfig};
use sc_observability::{
    ActionName, Level, LogEvent, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity, SchemaVersion,
    ServiceName, TargetCategory, Timestamp,
};

# fn event(service: &ServiceName) -> Result<LogEvent, Box<dyn std::error::Error>> {
#     Ok(LogEvent {
#         version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)?,
#         timestamp: Timestamp::UNIX_EPOCH,
#         level: Level::Info,
#         service: service.clone(),
#         target: TargetCategory::new("billing.invoice")?,
#         action: ActionName::new("issue")?,
#         message: Some("invoice issued".to_owned()),
#         identity: ProcessIdentity::default(),
#         trace: None,
#         request_id: None,
#         correlation_id: None,
#         outcome: None,
#         diagnostic: None,
#         state_transition: None,
#         fields: Default::default(),
#     })
# }
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let log_root = std::env::temp_dir().join(format!("sc-obs-mig-v2-{}", std::process::id()));
let service = ServiceName::new("billing")?;
let logger = Logger::new(LoggerConfig::default_for(service.clone(), log_root.clone()))?;

match logger.try_log(event(&service)?) {
    Ok(()) => {}
    Err(error @ EventError::Validation { .. }) => eprintln!("rejected: {}", error.code()),
    // The canonical enums are #[non_exhaustive].
    Err(other) => return Err(other.into()),
}
logger.flush()?;
let _stopped = logger.shutdown();
# std::fs::remove_dir_all(&log_root).ok();
# Ok(())
# }
```

Match named variants for control flow and read the stable code from the
carried `ErrorContext`. Every named variant owns the original boxed context,
so the code, remediation, source chain and backtrace survive. Do not rebuild
an error from its display text. [Error API migration](../migrate-error-api.md)
lists the nine families and their named causes.

### Where each released entry point moved

| Crate | Released (root) | `v2` |
| --- | --- | --- |
| `sc-observability-types` | nine tuple wrappers (`InitError`, `FlushError`, ...) and `typed::*Failure` | same-name named enums, `v2::{ProcessIdentityResolver, LogProjector, ...}` returning them |
| `sc-observability` | `Logger`, `LoggerBuilder`, `LogError`/`LogFailure` results, `TypedLogSink` | `v2::{Logger, LoggerBuilder, LogSink}` with `InitError`, `EventError`, `FlushError`, `LogSinkError` |
| `sc-observe` | `ObservabilityConfig`, `Observability`, `ObservabilityBuilder` with released errors | `v2::{ObservabilityConfig, Observability, ObservabilityBuilder}` with `InitError`, `FlushError`, `ShutdownError` |
| `sc-observability-log` | `init`, `LogGuard`, `LogControl`, local lifecycle error enums | `v2::{init, LogGuard, LogControl}` with shared `InitError`, `FlushError`, `ShutdownError` |

Independently constructing a root `Logger` and a `v2::Logger` creates two
loggers, each with its own runtime, queue and lifecycle. Consuming a logger
or builder through `From` instead preserves its existing inner state; it
changes the public surface without constructing another runtime. Available
conversions and shared views include:

- `From` in both directions between `sc_observability::Logger<State>` and
  `sc_observability::v2::Logger<State>`, preserving the inner logger and state;
- `From` in both directions between `sc_observability::LoggerBuilder` and
  `sc_observability::v2::LoggerBuilder`, preserving the builder configuration;
- `sc_observability_log::LogControl::into_v2`, which returns a `v2` control
  over the same process-wide bridge;
- `sc_observe::v2::ObservabilityConfig`, which dereferences to the root
  `ObservabilityConfig`.

So migrate at the point that owns the runtime: the code that builds and
shuts down a logger, observability instance, telemetry pipeline or log
bridge moves to `v2` together with the code that uses that value.

### Observation routing

```rust
#![deny(deprecated)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sc_observability_types::v2::{ObservationSubscriber, SubscriberError, SubscriberRegistration};
use sc_observability_types::{Observation, ServiceName, ToolName};
use sc_observe::v2::{Observability, ObservabilityConfig};

struct InvoiceIssued {
    amount_cents: u64,
}

#[derive(Default)]
struct Totals(AtomicUsize);

impl ObservationSubscriber<InvoiceIssued> for Totals {
    fn observe(&self, observation: &Observation<InvoiceIssued>) -> Result<(), SubscriberError> {
        self.0.fetch_add(observation.payload.amount_cents as usize, Ordering::Relaxed);
        Ok(())
    }
}

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let log_root = std::env::temp_dir().join(format!("sc-obs-mig-observe-{}", std::process::id()));
let totals = Arc::new(Totals::default());
let config = ObservabilityConfig::default_for(ToolName::new("billing")?, log_root.clone())?;
let observability = Observability::builder(config)
    .register_subscriber(SubscriberRegistration::new(totals.clone()))
    .build()?;

observability.emit(Observation::new(
    ServiceName::new("billing")?,
    InvoiceIssued { amount_cents: 1250 },
))?;
assert_eq!(totals.0.load(Ordering::Relaxed), 1250);
observability.flush()?;
observability.shutdown()?;
# std::fs::remove_dir_all(&log_root).ok();
# Ok(())
# }
```

The `v2` builder takes the `v2` registrations, whose open traits return the
canonical `SubscriberError` and `ProjectionError`. Registration stays
construction-time only, and a build with no route is rejected.

### Host logger bridge

```rust
#![deny(deprecated)]

use std::time::Duration;

use sc_observability_log::v2::{self, BridgeOptions};
use sc_observability_log::{ActionName, LoggerConfig, ServiceName};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let log_root = std::env::temp_dir().join(format!("sc-obs-mig-log-{}", std::process::id()));
let mut config = LoggerConfig::default_for(ServiceName::new("billing")?, log_root.clone());
config.enable_console_sink = false;
let guard = v2::init(
    config,
    BridgeOptions {
        default_action: ActionName::new("billing.record")?,
        parse_bracket_action: false,
    },
)?;
guard.flush(Duration::from_secs(5))?;
guard.shutdown(Duration::from_secs(5))?;
# std::fs::remove_dir_all(&log_root).ok();
# Ok(())
# }
```

`v2::init` installs the same process-wide bridge as the root `init`; a
process still has one host logger. Detaching an attachment never grants
ownership of the host logger.

## OTLP

Phase H removed the OTLP facade without a compatibility path (H-006):
the `v1`/`v2` `Telemetry`, `RuntimeTelemetry`, `OtelConfig` and span assembly
types, and the `otlp-sdk`/`sync-http` backends. OTLP users call the official
OpenTelemetry SDK through `sc_observability_otlp::{api, sdk}`: a Tokio host
bridges logging with `OtelLogSink` (feature `log-sink`), and a synchronous
host sends logs, spans and metrics with `sync::Client` (feature
`synchronous-client`). ADR-023 records the contract.

## What a later major release removes

ADR-020 keeps the deprecated compatibility paths available throughout the
1.5.0 compatible release. Consumers should migrate to canonical v2 paths
before a future major release removes the compatibility layer. The layer is
kept apart so its removal does not touch the canonical implementation:

- the `compat` modules of `sc-observability`, `sc-observe` and
  `sc-observability-log`, and the nine released
  wrappers in `sc-observability-types/src/errors.rs`;
- the root re-exports of those items;
- the named per-item deletion points in the `removable_paths` and
  `removal_point` fields of
  [`docs/compatibility/registry.json`](../compatibility/registry.json).

ADR-020 requires that the canonical implementation not depend on those
modules. Moving to
`v2` names and denying deprecated usage now is the preparation that release
will need; no other migration step is required in 1.x.

## Checking the examples

Build the four crates, then pass each library to
`rustdoc`:

```sh
cargo build --locked -p sc-observability-types -p sc-observability -p sc-observe \
  -p sc-observability-log --message-format=json
rustdoc --test --edition 2024 -L dependency=target/debug/deps \
  --extern sc_observability=<rlib> --extern sc_observability_types=<rlib> \
  --extern sc_observe=<rlib> --extern sc_observability_log=<rlib> \
  docs/migration/compatible-1x.md
```

Take each `<rlib>` from the `filenames` of the matching `compiler-artifact`
message. Every block must pass.
