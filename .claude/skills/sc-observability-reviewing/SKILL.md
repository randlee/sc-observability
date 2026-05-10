---
name: sc-observability-reviewing
description: Use when the user says things like "review this project for sc-observability best practices" or "check whether our logging setup matches the sc-observability standard". Reviews a Rust project's observability setup against the shared house style and recommends missing baseline items and easy improvements.
---

# SC-Observability Reviewing

Use this skill when a user wants a focused review of a Rust project's logging
or observability setup against the `sc-observability` house style.

## Workflow

1. Determine the app type:
   - CLI
   - long-running service
   - hybrid tool
2. Load `references/review-checklist.md`.
3. Use `references/app-type-guidance.md` to adjust expectations by app type.
4. Use `references/remediation-patterns.md` to turn gaps into concrete
   recommendations.
5. Prioritize recommendations:
   - missing baseline items first
   - then inconsistent conventions
   - then optional improvements

## Review Focus

- crates.io adoption surface
- log root and file path convention
- override path and environment precedence
- console setup path
- warnings/errors coverage
- service startup/shutdown coverage
- CLI success-event coverage
- stable naming for `target` and `action`
- runtime verification through `logger.health()`

## Output Rule

Keep findings concrete. Distinguish:

- missing baseline
- deliberate divergence
- app-type-specific choice
