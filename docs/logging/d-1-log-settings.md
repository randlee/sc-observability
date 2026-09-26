# D1 startup logging settings

`LogSettings` is the serde-stable, startup-only logging configuration. Put it
under an application `logging` JSON key, capture one `EnvSnapshot`, parse `SC`
and an application prefix, then resolve once before creating `LoggerConfig`.
There is no dynamic reload.

Resolution is `defaults < JSON < SC_ environment < application environment`
for every field except `logRoot`: a non-empty JSON `logRoot` wins over both
environment namespaces. Absent and JSON `null` values are unset; present empty
roots and environment values are invalid. The only accepted environment keys
are `LOG_LEVEL`, `LOG_ROOT`, `LOG_FILE`, `LOG_CONSOLE`, and the six retained
policy keys in the D13 field inventory. Environment booleans are exactly
`true` or `false`; selected namespace keys are case-sensitive.

| Error | Stable code |
| --- | --- |
| `PrefixCollision` | `SC_LOG_SETTINGS_PREFIX_COLLISION` |
| `InvalidEnvironment` | `SC_LOG_SETTINGS_INVALID_ENVIRONMENT` |
| `UnknownKey` | `SC_LOG_SETTINGS_UNKNOWN_KEY` |
| `InvalidValue` | `SC_LOG_SETTINGS_INVALID_VALUE` |
| `Resolution` | `SC_LOG_SETTINGS_RESOLUTION` |

`retainedLogPolicy` is atomic. A JSON policy or any supplied retained-policy
environment key replaces the complete policy; environment omissions use the
canonical policy defaults and never merge into JSON policy fields.
