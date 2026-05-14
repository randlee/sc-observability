# Remediation Patterns

## Missing Default Log Root

Recommend:

- introduce one configuration helper that resolves the app's default root
- if following house style, use `dirs::home_dir()` and `~/.<app>`

## Missing Console Path

Recommend:

- enable `config.enable_console_sink = true` for simple stdout console logging
- use `LoggerBuilder` plus `ConsoleSink::stderr()` when stderr is preferred

## Missing Lifecycle Events

Recommend:

- add service startup and shutdown events first
- add a single CLI success event per successful command path

## No Runtime Verification

Recommend:

- expose `logger.health()` in diagnostics or startup logs
- inspect `active_log_path` during initialization

## Naming Drift

Recommend:

- normalize `target` to subsystem-style namespaces
- normalize `action` to stable verbs or lifecycle names
