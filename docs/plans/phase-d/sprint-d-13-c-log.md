# d-13: Logging contract

## Plan metadata

- Wave: 2
- Branch: `sprint/d-13-c-log`
- PR target: `sprint/d-12-c-types`
- Blocked by: `obs-phase-d-plan-qa`
- Owned paths:
  - `crates/sc-observability/src/settings.rs`
  - `crates/sc-observability/src/typed.rs`
  - `crates/sc-observability-log/src/lib.rs`
  - `crates/sc-observability-types/src/typed.rs`

## Deliverables

1. Add the public `LogSettings`, `ResolvedLogSettings`, `EnvSnapshot`, and `LogSettingsInputs` source/resolved configuration contract, with serde behavior and stable `LogSettingsError` codes from D.1.
2. Add `AttachmentOptions`, the open `BridgeEventPolicy` trait, `BridgeEventDecision`, `PolicyRejection`, and non-owning `LogAttachment` contract from D.2.
3. Add `SinkRegistration::typed(Arc<dyn TypedLogSink>) -> Self` and `LoggerBuilder::register_typed_sink(...)` public signatures from D.3.

```rust
pub trait BridgeEventPolicy: Send + Sync { fn decide(&self, event: &LogEvent) -> BridgeEventDecision; }
pub fn attach_logger(logger: Arc<Logger>, options: AttachmentOptions) -> Result<LogAttachment, InitError>;
pub fn register_typed_sink(&mut self, sink: Arc<dyn TypedLogSink>) -> Result<&mut Self, SinkRegistrationError>;
```

4. Remove typed Failure duplicates and `impl_legacy_classification!`; document the retained D3 `TypedLogSink`/`legacy_sink` adapter boundary.

## This Sprint Does Not Close

Environment resolution, bridge lifecycle, and typed-sink implementation are closed by D.1, D.2, and D.3 respectively.


## Design

## Public contract

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct LogSettings {
    pub level: Option<LevelFilter>,
    pub log_root: Option<PathBuf>,
    pub enable_file_sink: Option<bool>,
    pub enable_console_sink: Option<bool>,
    pub retained_log_policy: Option<RetainedLogPolicy>,
}

pub struct ResolvedLogSettings {
    pub level: LevelFilter,
    pub log_root: PathBuf,
    pub enable_file_sink: bool,
    pub enable_console_sink: bool,
    pub retained_log_policy: RetainedLogPolicy,
}

pub struct EnvSnapshot { /* selected environment captured once */ }

impl LogSettings {
    pub fn from_env(snapshot: &EnvSnapshot, prefix: EnvPrefix) -> Result<Self, LogSettingsError>;
    pub fn resolve(inputs: LogSettingsInputs) -> Result<ResolvedLogSettings, LogSettingsError>;
}

pub struct LogSettingsInputs {
    pub file: Option<LogSettings>,
    pub shared_env: LogSettings, // resolved from EnvSnapshot
    pub application_env: Option<LogSettings>, // resolved from EnvSnapshot
    pub default_root: PathBuf,
}

impl ResolvedLogSettings {
    pub fn into_logger_config(self, service_name: ServiceName) -> LoggerConfig;
}
```

The contract reuses existing owners: `LevelFilter` is the legal type owned by
`sc-observability-types`; `EnvPrefix` supplies its existing validation and
normalization rules, and `RetainedLogPolicy` supplies strong policy values and
millisecond duration semantics. D.1 must not add `settings_level_wire`,
`LogEnvPrefix`, or an overrides type. `retainedLogPolicy` is atomic: when
supplied, it replaces the policy as one validated value; an absent or `null`
value means no policy override. D.1 does not provide field-wise nested overlays
or add a second policy codec.

The shared namespace is `EnvPrefix::new("SC")`; an application such as BTIT
uses `EnvPrefix::new("BTIT")` and maps to `BTIT_LOG_*`. Prefixes follow the
existing `EnvPrefix` contract (no caller-supplied trailing underscore).
Resolution is normally `defaults < JSON < SC_ environment < application
environment`. To preserve LOG-009, an explicitly non-empty JSON `logRoot`
wins over `SC_LOG_ROOT`; the environment root is consulted only when JSON root
is absent/empty, then the application root wins if configured.




## Authoritative field inventory

The following is the complete D.1 schema. No other `SC_LOG_*` key is accepted.
Defaults are the values passed to or produced by `LoggerConfig::default_for`.

| Rust field | JSON key | `SC_` environment key | Unit / representation | Default | Validation |
| --- | --- | --- | --- | --- | --- |
| `level` | `level` | `SC_LOG_LEVEL` | existing `LevelFilter` serde token | `info` | delegates exact accepted spelling/case to `LevelFilter`; no free string retained |
| `log_root` | `logRoot` | `SC_LOG_ROOT` | non-empty OS path | `default_root` argument | present empty value is invalid |
| `enable_file_sink` | `enableFileSink` | `SC_LOG_FILE` | JSON boolean / env `true` or `false` | `true` | no numeric/truthy aliases |
| `enable_console_sink` | `enableConsoleSink` | `SC_LOG_CONSOLE` | JSON boolean / env `true` or `false` | `false` | no numeric/truthy aliases |
| `retained_log_policy.rotation_max_bytes` | `retainedLogPolicy.rotation_max_bytes` | `SC_LOG_ROTATION_MAX_BYTES` | canonical `ByteCount` serde / bytes | canonical policy default | existing strong-type validation |
| `retained_log_policy.rotation_max_files` | `retainedLogPolicy.rotation_max_files` | `SC_LOG_ROTATION_MAX_FILES` | canonical `FileCount` serde | canonical policy default | existing strong-type validation |
| `retained_log_policy.retention_max_age` | `retainedLogPolicy.retention_max_age` | `SC_LOG_RETENTION_MAX_AGE_MS` | canonical `RetentionMaxAge` serde / milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.maintenance_cadence` | `retainedLogPolicy.maintenance_cadence` | `SC_LOG_MAINTENANCE_CADENCE_MS` | canonical `MaintenanceCadence` serde / milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.writer_shutdown_timeout` | `retainedLogPolicy.writer_shutdown_timeout` | `SC_LOG_WRITER_SHUTDOWN_TIMEOUT_MS` | canonical `WriterShutdownTimeout` serde / milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.maintenance_max_work_per_pass` | `retainedLogPolicy.maintenance_max_work_per_pass` | `SC_LOG_MAINTENANCE_MAX_WORK_PER_PASS` | optional count | canonical policy default | existing policy validation |

When one or more retention-policy environment keys are set, D.1 constructs one
whole `RetainedLogPolicy`: each supplied key overrides its canonical default
and every omitted retention field uses that canonical default. The resulting
policy is then the atomic environment override; it is never field-wise merged
with a supplied JSON policy.

The application-prefix form replaces only the leading `SC_` in this table.
If the application prefix normalizes to `SC`, construction fails with
`PrefixCollision`. A selected namespace containing a non-UTF-8 key/value,
duplicate case-folded key, or unknown `${prefix}_LOG_*` key fails with a stable
typed code; unrelated namespaces and non-UTF-8 values are ignored.
Queue capacity, redaction, and process identity retain current
`LoggerConfig::default_for` values and are not D.1 configuration fields.

JSON absent and JSON `null` both mean “no override” for each optional field.
An absent environment variable also means no override; a present empty value
is invalid. Unknown JSON keys fail because of `deny_unknown_fields`. During an
environment scan, any key beginning with the exact selected `${prefix}LOG_`
namespace but not listed above is an unknown-key error; unrelated environment
keys are ignored. Duplicate/case-variant environment keys are rejected.




## Public contract

The existing exhaustive two-field `BridgeOptions` and
`init(LoggerConfig, BridgeOptions) -> Result<LogGuard, InitError>` signatures
remain source-compatible and retain their current derives and owned lifecycle.

```rust
pub trait BridgeEventPolicy: Send + Sync {
    fn decide(&self, event: &LogEvent) -> BridgeEventDecision;
}

#[non_exhaustive]
pub enum BridgeEventDecision {
    Admit,
    Reject(PolicyRejection),
}

#[non_exhaustive]
pub enum PolicyRejection {
    Denied,
    PayloadTooLarge,
    Invalid,
}

#[non_exhaustive]
pub struct AttachmentOptions {
    pub bridge: BridgeOptions,
    pub policy: Arc<dyn BridgeEventPolicy>,
}

pub fn attach_logger(
    logger: Arc<Logger>,
    options: AttachmentOptions,
) -> Result<LogAttachment, InitError>;

pub struct LogAttachment { /* no LevelOwner and no Logger shutdown authority */ }

impl LogAttachment {
    pub fn control(&self) -> LogControl;
    pub fn detach(self, timeout: Duration) -> Result<(), DetachError>;
}

impl AttachmentOptions {
    pub fn new(bridge: BridgeOptions, policy: Arc<dyn BridgeEventPolicy>) -> Self;
}
```

The policy inspects but cannot mutate the assembled event and returns admission
or a typed reasoned rejection. Existing `RedactionPolicy` remains the sole
redaction owner. Policy runs on the shared backend path immediately before
every `Logger::try_log`, including `LogControl::try_log`; macro, tracing, and
control entry points cannot bypass it. The trait is intentionally open for
host implementations; its one method plus non-exhaustive decision/reason
enums is the forward-compatibility decision.
The policy applies only to facade/attachment admission; a host's direct
`Logger::try_log` intentionally bypasses it. `BridgeEventPolicy` is owned by
`sc-observability-log`, which owns that facade boundary.

`LogAttachment` deliberately has no `elevate_level`, `reset_level`, or logger
shutdown method. `detach` first closes/removes the bridge slot so no new calls
can clone the logger, then waits boundedly for already-entered bridge calls to
release their references. `Drop` performs the same bounded detach but discards
the result; callers that need proof use explicit `detach`. Detach never invokes
`Logger::shutdown`. The host retains its original `Arc<Logger>` and can recover
the owned `Logger` with `Arc::try_unwrap` after successful detach.

The process-global slot has explicit `Empty`, `Owned`, `Attached`, and
`Closing` states. Owned `init` and host attachment are mutually exclusive.
The facade shim and `log::set_max_level(Trace)` are installed at most once;
the shim owns the process-global maximum while the backend filter owns actual
admission. A detached slot may be reattached through that shim, while a
foreign logger is always rejected. A saved `LogControl` after detach returns
the stable `NotInstalled` failure. Owned shutdown and attachment detach call
one `close_and_drain` primitive; detach never shuts down the host logger.




## Typed sink signatures

```rust
pub trait TypedLogSink: Send + Sync {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkFailure>;
    fn flush(&self) -> Result<(), LogSinkFailure>;
    fn health(&self) -> SinkHealth;
}
impl SinkRegistration {
    pub fn typed(sink: Arc<dyn TypedLogSink>) -> Self;
}
impl LoggerBuilder {
    pub fn register_typed_sink(&mut self, sink: Arc<dyn TypedLogSink>) -> Result<&mut Self, SinkRegistrationError>;
}
```

## Typed error bridge

`crates/sc-observability-types/src/typed.rs` owns `LogFailure` and `TryLogFailure` and their conversions to D12 `LogSinkError::{Write, Flush}` and `ShutdownError::{Timeout, Drain}`.

## Implementation targets

- `crates/sc-observability-types/src/typed.rs` and `crates/sc-observability/src/typed.rs`: remove duplicate Failure classification and document the retained D3 adapter mapping (deliverable 4).

## Acceptance criteria

- `cargo test -p sc-observability --test log_settings` passes source/resolution contract cases (deliverable 1).
- `cargo test -p sc-observability-log --test bridge_*` passes typed policy and detach cases (deliverable 2).
- `cargo test -p sc-observability --test typed_registration` passes the typed-sink signature path (deliverable 3).
- `rg "impl_legacy_classification!" crates/sc-observability-types/src/typed.rs` returns zero after the D4.6 migration (deliverable 4).
