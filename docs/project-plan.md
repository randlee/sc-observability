# SC-Observability Project Plan

## Status

This repo is in initial extraction/setup.

The immediate goal is to establish:
- correct standalone ownership for observability types, facades, and OTLP export
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
     `sc-observability`, then `sc-observability-otlp`
   - workspace version stays above the source ATM workspace version that last
     published these crate names
4. Complete crates.io ownership and release readiness:
   - verify crate ownership/maintainers for `sc-observability-types`,
     `sc-observability`, and `sc-observability-otlp`
   - verify publish tokens and first-release permissions
   - document the handoff from ATM-published crates to this repo
5. Separate neutral observability code from ATM-specific adapters.
6. Move only generic types/config/export logic into this repo.
7. Verify ATM cutover readiness:
   - published crate names match the existing names used in ATM
   - replacement instructions are documented
   - no `agent-team-mail-*` dependencies remain
8. Write the migration plan after the agents are live and operating on the new
   repos.

## Rule

Any sprint plan added here must preserve the standalone boundary defined by:
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/git-workflows.md`
- `docs/publishing.md`
