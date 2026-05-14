# Adoption Checklist

## Audit First

- identify the current logging library or custom logger
- identify current log file paths and operator expectations
- identify whether console output is part of normal operation
- identify whether success events already exist for CLI commands
- identify whether startup and shutdown events already exist for services

## Logging-Only First

Prefer this sequence unless the user explicitly needs more on day one:

1. add `sc-observability`
2. wire `LoggerConfig::default_for(...)`
3. adopt the `~/.<app>` house-style root or an explicit override
4. replace or wrap the current logging entry points
5. verify the active file path through `logger.health()`

## Incremental Rollout

- replace the highest-value lifecycle and error paths first
- keep stable `target` and `action` naming from the beginning
- defer routing and OTLP until logging-only behavior is stable
- verify downstream log readers or shipping jobs if file paths change
