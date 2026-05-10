# Migrate From `log`

Use this reference when the existing project is based on `log` macros plus a
separate logger backend.

## Inventory

- which backend is active now
- whether file layout is part of operator tooling
- whether message formatting is structured or plain text
- where warnings and errors originate today

## Migration Strategy

- replace backend-specific setup with `LoggerConfig::default_for(...)`
- keep the log root decision explicit
- introduce stable `target` and `action` names instead of free-form categories
- migrate the highest-value paths first:
  - startup
  - shutdown
  - warnings
  - errors
  - CLI success

## Watch Items

- existing macros may hide category naming drift
- downstream tooling may assume older file locations
- plain-text-only logs usually need a deliberate structured field model
