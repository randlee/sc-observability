---
id: D.1
status: planned
branch: feature/phase-d-1-log-settings
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-1-log-settings
depends_on: []
relation: parallel_safe
assignee: cobs
model_class: terra
owned_docs: [docs/requirements.md, docs/api-design.md]
---

# D.1 — Shared startup `LogSettings` (#96)

## Goal and dependency

Create the single serde-stable, binding-friendly configuration value in
`sc-observability` that applications resolve before constructing a `Logger`.
It is parallel-safe with D.2 and D.3 because neither consumes this type. This
is additive 1.x work checked against the published 1.4.1 API/semver baseline.

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

impl LogSettings {
    pub fn from_env(prefix: EnvPrefix) -> Result<Self, LogSettingsError>;
    pub fn resolve(inputs: LogSettingsInputs) -> Result<ResolvedLogSettings, LogSettingsError>;
}

pub struct LogSettingsInputs {
    pub file: Option<LogSettings>,
    pub shared_env: LogSettings,
    pub application_env: Option<LogSettings>,
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
`LogEnvPrefix`, or an overrides type. `RetainedLogPolicy` gains `serde(default)`
only if needed to support a partial nested object, preserving its field names,
strong validation, and canonical units.

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
| `level` | `level` | `SC_LOG_LEVEL` | enum: `off`, `error`, `warn`, `info`, `debug`, `trace` | `info` | exact case-insensitive enum token; no free string retained |
| `log_root` | `logRoot` | `SC_LOG_ROOT` | non-empty OS path | `default_root` argument | present empty value is invalid |
| `enable_file_sink` | `enableFileSink` | `SC_LOG_FILE` | JSON boolean / env `true` or `false` | `true` | no numeric/truthy aliases |
| `enable_console_sink` | `enableConsoleSink` | `SC_LOG_CONSOLE` | JSON boolean / env `true` or `false` | `false` | no numeric/truthy aliases |
| `retained_log_policy.rotation_max_bytes` | `retainedLogPolicy.rotation_max_bytes` | `SC_LOG_ROTATION_MAX_BYTES` | canonical `ByteCount` serde / bytes | canonical policy default | existing strong-type validation |
| `retained_log_policy.rotation_max_files` | `retainedLogPolicy.rotation_max_files` | `SC_LOG_ROTATION_MAX_FILES` | canonical `FileCount` serde | canonical policy default | existing strong-type validation |
| `retained_log_policy.retention_max_age` | `retainedLogPolicy.retention_max_age` | `SC_LOG_RETENTION_MAX_AGE_MS` | canonical `RetentionMaxAge` serde / milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.maintenance_cadence` | `retainedLogPolicy.maintenance_cadence` | `SC_LOG_MAINTENANCE_CADENCE_MS` | canonical `MaintenanceCadence` serde / milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.writer_shutdown_timeout` | `retainedLogPolicy.writer_shutdown_timeout` | `SC_LOG_WRITER_SHUTDOWN_TIMEOUT_MS` | canonical `WriterShutdownTimeout` serde / milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.maintenance_max_work_per_pass` | `retainedLogPolicy.maintenance_max_work_per_pass` | `SC_LOG_MAINTENANCE_MAX_WORK_PER_PASS` | optional count | canonical policy default | existing policy validation |

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

## Deliverables

1. Add the public source/resolved types, typed errors, stable codes, rustdoc,
   serde behavior, and signatures above by reusing `EnvPrefix`,
   `LevelFilter`, and `RetainedLogPolicy`; add no parallel owners.
2. Implement deterministic environment parsing for the complete inventory and
   field-wise resolution in the documented order including the LOG-009 root
   exception. Parsing uses a
   captured environment snapshot so one resolution cannot mix process states.
3. Convert the resolved value to `LoggerConfig` and its strong policy types,
   preserving all non-inventory defaults and introducing no post-construction
   mutation.
4. Document the table, precedence, null/unset behavior, prefix rules, failure
   codes, and startup-only lifecycle. Add a public example embedding settings
   under an application's `logging` JSON key.

## Acceptance criteria

- Empty/`null` JSON resolves exactly like `LoggerConfig::default_for` for the
  supplied service and default root.
- Every row has fixtures for default, JSON, shared environment, application
  environment, and full precedence; nested retention fields merge per field
  rather than replacing the whole object.
- Every invalid boundary, present-empty environment value, unknown JSON key,
  and unknown selected-prefix environment key returns its documented typed
  failure and code.
- Collision, trailing-underscore misuse, case collision, selected-namespace
  non-UTF-8, and LOG-009 root-precedence fixtures freeze prefix behavior.
- Serde uses exactly the camelCase keys and delegates level tokens to the
  existing `LevelFilter` conversion/fixtures; level spelling is defined by
  that single owner.
- Configuration is fully resolved before logger construction; no setter,
  watcher, or late reload is introduced.

## Required validation

- Table-driven unit tests generated from the authoritative inventory for JSON,
  environment, precedence, defaults, validation, and conversion parity.
- Paired regressions prove `LogSettings` delegates to the existing
  `LevelFilter` wire and `RetainedLogPolicy` validation rather than defining
  second codecs or units.
- Public consumer compile fixture plus `cargo test --workspace --locked`.
- Docs consistency, rustdoc, public API, and semver gates used by the repository
  at execution time.

## Non-closure

Do not migrate consumer applications, add fields outside the inventory,
implement dynamic reload, or plan #88 bindings/OTEL work.
