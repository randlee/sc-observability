---
id: D.2
status: proposed
branch: feature/phase-d-2-log-settings
base: develop
---

# D.2 — Shared startup `LogSettings` (#96)

## Goal and dependency

Create the single serde-stable, binding-friendly configuration value in
`sc-observability` that apps resolve before constructing a `Logger`. It has no
internal sprint dependency and must merge before D.1.

## Deliverables

1. Add public `LogSettings` and per-field retention overrides with optional,
   camelCase serde fields for level, root, file/console sink enablement, and
   retention/rotation settings. Define defaults as non-verbose `Info`, logging
   enabled from process start, and `LoggerConfig::default_for` compatibility
   when no override is supplied.
2. Implement typed env parsing for the documented `SC_LOG_*` variables and
   optional application prefix. Resolve `env > JSON file > defaults`; unknown,
   malformed, overflowed, or invalid values return a stable typed error rather
   than silently falling back.
3. Provide `into_logger_config(service_name, default_root)` that maps every
   resolved value to existing `LoggerConfig`/`RetainedLogPolicy` without
   changing runtime-level ownership or allowing post-construction config
   mutation.
4. Document JSON keys, environment keys, precedence, defaults, failure codes,
   and startup-only lifecycle. Add a public-only example for an app embedding
   settings below its own `logging` JSON key.

## Acceptance criteria

- Empty JSON round-trips and resolves exactly like today's
  `LoggerConfig::default_for`.
- Each supported environment key overrides file and default; every field has a
  precedence fixture, and invalid/unknown input yields the documented typed
  failure and code.
- Serde serialization is stable/camelCase and does not expose arbitrary free
  string level values at the public configuration boundary.
- Configuration is fully resolved before logger construction; no setter or
  late reload is introduced.

## Required validation

- Focused unit tests for JSON, precedence, defaults, all env keys, invalid
  values, and conversion parity.
- Public consumer compile fixture plus `cargo test --workspace --locked`.
- Docs consistency and rustdoc gate used by the repository at execution time.

## Non-closure

Do not migrate consumer applications in this sprint, implement dynamic reload,
or plan #88 bindings/OTEL work.
