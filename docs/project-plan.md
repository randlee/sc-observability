# SC-Observability Project Plan

## Status

This repo is in initial extraction/setup.

The immediate goal is to establish:
- correct standalone ownership for observability types, facades, and OTLP export
- a real `sc-observe` workspace crate between logging and OTLP layers
- zero `agent-team-mail-*` dependencies
- a clean publishable workspace structure

## Near-Term Work

1. Set up repository git flow:
   - use `main` and `develop`
   - feature branches target `develop`
   - release tags and release publication come from `main`
   - keep repo workflow and review discipline aligned with ATM
2. Match GitHub automation and protection to ATM:
   - CI triggers match the ATM repo pattern for `pull_request` and `push`
   - branch protection and rulesets match ATM for `main` and `develop`
   - GitHub secrets and environments are configured and use the same variable
     names as ATM where the workflows overlap
3. Verify repository setup end to end:
   - release preflight validates publish order and version alignment
   - release workflow is ready to publish `sc-observability-types`,
     `sc-observability`, `sc-observe`, then `sc-observability-otlp`
   - workspace version stays above the source ATM workspace version that last
     published these crate names
4. Complete crates.io ownership and release readiness:
   - verify crate ownership/maintainers for `sc-observability-types`,
     `sc-observability`, `sc-observe`, and `sc-observability-otlp`
   - verify publish tokens and first-release permissions
   - document the handoff from ATM-published crates to this repo
5. Separate neutral observability code from ATM-specific adapters and prove the
   external adapter pattern with non-production examples.
6. Move only generic types/config/export logic into this repo.
7. Verify ATM cutover readiness:
   - published crate names match the existing names used in ATM
   - replacement instructions are documented
   - no `agent-team-mail-*` dependencies remain
8. Maintain extraction inventory and boundary ADRs as modules move from ATM to
   the standalone repo.
9. Write the migration plan after the agents are live and operating on the new
   repos.

## Rule

Any sprint plan added here must preserve the standalone boundary defined by:
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/git-workflows.md`
- `docs/publishing.md`

## Implementation Planning Set

The next implementation phase should use these planning documents together:

- `docs/implementation-plan.md`
- `docs/public-api-checklist.md`
- `docs/atm-adapter-mapping-spec.md`
- `docs/test-strategy.md`
- `docs/sprint-plan.md`

## Query/Follow API Implementation Phase

The next docs-approved implementation phase is the logging query/follow API for
GitHub issues `#24` through `#29`.

Scope constraints:

- docs and code must preserve the shared boundary defined in
  `docs/requirements.md` and `docs/architecture.md`
- no ATM-specific types or `agent-team-mail-*` dependencies are introduced
- no async runtime becomes required for query/follow behavior
- the implementation order follows `#24 -> #25 -> (#26, #27, #28) -> #29`

### Wave 1: Shared Query Contract

Issues:
- `#24` LogQuery model

Crates:
- `sc-observability-types`

Deliverables:
- `LogQuery`, `LogOrder`, and `LogFieldMatch`
- filter fields for service, levels, target, action, request/correlation ids,
  time bounds, field matches, limit, and order
- serde-ready shared contracts and validation rules where required

Acceptance criteria:
- req-qa can trace the shipped contract to `requirements.md` TYP-032 and
  TYP-033
- arch-qa can confirm the contract lives in `sc-observability-types`
- `cargo check` passes with no new crate-edge violations

### Wave 2: Shared Query Error Surface

Issues:
- `#25` QueryError

Crates:
- `sc-observability-types`

Deliverables:
- `QueryError` enum with `InvalidQuery`, `Io`, `Decode`, `Unavailable`, and
  `Shutdown`
- stable error-code constants for each variant path
- `DiagnosticInfo` integration where the implementation needs diagnostic
  projection

Acceptance criteria:
- req-qa can trace the shipped error surface to TYP-035 and TYP-036
- arch-qa can confirm the error vocabulary remains shared and ATM-free
- `cargo check` passes with no public API drift against wave 1

### Wave 3: Logger Query/Follow Runtime

Issues:
- `#26` historical query API
- `#27` follow/tail API
- `#28` query health signal

Crates:
- `sc-observability-types`
- `sc-observability`

Deliverables:
- `LogSnapshot`
- `Logger::query(&self, &LogQuery) -> Result<LogSnapshot, QueryError>`
- `Logger::follow(&self, LogQuery) -> Result<LogFollowSession, QueryError>`
- `LogFollowSession::poll()` and `LogFollowSession::health()`
- `QueryHealthReport` and `LoggingHealthReport.query`
- rotation-aware behavior for active and rotated JSONL files

Acceptance criteria:
- req-qa can trace the shipped runtime to TYP-034, TYP-037, and LOG-025
  through LOG-031
- arch-qa can verify crate placement, exact signatures, and the synchronous
  poll model match `architecture.md`
- tests prove no duplicate or silently skipped committed records across file
  rotation/truncation scenarios
- `cargo check` passes with no async-runtime dependency added

### Wave 4: Independent JSONL Reader

Issues:
- `#29` JsonlLogReader

Crates:
- `sc-observability`

Deliverables:
- public `JsonlLogReader`
- offline historical query/follow support using the same query contracts and
  error vocabulary as `Logger`
- reader behavior documented as independent from live `Logger`,
  `sc-observe`, and `sc-observability-otlp`

Acceptance criteria:
- req-qa can trace the shipped reader to LOG-028 through LOG-032
- arch-qa can verify the reader stays in `sc-observability` and does not pull
  in routing or OTLP concerns
- `cargo check` passes and boundary CI remains green

### Exit Condition For The Phase

The query/follow implementation phase is complete only when:

- all four waves are merged in dependency order
- public API docs and tests stay aligned with `requirements.md` and
  `architecture.md`
- req-qa and arch-qa have an explicit acceptance target for each wave
- downstream implementation can begin immediately without revisiting crate
  ownership or public signature questions
