# D1 startup logging settings

`LogSettings` is the serde-stable, startup-only logging configuration. Put it
under an application `logging` JSON key, capture one `EnvSnapshot`, parse `SC`
and an application prefix, then resolve once before constructing `LoggerConfig`.
There is no dynamic reload or post-construction mutation.

## Authoritative field table

`LogSettings` accepts exactly these fields. The environment column uses the
shared `SC` namespace; an application namespace replaces only that leading
`SC_` (for example, `APP_LOG_LEVEL`).

| Rust field | JSON key | Environment key | Representation | Default | Validation |
| --- | --- | --- | --- | --- | --- |
| `level` | `level` | `SC_LOG_LEVEL` | existing `LevelFilter` serde token | `info` | exact spelling and case accepted by `LevelFilter` |
| `log_root` | `logRoot` | `SC_LOG_ROOT` | non-empty OS path | supplied `default_root` | a present empty value is invalid |
| `enable_file_sink` | `enableFileSink` | `SC_LOG_FILE` | JSON boolean; environment `true` or `false` | `true` | no numeric or truthy aliases |
| `enable_console_sink` | `enableConsoleSink` | `SC_LOG_CONSOLE` | JSON boolean; environment `true` or `false` | `false` | no numeric or truthy aliases |
| `retained_log_policy.rotation_max_bytes` | `retainedLogPolicy.rotation_max_bytes` | `SC_LOG_ROTATION_MAX_BYTES` | canonical `ByteCount` serde, bytes | canonical policy default | existing strong-type validation |
| `retained_log_policy.rotation_max_files` | `retainedLogPolicy.rotation_max_files` | `SC_LOG_ROTATION_MAX_FILES` | canonical `FileCount` serde | canonical policy default | existing strong-type validation |
| `retained_log_policy.retention_max_age` | `retainedLogPolicy.retention_max_age` | `SC_LOG_RETENTION_MAX_AGE_MS` | canonical `RetentionMaxAge` serde, milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.maintenance_cadence` | `retainedLogPolicy.maintenance_cadence` | `SC_LOG_MAINTENANCE_CADENCE_MS` | canonical `MaintenanceCadence` serde, milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.writer_shutdown_timeout` | `retainedLogPolicy.writer_shutdown_timeout` | `SC_LOG_WRITER_SHUTDOWN_TIMEOUT_MS` | canonical `WriterShutdownTimeout` serde, milliseconds | canonical policy default | existing strong-type validation |
| `retained_log_policy.maintenance_max_work_per_pass` | `retainedLogPolicy.maintenance_max_work_per_pass` | `SC_LOG_MAINTENANCE_MAX_WORK_PER_PASS` | optional count | canonical policy default | existing policy validation |

Resolution is `defaults < JSON < SC_ environment < application environment`
for every field except `logRoot`: a non-empty JSON `logRoot` wins over both
environment namespaces; otherwise an application root wins over `SC_LOG_ROOT`.
Absent and JSON `null` values are unset. Present empty roots and environment
values are invalid. `retainedLogPolicy` is atomic: a JSON policy or any one
policy environment key replaces the whole policy; omitted policy environment
fields use canonical defaults and never merge into JSON policy fields.

## Environment prefix rules

`EnvPrefix` supplies validation and normalization. Callers provide a prefix
without a trailing underscore; the parser constructs `${prefix}_LOG_`. An
application prefix replaces only the leading `SC_` from the table. An
application prefix that normalizes to `SC` is rejected with `PrefixCollision`.

Selected namespace keys must use the exact case of `${prefix}_LOG_` and one of
the table keys. A case-folded duplicate, a case variant of the selected prefix,
or a non-UTF-8 selected key or value is `InvalidEnvironment`. An unlisted key
with the exact selected prefix is `UnknownKey`. Unrelated keys—and non-UTF-8
values attached to unrelated keys—are ignored. This makes one `EnvSnapshot`
deterministic while preventing selected configuration from being silently lost.

| Error | Stable code |
| --- | --- |
| `PrefixCollision` | `SC_LOG_SETTINGS_PREFIX_COLLISION` |
| `InvalidEnvironment` | `SC_LOG_SETTINGS_INVALID_ENVIRONMENT` |
| `UnknownKey` | `SC_LOG_SETTINGS_UNKNOWN_KEY` |
| `InvalidValue` | `SC_LOG_SETTINGS_INVALID_VALUE` |
| `Resolution` | `SC_LOG_SETTINGS_RESOLUTION` |
