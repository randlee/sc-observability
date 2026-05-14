---
name: sc-observability-bootstrapping
description: Use when the user says things like "set up sc-observability in a new Rust project" or "add structured logging to a new Rust app". Bootstraps a new project with sc-observability from crates.io using the shared light-logging house style, ~/.<app> log roots, and a rendered starter template.
---

# SC-Observability Bootstrapping

Use this skill for new Rust projects or repos that want to start with
`sc-observability`.

## Workflow

1. Choose the lightest crate set that fits the request:
   - logging only: `sc-observability`
   - typed routing: `sc-observe`
   - OTLP export: `sc-observability-otlp`
2. Default to logging-only setup unless the user explicitly needs more on day
   one.
3. Apply the house style from
   `references/standard-configuration.md`.
4. Use `assets/observability.rs.j2` when the user wants starter code or
   `sc-compose` generation.
5. Use `references/console-and-log-root.md` for custom roots, console output,
   and `SC_LOG_ROOT` precedence.

## Required Guidance

- `~/.<app>` is a skill-defined house style, not a crate default.
- The crate file-layout rule remains `<log_root>/logs/<service>.log.jsonl`.
- Add `sc-observability-types` directly only when implementing custom sinks or
  extending the shared types layer.
- Keep informational logging sparse by default:
  - warnings and errors always emitted
  - long-running apps log startup and shutdown
  - CLIs log one success event per successful command

## References

- `references/standard-configuration.md`
- `references/console-and-log-root.md`
- `README.md`
- `CONSUMING.md`

## Template Asset

- `assets/observability.rs.j2`

The template is a tier-1 rendered starter artifact intended for `sc-compose`
style token substitution using:

- `{{APP_NAME}}`
- `{{SERVICE_NAME}}`
- `{{ENABLE_STDOUT_CONSOLE}}`
- `{{ENABLE_STDERR_CONSOLE}}`

## Guideline Evaluation

Reviewed against
`/Users/randlee/Documents/github/synaptic-canvas/docs/claude-code-skills-agents-guidelines.md`.

- `SKILL.md` stays concise: confirmed
- detailed material lives in `references/`: confirmed
- starter code lives in `assets/`: confirmed
- scope reductions made: none
