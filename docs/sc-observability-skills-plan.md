# SC-Observability Skills and Marketplace Plan

## 1. Purpose

This document defines the implementation plan for a repo-local and marketplace-
published `sc-observability` skill package.

The package will provide three Claude Code skills:

1. `sc-observability-new-project`
2. `sc-observability-existing-project`
3. `sc-observability-review`

These skills are intended to help downstream Rust applications adopt the
published `sc-observability` crates from crates.io with a consistent house
style for structured logging, configuration, and review.

## 2. Inputs And Constraints

This plan is constrained by the following source material and decisions:

- this repo's current local skill layout under `.claude/skills/`
- the Synaptic Canvas skill and agent guidance:
  `/Users/randlee/Documents/github/synaptic-canvas/docs/claude-code-skills-agents-guidelines.md`
- the Synaptic Canvas marketplace-forwarding guidance:
  `/Users/randlee/Documents/github/synaptic-canvas/docs/marketplace-forwarding.md`
- the current consumer-facing repo docs:
  - `README.md`
  - `CONSUMING.md`
  - `docs/architecture.md`
  - `docs/requirements.md`
  - `docs/migration-guide.md`
  - `docs/atm-quickstart.md`

Accepted product decisions for this plan:

- marketplace name: `sc-observability`
- package name: `sc-observability`
- the skills must exist in both:
  - this repo's local `.claude/skills/` tree
  - the marketplace package under `packages/sc-observability/skills/`
- migration content is useful, but does not need to be a first-class skill in
  this phase
- a review skill must exist in the same package

## 3. Marketplace Rule

Per the Synaptic Canvas marketplace-forwarding guidance, this repo cannot
technically "forward" installs to the Synaptic Canvas marketplace.

The supported model is:

1. publish a minimal independent marketplace in this repo
2. document that users should add both marketplaces when they want both sets
   of packages:
   - `randlee/synaptic-canvas`
   - `randlee/sc-observability`

This plan therefore treats marketplace forwarding as a documentation and
consumer-install pattern, not a technical dependency or inheritance mechanism.

## 4. Deliverables

### 4.1 Local Skill Tree

The repo-local skill copy will live under `.claude/skills/`:

- `.claude/skills/sc-observability-new-project/SKILL.md`
- `.claude/skills/sc-observability-new-project/references/standard-configuration.md`
- `.claude/skills/sc-observability-new-project/references/console-and-log-root.md`
- `.claude/skills/sc-observability-new-project/assets/observability.rs.j2`
- `.claude/skills/sc-observability-existing-project/SKILL.md`
- `.claude/skills/sc-observability-existing-project/references/standard-configuration.md`
- `.claude/skills/sc-observability-existing-project/references/console-and-log-root.md`
- `.claude/skills/sc-observability-existing-project/references/adoption-checklist.md`
- `.claude/skills/sc-observability-existing-project/references/migrate-from-log.md`
- `.claude/skills/sc-observability-existing-project/references/migrate-from-tracing.md`
- `.claude/skills/sc-observability-existing-project/references/migrate-from-custom-jsonl.md`
- `.claude/skills/sc-observability-review/SKILL.md`
- `.claude/skills/sc-observability-review/references/review-checklist.md`
- `.claude/skills/sc-observability-review/references/remediation-patterns.md`
- `.claude/skills/sc-observability-review/references/app-type-guidance.md`

### 4.2 Marketplace Package

The marketplace package copy will live under `packages/sc-observability/`:

- `packages/sc-observability/.claude-plugin/plugin.json`
- `packages/sc-observability/README.md`
- `packages/sc-observability/CHANGELOG.md`
- `packages/sc-observability/skills/sc-observability-new-project/SKILL.md`
- `packages/sc-observability/skills/sc-observability-new-project/references/standard-configuration.md`
- `packages/sc-observability/skills/sc-observability-new-project/references/console-and-log-root.md`
- `packages/sc-observability/skills/sc-observability-new-project/assets/observability.rs.j2`
- `packages/sc-observability/skills/sc-observability-existing-project/SKILL.md`
- `packages/sc-observability/skills/sc-observability-existing-project/references/standard-configuration.md`
- `packages/sc-observability/skills/sc-observability-existing-project/references/console-and-log-root.md`
- `packages/sc-observability/skills/sc-observability-existing-project/references/adoption-checklist.md`
- `packages/sc-observability/skills/sc-observability-existing-project/references/migrate-from-log.md`
- `packages/sc-observability/skills/sc-observability-existing-project/references/migrate-from-tracing.md`
- `packages/sc-observability/skills/sc-observability-existing-project/references/migrate-from-custom-jsonl.md`
- `packages/sc-observability/skills/sc-observability-review/SKILL.md`
- `packages/sc-observability/skills/sc-observability-review/references/review-checklist.md`
- `packages/sc-observability/skills/sc-observability-review/references/remediation-patterns.md`
- `packages/sc-observability/skills/sc-observability-review/references/app-type-guidance.md`

### 4.3 Marketplace Metadata

The minimal marketplace metadata set will include:

- `.claude-plugin/marketplace.json`
- `docs/registries/nuget/registry.json`
- a package-level README and changelog suitable for registry metadata linkage

The marketplace metadata must advertise one package, `sc-observability`, that
contains three skills.

## 5. Shared Standard Configuration

The skills package must define one shared standard configuration model so apps
use `sc-observability` consistently.

### 5.1 Default Log Root Convention

The standard house style is:

- application config chooses a logical app name
- default log root is `~/.<app>`
- the built-in file sink therefore writes to:
  `~/.<app>/logs/<service>.log.jsonl`

This is intentionally layered on top of the crate contract documented in
`CONSUMING.md`, which defines the final sink layout as:

- `<log_root>/logs/<service>.log.jsonl`

The skill content must make it explicit that the repo-wide convention is to set
`log_root` to `~/.<app>` by default, not to change the sink naming contract.

### 5.2 Default Logging Behavior

The standard baseline for downstream apps is:

- use `sc-observability` from crates.io as the initial default
- start with logging-only adoption before routing or OTLP unless the user asks
  for a heavier stack
- default to a light logging posture:
  - warnings and errors are always emitted
  - informational logging is intentionally sparse by default
  - informational events should focus on lifecycle and meaningful successful
    operations rather than verbose step-by-step tracing
- built-in file sink enabled by default
- built-in console sink disabled by default
- long-running applications emit startup and shutdown events
- CLIs generally emit one successful completion event per successful command
- `logger.health()` and `active_log_path` are the standard runtime verification
  hooks

### 5.3 Override And Console Rules

The shared configuration references must define:

- the standard method for overriding the default log root in code
- how `SC_LOG_ROOT` interacts with explicit configuration
- how to enable console logging quickly
- when to use `ConsoleSink::stdout()` versus `ConsoleSink::stderr()`
- when file-only, console-only, or file-plus-console operation is appropriate

### 5.4 Naming Guidance

The shared configuration references must define a basic event-naming house
style:

- `service` uses the app/service identity
- `target` uses stable subsystem-style namespaces
- `action` uses stable action names, not ad hoc prose
- startup, shutdown, success, warning, and error events should be named
  predictably enough to support consistent search and review

## 6. Skill Design Rules

All three skills must follow the Synaptic Canvas skill guidance:

- keep `SKILL.md` concise and procedural
- move detailed material to `references/`
- put starter code in `assets/`
- avoid unnecessary auxiliary docs
- keep one-level reference discovery from `SKILL.md`
- use skill metadata that clearly signals when the skill should trigger

This phase does not require dedicated execution agents. The skills are guidance
and composition assets first.

If agent-backed execution is added later, it must be designed separately and
validated against the same Synaptic Canvas architecture guidance.

## 7. Skill Specifications

### 7.1 `sc-observability-new-project`

Purpose:
- help a user set up a new Rust project or repo with `sc-observability` from
  crates.io

Primary workflow:
1. choose the right crate layer:
   - logging only: `sc-observability`
   - typed routing: `sc-observe`
   - OTLP: `sc-observability-otlp`
2. recommend logging-only setup as the default starting path
3. apply the standard configuration convention:
   - default log root `~/.<app>`
   - active file path `~/.<app>/logs/<service>.log.jsonl`
4. provide a minimal setup recipe for dependencies and logger construction
5. provide template-backed starter code through `observability.rs.j2`
6. explain how to override the log root and enable console logging
7. point to deeper repo docs only when needed

Required references:
- `references/standard-configuration.md`
- `references/console-and-log-root.md`

Required asset:
- `assets/observability.rs.j2`

Required template contents:
- app-name-based default log root helper
- standard `LoggerConfig::default_for(...)` setup
- easy switch for enabling console logging
- example startup/shutdown events for long-running apps
- example success event for CLIs
- `logger.health()` usage including active path inspection

Guideline evaluation step for this skill:
1. review the finished skill against:
   `/Users/randlee/Documents/github/synaptic-canvas/docs/claude-code-skills-agents-guidelines.md`
2. confirm:
   - metadata clearly describes trigger conditions
   - `SKILL.md` stays concise
   - detailed material lives in `references/`
   - template code lives in `assets/`
   - no unnecessary agent layer is introduced
3. record any required reductions in scope or wording before the skill is
   declared done

### 7.2 `sc-observability-existing-project`

Purpose:
- help a user bring `sc-observability` into an existing Rust project without
  overloading the base skill with full migration playbooks

Primary workflow:
1. inspect the current project shape
2. decide whether logging-only adoption is enough
3. apply the same standard configuration convention used by the new-project
   skill
4. provide an adoption checklist:
   - dependency additions
   - logger construction
   - log root choice
   - console behavior
   - event naming
   - runtime verification
5. use migration references only when the current project already has a notable
   logging surface

Required references:
- `references/standard-configuration.md`
- `references/console-and-log-root.md`
- `references/adoption-checklist.md`
- `references/migrate-from-log.md`
- `references/migrate-from-tracing.md`
- `references/migrate-from-custom-jsonl.md`

Scope rule:
- the skill itself remains focused on adoption into an existing repo
- library-specific migration details live in references, not in the core
  `SKILL.md`

Guideline evaluation step for this skill:
1. review the finished skill against:
   `/Users/randlee/Documents/github/synaptic-canvas/docs/claude-code-skills-agents-guidelines.md`
2. confirm:
   - `SKILL.md` remains the high-level workflow and navigation layer
   - migration details were pushed into references
   - the skill does not collapse setup, migration, and review into one large
     body
   - reference files are one hop away from `SKILL.md`
3. trim or split content further if the skill body grows beyond a lean
   discovery-layer document

### 7.3 `sc-observability-review`

Purpose:
- review a Rust project's observability setup against the `sc-observability`
  house style and recommend missed or inconsistent items

Primary workflow:
1. determine app type:
   - CLI
   - long-running service
   - hybrid tool
2. inspect current logging and observability setup
3. compare it to the standard configuration contract
4. produce prioritized recommendations
5. explicitly call out missing baseline items

Review scope must cover:
- use of `sc-observability` from crates.io where applicable
- default log root convention
- final log path expectation
- log-root override method
- console setup method
- file sink defaults
- warning and error coverage
- startup/shutdown behavior for long-running apps
- single success event pattern for CLIs
- event naming consistency
- runtime verification via `logger.health()` or equivalent checks

Required references:
- `references/review-checklist.md`
- `references/remediation-patterns.md`
- `references/app-type-guidance.md`

Guideline evaluation step for this skill:
1. review the finished skill against:
   `/Users/randlee/Documents/github/synaptic-canvas/docs/claude-code-skills-agents-guidelines.md`
2. confirm:
   - the skill is a review/discovery layer, not an overbuilt execution engine
   - checklist details and remediation patterns live in references
   - the `SKILL.md` body stays compact and decision-oriented
   - the output guidance is specific enough to be actionable
3. revise wording if the skill becomes a generic review skill instead of a
   focused `sc-observability` review surface

## 8. Reference Content Plan

### 8.1 `standard-configuration.md`

This shared reference must define:

- the house-style default `~/.<app>` log root
- resulting file layout
- default sinks
- light logging expectations and boundaries
- warning/error expectations
- startup/shutdown expectations for long-running apps
- success-event expectations for CLIs
- target/action naming rules
- `logger.health()` verification pattern

This file should be duplicated exactly between the relevant local and package
copies unless a later sync mechanism is introduced.

### 8.2 `console-and-log-root.md`

This reference must define:

- explicit in-code log-root override
- `SC_LOG_ROOT` behavior and precedence
- easy console enablement
- `stdout()` versus `stderr()` guidance
- common local-development versus production usage modes

### 8.3 `adoption-checklist.md`

This reference must define:

- current-state audit items
- dependency updates
- config insertion points
- expected runtime validation checks
- rollout advice for incremental adoption

### 8.4 Migration References

These references are support material for existing-project use cases:

- `migrate-from-log.md`
- `migrate-from-tracing.md`
- `migrate-from-custom-jsonl.md`

They should explain:

- what to inventory first
- what can remain temporarily during adoption
- what to replace with `sc-observability`
- what ingestion/path/health assumptions need review

### 8.5 Review References

These references must support the review skill:

- `review-checklist.md`
- `remediation-patterns.md`
- `app-type-guidance.md`

They should let the review skill distinguish between:

- a missing baseline
- a deliberate divergence
- an app-type-specific choice

## 9. Marketplace Metadata Plan

### 9.1 `.claude-plugin/marketplace.json`

This file must:

- register the repo as marketplace `sc-observability`
- advertise one package named `sc-observability`
- point the package source to `./packages/sc-observability`
- include the required package metadata fields described by the Synaptic Canvas
  marketplace guidance

### 9.2 `packages/sc-observability/.claude-plugin/plugin.json`

This file must:

- define package name `sc-observability`
- list the three skill artifacts
- omit commands and agents unless they are intentionally added later

### 9.3 Package README And Changelog

The package-level `README.md` and `CHANGELOG.md` must exist so marketplace and
registry metadata can point to real package documentation.

The package README should cover:

- what the `sc-observability` package contains
- the three included skills
- the independent-marketplace install model
- the recommendation to add Synaptic Canvas separately when needed

### 9.4 `docs/registries/nuget/registry.json`

This file must:

- advertise the `sc-observability` package
- report the correct skill count
- use the repo as the distribution source
- declare tier based on actual install assumptions

Expected tier for this phase:
- tier `0` if the package is self-contained guidance and templates only
- tier `1` only if required token substitution is introduced
- do not claim a lower tier than the package really needs

### 9.5 Forwarding Guidance

The marketplace docs should explain:

- users add this repo's marketplace independently
- users add Synaptic Canvas separately when they want both sources
- this repo does not proxy Synaptic Canvas packages

## 10. Duplication And Sync Rule

Because the skills must exist in both the repo-local `.claude/skills/` tree and
the marketplace package, this phase must define a practical sync rule.

Required rule for this phase:

- local and marketplace skill copies start identical at creation time
- every content change to a local skill must be mirrored into the package copy
  in the same change set
- the plan review must check both trees for drift before completion

If drift becomes costly later, a follow-on task may introduce generation or
sync tooling, but this plan does not require such tooling up front.

## 11. Implementation Sequence

1. Create this planning document.
2. Create the minimal marketplace skeleton:
   - `.claude-plugin/marketplace.json`
   - `packages/sc-observability/.claude-plugin/plugin.json`
   - `packages/sc-observability/README.md`
   - `packages/sc-observability/CHANGELOG.md`
   - `docs/registries/nuget/registry.json`
3. Create the local skill directories.
4. Create the marketplace package skill directories.
5. Write shared reference content:
   - standard configuration
   - console/log-root guidance
6. Write `observability.rs.j2`.
7. Write `sc-observability-new-project/SKILL.md`.
8. Evaluate the new-project skill against the Synaptic Canvas skill guidelines
   and revise it.
9. Write existing-project references and `SKILL.md`.
10. Evaluate the existing-project skill against the Synaptic Canvas skill
    guidelines and revise it.
11. Write review references and `SKILL.md`.
12. Evaluate the review skill against the Synaptic Canvas skill guidelines and
    revise it.
13. Review both trees for local/package parity.
14. Validate marketplace metadata for correct package and artifact counts.
15. Review the full package for completeness and trim any bloated skill bodies.

## 12. Plan Review Step

After the plan is written, perform a plan-review pass before implementation
starts.

The review must verify:

- all three skills are accounted for
- the standard configuration contract is explicit
- log-root override and console guidance are explicit
- the template is part of the plan
- migration material is references-only in this phase
- marketplace duplication is accounted for
- metadata files are named explicitly
- package README/changelog coverage is accounted for
- guideline-evaluation steps exist for each skill
- forwarding limitations are stated accurately

If any detail is missing, update this plan before it is committed.

## 13. Exit Criteria

This plan is complete when:

- the document captures the full artifact inventory
- each skill has a defined purpose, workflow, and reference set
- the standard configuration is defined as a first-class shared contract
- per-skill Synaptic Canvas guideline-evaluation steps are included
- marketplace and duplication rules are explicit
- the plan-review pass is complete with any missing details corrected

## 14. Non-Goals For This Phase

This plan does not require:

- a fourth migration skill
- agent-backed execution helpers
- automated sync tooling between local and package copies
- a technical marketplace-forwarding mechanism
- changes to the core `sc-observability` crate behavior itself
