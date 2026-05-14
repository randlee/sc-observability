# Review Checklist

## Baseline

- `sc-observability` is used when the project wants the shared logging surface
- `sc-observability-types` is only a direct dependency when actually needed
- default file sink behavior is understood

## Pathing

- the project defines a default log root convention
- if following house style, it resolves to `~/.<app>`
- the resulting active path matches `<log_root>/logs/<service>.log.jsonl`
- the app can verify the resolved path through `logger.health()`

## Behavior

- warnings and errors are always emitted
- informational logging is not noisy by default
- services log startup and shutdown
- CLIs log one success event per successful command

## Configuration

- the log root can be overridden cleanly
- `SC_LOG_ROOT` behavior is documented or understood
- console output is easy to enable intentionally

## Naming

- `service` is stable
- `target` is subsystem-oriented
- `action` is stable and searchable
