# d-13: Logging contract

Generated projection of `obs-d-13`; the bead is authoritative.

## Plan metadata

- Wave: 1
- Layer: 3
- Assignee / model: cobs / terra
- Relation: `root`
- Closure: `contract`
- Target boundary: logging contract
- Branch: `sprint/d-13-logging-contract`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-13-logging-contract`
- PR target (merge order only): `sprint/d-21-otlp-contract`
- Blocked by: `obs-phase-d-plan-qa`
- Requirements: LAY-002, LAY-006, LAY-007, LOG-001, LOG-003, LOG-004, LOG-008, LOG-009, LOG-010, LOG-015, LOG-018, LOG-019, LOG-020, LOG-023, LOG-037, LOG-038, LOG-040, LOG-042, LOG-043, LOG-046, NFR-005, NFR-006, NFR-012, PHB-002, PHB-007, PHB-010, PHB-011, PHB-013, PHD-001, TYP-023, TYP-024, TYP-026, TYP-030, TYP-039
- ADRs: ADR-002, ADR-003, ADR-006, ADR-010, ADR-011, ADR-013, ADR-014, ADR-015, ADR-017, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-log/src/bridge.rs`
  - `crates/sc-observability-log/src/lib.rs`
  - `crates/sc-observability-log/tests/attachment_contracts.rs`
  - `crates/sc-observability-types/src/typed.rs`
  - `crates/sc-observability/src/builder.rs`
  - `crates/sc-observability/src/runtime.rs`
  - `crates/sc-observability/src/settings.rs`
  - `crates/sc-observability/src/typed.rs`
  - `crates/sc-observability/tests/log_contracts.rs`
  - `docs/plans/phase-d/sprint-d-13-logging-contract.md`

## Goal

Freeze the logging contracts against ADR-017/019 and PHB-002 without waiting for D.1/D.2/D.3 implementation tests.

## Deliverables

1. Define the LogSettings/ResolvedLogSettings/EnvSnapshot/LogSettingsInputs contract, validated log-root handling, serialization and stable LogSettingsError codes; add contract-only defaults and serde tests.

2. Define AttachmentOptions, open BridgeEventPolicy, decisions and non-owning LogAttachment; define DetachError and contract fixtures for timeout, stale NotInstalled and foreign logger rejection.

3. Freeze the final canonical 2.0 sink signatures and SinkRegistrationError in the design. TypedLogSink and LogSink stay open/object-safe. Compiled wave-1 fixtures use a private error-parameterized harness and existing baseline types only; obs-d-1/2/3 bind obs-d-12 canonical errors/codes in wave 2.

4. Specify canonical 2.0 typed.rs signatures/conversions and exact retiring symbols in this design, with no standing inventory file. Add compile/grep checks proving the private contract harness introduces no duplicate Failure classification or impl_legacy_classification!; obs-d-18 performs actual public-export activation and removal.

## This Sprint Does Not Close

D.1 resolves settings in the runtime, D.2 implements the process-global attachment, and D.3 implements builder registration. End-to-end logging and major-release approvals close in D.18.

## Design

## Settings contract

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
    pub log_root: LogRoot,
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
is absent/empty, an explicit JSON root also wins over the application root. Application root wins over SC_LOG_ROOT only when JSON root is absent. Empty explicit roots are invalid rather than silently overridden.




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
environment scan, any key beginning with the exact selected `${prefix}_LOG_`
namespace but not listed above is an unknown-key error; unrelated environment
keys are ignored. Duplicate/case-variant environment keys are rejected.




## Attachment contract

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
    pub fn detach(&mut self, timeout: Duration) -> Result<(), DetachError>;
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
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError>;
    fn flush(&self) -> Result<(), LogSinkError>;
    fn health(&self) -> SinkHealth;
}
impl SinkRegistration {
    pub fn typed(sink: Arc<dyn TypedLogSink>) -> Self;
}
impl LoggerBuilder {
    pub fn register_typed_sink(&mut self, sink: Arc<dyn TypedLogSink>) -> Result<&mut Self, SinkRegistrationError>;
}
```

## Ownership, errors and capability decisions

Settings and sink behavior contracts belong to sc-observability; shared neutral error context remains sc-observability-types; attachment/DetachError belongs to sc-observability-log under PHB-002/ADR-011. Dependencies point bridge -> core/types, never types -> bridge. DetachError::{Timeout, NotInstalled, ForeignLoggerInstalled} is non-exhaustive and carries boxed ErrorContext. Its stable codes are SC_LOG_DETACH_TIMEOUT, SC_LOG_DETACH_NOT_INSTALLED and SC_LOG_FOREIGN_LOGGER_INSTALLED; D.12 installs these registry constants, obs-d-13 specifies the companion enum and tests its shape in the private baseline-only harness; obs-d-2 binds the registry-backed production form in wave 2. SinkRegistrationError::{Duplicate, Invalid, Closed} is specified for the final core typed.rs contract with boxed ErrorContext and corresponding SC_LOG_SINK_REGISTRATION_* codes in the D.12-owned registry. LogSettingsError::{PrefixCollision, InvalidEnvironment, UnknownKey, InvalidValue, Resolution} uses the already specified LOG-001..005 diagnostics; those are diagnostic codes, not requirement IDs.

TypedLogSink and LogSink remain open because downstream custom sinks are required; their final write/flush errors are canonical LogSinkError; the snippets specify the final bound contract, not wave-1 imports. TypedLogSink is a documented alias/forwarding surface to the canonical open sink contract, not a new duplicate classifier. The old adapter remains only as transitional compatibility until D.18. Both registration entry points preserve sink metadata and are implemented in D.3 builder.rs. obs-d-13 typed.rs supplies the error-parameterized private contract/test double without changing D.3-owned builder.rs.

ResolvedLogSettings holds a validated LogRoot wrapper with private PathBuf and AsRef<Path>; source LogSettings retains optional PathBuf for serde compatibility and validates at resolution. LOG-009 is explicit-config precedence: a nonempty JSON logRoot outranks both environment namespaces; other fields use defaults < JSON < SC_ < application. Empty roots reject.

Detach takes &mut self: Timeout retains the attachment for retry/inspection after admission closes. Successful detach transitions it to detached and releases its logger references. Drop remains a bounded best-effort operation; proof requires explicit successful detach. LogControl remains a read/admission capability with no level/shutdown methods. Its stale-slot NotInstalled runtime check is retained because cloned handles outlive detach; a lifetime phantom cannot encode process-global concurrent revocation. Compile-fail contract examples prove that attachment/control cannot invoke owner-only operations.

The full schemas above are authoritative here; implementation beads reference them rather than restating signatures. Contract fixture tests in log_contracts.rs/attachment_contracts.rs exercise the contract state/test doubles, never an unfinished production bridge or settings resolver.

## Handoff to obs-d-2 (wave 2)

Created/staged by obs-d-13, owned by obs-d-2 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-log/src/bridge.rs`

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-13, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-log/src/lib.rs`
- `crates/sc-observability-types/src/typed.rs`
- `crates/sc-observability/src/settings.rs`
- `crates/sc-observability/src/typed.rs`

## Handoff to obs-d-3 (wave 2)

Created/staged by obs-d-13, owned by obs-d-3 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability/src/builder.rs`

## Handoff to obs-d-1 (wave 2)

Created/staged by obs-d-13, owned by obs-d-1 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability/src/runtime.rs`


## Independent wave-1 compilation (lead ruling PLAN-SCOPE-016)

Every concrete code block above is the final contract specification. obs-d-13's compiled fixtures use a private error-parameterized harness and existing baseline types only; no new obs-d-12 canonical error name or registry row is imported or defined in this wave. This applies to settings/attachment/registration errors and their code assertions, not just LogSinkError. The harness accepts the error/code payload as data and tests structural shape, policy/ownership and resolution rules. It does not export a second canonical enum or duplicate registry constants. Production registry binding is checked by the wave-2 consumer. Final concrete signatures remain frozen; fixture type parameters do not become extra public generic parameters. obs-d-18 activates the typed.rs canonical public exports after both contract and implementation gates. The exact D.18 retirement targets in `sc-observability-types/src/typed.rs` are the legacy classifier implementations generated by `impl_legacy_classification!` for `IdentityFailure`/`IdentityFailureKind`, `InitFailure`/`InitFailureKind`, `EventFailure`/`EventFailureKind`, `FlushFailure`/`FlushFailureKind`, `ShutdownFailure`/`ShutdownFailureKind`, `ProjectionFailure`/`ProjectionFailureKind`, `SubscriberFailure`/`SubscriberFailureKind`, `LogSinkFailure`/`LogSinkFailureKind`, and `ExportFailure`/`ExportFailureKind`, respectively paired with `IdentityError`, `InitError`, `EventError`, `FlushError`, `ShutdownError`, `ProjectionError`, `SubscriberError`, `LogSinkError`, and `ExportError`. D.18 removes those exact macro expansions and their generated `ClassifiedError` classifier implementations after the canonical public enums are active; it does not remove the public enum specification or this private independent harness.

ADR-019 records atomic retained-policy resolution, explicit root precedence, open policy/sink traits, &mut detach retry, stale control runtime checks, and the independent compilation split. ADR-006 constrains the generic settings namespaces: no ATM-specific loader or adapter behavior is added. LOG-042/046 and PHB-013 constrain the host attachment contract: one host-owned writer/maintenance worker and definitive owner shutdown; attachment/control never gain that authority.


## Handoff to obs-d-1

obs-d-13 freezes concrete settings/attachment/TypedLogSink signatures and supplies baseline-only private fixtures. obs-d-1 consumes that specification plus obs-d-12 canonical errors/registry rows and binds them in its owned runtime.rs in wave 2. obs-d-3 consumes TypedLogSink and owns builder.rs implementation; it does not own typed.rs. typed.rs final exports activate in obs-d-18.


## Handoff to obs-d-2

obs-d-13 freezes concrete settings/attachment/TypedLogSink signatures and supplies baseline-only private fixtures. obs-d-2 consumes that specification plus obs-d-12 canonical errors/registry rows and binds them in its owned bridge.rs in wave 2. obs-d-3 consumes TypedLogSink and owns builder.rs implementation; it does not own typed.rs. typed.rs final exports activate in obs-d-18.


## Handoff to obs-d-3

obs-d-13 freezes concrete settings/attachment/TypedLogSink signatures and supplies baseline-only private fixtures. obs-d-3 consumes that specification plus obs-d-12 canonical errors/registry rows and binds them in its owned builder.rs in wave 2. obs-d-3 consumes TypedLogSink and owns builder.rs implementation; it does not own typed.rs. typed.rs final exports activate in obs-d-18.


## Release gate and scope

This boundary releases only its named artifact to obs-d-18 after its paired sanity check; final 2.0 semver/API approval, obsolete-wrapper removal and release inventory are obs-d-18 gates. The phase-root workspace invariant applies once to every sprint.


## Handoff to obs-d-17

Handoff to obs-d-17: the canonical sink contract is consumed by the log-consumer check and named examples; sink write/flush and shutdown outcomes remain typed and owner-scoped.

## Acceptance criteria

- [ ] `cargo test -p sc-observability --test log_contracts --locked` runs nonzero settings_serde_defaults, log_root_validation, private_sink_contract_object_safety and registration_error_payloads contract cases (#1/#3).
- [ ] `cargo test -p sc-observability-log --test attachment_contracts --locked` runs detach_retry_after_timeout, stale_control_not_installed and foreign_logger_rejected against the owned contract fixture; compile-fail examples deny owner authority (#2).
- [ ] The private contract harness preserves supplied code/remediation/source without new classification. #4: cargo test -p sc-observability --test log_contracts --locked runs contract_harness_preserves_context; git diff of typed.rs plus rg -n "impl_legacy_classification!|impl .*Failure" over newly added harness blocks yields no new obsolete classifiers. Existing compatibility code is not a failure here; obs-d-18 checks actual deletion after activation. No standing inventory file is created.
- [ ] Contract checks use only owned test targets, without wildcard cargo test names or D.1/D.2/D.3 implementation fixtures. PHB-003/005 historical 1.x rules are not asserted as the new 2.0 compatibility gate.

- [ ] #1–4: cargo check --workspace --all-features --locked and the named fixture tests pass from the unchanged develop baseline plus obs-d-13 alone, with no obs-d-12 canonical-name or registry-row imports. Follow the root workspace invariant.
