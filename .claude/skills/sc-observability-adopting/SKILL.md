---
name: sc-observability-adopting
description: Use when the user says things like "bring sc-observability into an existing Rust project" or "adopt sc-observability in a codebase that already has logging". Adopts sc-observability incrementally with a logging-first path and focused migration references for log, tracing, or custom JSONL setups.
---

# SC-Observability Adopting

Use this skill when an existing Rust codebase wants to adopt
`sc-observability`.

## Workflow

1. Inspect the current logging shape first.
2. Default to a logging-only adoption path before adding routing or OTLP.
3. Apply the same house style from
   `references/standard-configuration.md`.
4. Use `references/adoption-checklist.md` for the step-by-step insertion
   plan.
5. Only load migration references when the project already depends on:
   - `log`
   - `tracing`
   - custom JSONL or custom retained logging

## Scope Boundary

This skill is for adopting `sc-observability` into an existing repo.

Library-specific migration detail lives in:

- `references/migrate-from-log.md`
- `references/migrate-from-tracing.md`
- `references/migrate-from-custom-jsonl.md`

These files are new writes for this package. Do not treat this skill as a
wrapper around the repo's ATM-scoped migration docs.

## Required Guidance

- `~/.<app>` remains a house style, not a crate default.
- Add `sc-observability-types` directly only for custom sinks or shared-type
  extension work.
- Preserve stable `target` and `action` naming while replacing existing
  logging entry points.

## References

- `references/standard-configuration.md`
- `references/console-and-log-root.md`
- `references/adoption-checklist.md`
- migration references as needed

## Guideline Evaluation

Reviewed against
`/Users/randlee/Documents/github/synaptic-canvas/docs/claude-code-skills-agents-guidelines.md`.

- `SKILL.md` stays concise: confirmed
- detailed material lives in `references/`: confirmed
- starter code lives in `assets/`: confirmed
- scope reductions made: migration-specific playbooks stay in `references/`
  instead of expanding the core skill body
