# Migrate From Custom JSONL

Use this reference when the project already writes its own structured JSONL
logs or has a custom retained sink pipeline.

## Inventory

- current path layout
- current schema fields
- rotation and retention behavior
- custom health or dropped-event accounting

## Migration Strategy

- compare the current schema with `LogEvent`
- preserve required operational fields first
- decide whether custom sink behavior should remain custom or can be replaced
  by the built-in file sink
- confirm path compatibility before changing active log locations

## Watch Items

- custom schemas often encode app-specific semantics in ad hoc fields
- downstream tooling may depend on exact filenames or field names
- custom health behavior may need explicit remapping to `logger.health()`
