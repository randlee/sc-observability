---
id: D.2
status: proposed
branch: feature/phase-d-2-log-settings
base: develop
---

# D.2 — Shared startup `LogSettings` (#96)

## Goal and dependency

Create the single serde-stable, binding-friendly configuration value in
`sc-observability` that applications resolve before constructing a `Logger`.
It has no internal sprint dependency and must merge before D.1.

## Public contract

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct LogSettings {
    pub level: Option<LevelFilter>,
    pub log_root: Option<PathBuf>,
    pub enable_file_sink: Option<bool>,
    pub enable_console_sink: Option<bool>,
    pub retained_log_policy: Option<RetainedLogPolicyOverrides>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RetainedLogPolicyOverrides {
    pub rotation_max_bytes: Option<u64>,
    pub rotation_max_files: Option<u64>,
    pub retention_max_age_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEnvPrefix(String);

impl LogEnvPrefix {
    pub fn shared() -> Self; // exactly "SC_"
    pub fn application(value: &str) -> Result<Self, LogSettingsError>;
}

pub struct ResolvedLogSettings {
    pub level: LevelFilter,
    pub log_root: PathBuf,
    pub enable_file_sink: bool,
    pub enable_console_sink: bool,
    pub rotation_max_bytes: ByteCount,
    pub rotation_max_files: FileCount,
    pub retention_max_age: RetentionMaxAge,
}

impl LogSettings {
    pub fn from_env(prefix: LogEnvPrefix) -> Result<Self, LogSettingsError>;
    pub fn resolve(
        file: Option<Self>,
        shared_env: Self,
        application_env: Option<Self>,
        default_root: PathBuf,
    ) -> Result<ResolvedLogSettings, LogSettingsError>;
}

impl ResolvedLogSettings {
    pub fn into_logger_config(self, service_name: ServiceName) -> LoggerConfig;
}
```

`LogEnvPrefix::shared()` is exactly `SC_`. An application prefix must match
`[A-Z][A-Z0-9_]*_`; `LogEnvPrefix::application("BTIT_")`, for example, maps
the same suffixes to `BTIT_LOG_*`. Resolution is field-wise
`defaults < JSON < SC_ environment < application environment`. If no
application prefix is requested, the last layer is absent.

## Authoritative field inventory

The following is the complete D.2 schema. No other `SC_LOG_*` key is accepted.
Defaults are the values passed to or produced by `LoggerConfig::default_for`.

| Rust field | JSON key | `SC_` environment key | Unit / representation | Default | Validation |
| --- | --- | --- | --- | --- | --- |
| `level` | `level` | `SC_LOG_LEVEL` | enum: `off`, `error`, `warn`, `info`, `debug`, `trace` | `info` | exact case-insensitive enum token; no free string retained |
| `log_root` | `logRoot` | `SC_LOG_ROOT` | non-empty OS path | `default_root` argument | present empty value is invalid |
| `enable_file_sink` | `enableFileSink` | `SC_LOG_FILE` | JSON boolean / env `true` or `false` | `true` | no numeric/truthy aliases |
| `enable_console_sink` | `enableConsoleSink` | `SC_LOG_CONSOLE` | JSON boolean / env `true` or `false` | `false` | no numeric/truthy aliases |
| `retained_log_policy.rotation_max_bytes` | `retainedLogPolicy.rotationMaxBytes` | `SC_LOG_ROTATION_MAX_BYTES` | bytes, base-10 unsigned integer | `67_108_864` | `1..=u64::MAX` |
| `retained_log_policy.rotation_max_files` | `retainedLogPolicy.rotationMaxFiles` | `SC_LOG_ROTATION_MAX_FILES` | file count, base-10 unsigned integer | `10` | must fit target `usize`; zero means retain no rotated files |
| `retained_log_policy.retention_max_age_days` | `retainedLogPolicy.retentionMaxAgeDays` | `SC_LOG_RETENTION_MAX_AGE_DAYS` | calendar days, base-10 unsigned integer | `7` | `1..=u32::MAX` and duration conversion must not overflow |

The application-prefix form replaces only the leading `SC_` in this table.
`maintenance_cadence`, `writer_shutdown_timeout`, queue capacity, redaction,
process identity, and maintenance work limit retain current
`LoggerConfig::default_for` values and are not D.2 configuration fields.

JSON absent and JSON `null` both mean “no override” for each optional field.
An absent environment variable also means no override; a present empty value
is invalid. Unknown JSON keys fail because of `deny_unknown_fields`. During an
environment scan, any key beginning with the exact selected `${prefix}LOG_`
namespace but not listed above is an unknown-key error; unrelated environment
keys are ignored. Duplicate/case-variant environment keys are rejected.

## Deliverables

1. Add the public source and resolved types, prefix type, typed errors, stable
   codes, rustdoc, serde behavior, and signatures above.
2. Implement deterministic environment parsing for the complete inventory and
   field-wise resolution in the documented four-layer order. Parsing uses a
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
- Serde uses exactly the camelCase keys and typed level values in the table;
  round-trip fixtures freeze the representation.
- Configuration is fully resolved before logger construction; no setter,
  watcher, or late reload is introduced.

## Required validation

- Table-driven unit tests generated from the authoritative inventory for JSON,
  environment, precedence, defaults, validation, and conversion parity.
- Public consumer compile fixture plus `cargo test --workspace --locked`.
- Docs consistency, rustdoc, public API, and semver gates used by the repository
  at execution time.

## Non-closure

Do not migrate consumer applications, add fields outside the inventory,
implement dynamic reload, or plan #88 bindings/OTEL work.
