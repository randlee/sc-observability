# Phase D recovery

This plan completes recovery from integration commit `14ea9dc438389591a6d7c39f755e8da3460c4017` through the `phase-d-finish` stack. Frozen stack **#889 remains untouched**. Its CI evidence informs fixes; its workflow expansion is not imported wholesale.

The order is **remove duplicate execution → fix known issues locally → enable Windows CI and finish the residual sweep**. The sprint dependency graph is in [sprints.jsonl](sprints.jsonl). Each scope has a development bead and its corresponding `-sanity` bead; source dependencies determine composition order, not a requirement to wait for remote CI before beginning scoped work.

## Execution rules

- aobs maintains scope and beads. Team-lead dispatches development and coordinates focused sanity/QA. Quality-mgr approves scope and verifies fixes.
- Use a separate branch for each fix scope. Keep today's CI platform coverage unchanged throughout stages 1 and 2. Run relevant local tests and Windows-target compilation/clippy through xwin on macOS; xwin does not execute Windows tests.
- Before every commit and push, run all locally executable test/check commands from pull-request workflows, plus xwin on macOS, and retain each command and exit code. This includes documentation-only changes. Fix scoped failures; report unrelated failures with base-versus-head evidence. Never push red silently. Remote CI does not gate starting the next scoped task; Windows runtime execution remains deferred to stage 3.
- Remove only genuinely dead code, including suppressed dead code. Preserve live code under supported features, platforms, tests, and macros. Remove unjustified suppressions without deleting live behavior or adding dummy uses.
- Fix a missed issue in its owning layer and rebase descendants. Do not build an append-only chain of fixes for red CI.

GUI test suites must not launch on the operator desktop. Record unrun GUI qualification honestly; do not count it as passing or add a replacement CI job.

## Stage 1: remove duplicate tests and checks

| Bead | Scope and retained coverage |
| --- | --- |
| `obs-phase-d-ci-dedupe` | Consolidate repeated API/semver, API-doc regeneration, binding-schema, rustdoc, and other demonstrated duplicate test execution. Retain one effective execution of each unique check. Missed duplicates amend the owning [PR #941](https://github.com/randlee/sc-observability/pull/941), then descendants rebase. |

The duplication inventory applies to the recovery baseline. Expanded job counts from the frozen stack are not evidence that those jobs run on this baseline. Removing duplicate execution must not remove platform-specific behavior coverage or release-package tests.

## Stage 2: fix all known issues without enabling Windows CI

| Bead | Bounded scope |
| --- | --- |
| `obs-phase-d-dead-code` | Sync-http feature wiring and genuinely unused code reported by clippy, including file-identity helpers. |
| `obs-phase-d-dead-code-2` | Dead code and stale suppressions in examples and test fixtures; retain meaningful tests. |
| `obs-phase-d-dead-code-3` | Remaining dead OTLP items, test-only gates, and stale macro/OTLP suppressions. |
| `obs-phase-d-xwin-1` | Windows-target query.rs clippy diagnostics and the Tauri test feature gate. |
| `obs-recovery-config-live-gates` | Remaining configuration-specific suppressions: preserve live validation, contracts, factory and lifecycle behavior with accurate gates. Reconcile earlier cleanup batches before editing. |
| `obs-recovery-unused-emitter-traits` | Verify and remove unused internal emitter indirection; retain public behavior and reconcile affected requirements/design prose. |
| `obs-recovery-api-parity` | ADR and compiled public-API equality comparison across release platforms with matching feature sets and toolchain. Missing target evidence must fail, not appear as equality. |
| `obs-recovery-otlp-retry-tests` | Reproduce and fix the two known retry-ownership test failures without weakening retry or completion assertions. |
| `obs-recovery-schema-portability` | Resolve Windows generated-schema drift while retaining the schema contract. |
| `obs-recovery-importer-timezone` | Supply portable timezone data for the sanity-history importer. |
| `obs-recovery-publishkit-triage` | Map the historical publish-kit failures to root causes and bounded fixes; distinguish launch failures from product assertion failures. |
| `obs-recovery-wheel-toolchain` | Resolve the deterministic macOS wheel-runner Rust toolchain installation conflict reported on #942 by amending the owning CI layer #941; keep #942 dead-code-only. |
| `obs-recovery-viewer-portability` | Held by team-lead pending separation of retained fixture-unit failures from deferred full viewer qualification. No viewer behavior/test expansion is authorized in recovery. |
| `obs-recovery-publishkit-shell` | One portable shell fixture selecting Git Bash rather than WSL on Windows; migrate probe/preflight/publish-kit tests with platform-correct paths. |
| `obs-recovery-publishkit-callers` | Migrate remaining publish-kit callers to that helper, including the stage-target separator assertion, preserving fail-closed release checks. |
| `obs-recovery-follow-test-portability` | Restore the Windows-ignored truncate/recreate file-follow test through a portable fixture with the same no-duplicate-event assertions. |
| `obs-recovery-cli-collector-response` | Reproduce and resolve the two known CLI collector-response failures at their owning boundary. |

Local tests and xwin cover the applicable configurations in each bead. Python/shell portability work retains meaningful platform-specific fixture assertions locally; actual Windows behavior is qualified in stage 3. A historical failure that no longer reproduces requires evidence at the recovery head, not an unnecessary replacement fix.

### Failure provenance and existing findings

The bottom frozen layer, [PR #886](https://github.com/randlee/sc-observability/pull/886), did **not** run Windows tests. The known-failure inventory instead uses [PR #890](https://github.com/randlee/sc-observability/pull/890) at `f784149d`: eight failed Windows jobs across six areas. Repeated jobs are not eight independent defects. The six areas are core Windows clippy (also blocking Python runtime), schema drift (also affecting TypeScript schema), OTLP retry tests, viewer harness (blocking API governance), importer timezone data, and publish-kit subprocess/path failures. Scoped beads above own the applicable repairs. Native/xwin inventories and the later wheel and collector-response reproductions extend this historical evidence.

Existing findings are reused rather than copied or closed merely because recovery work was assigned:

| Existing finding | Remediation bead |
| --- | --- |
| `obs-pm-win-viewer-download-exe-name` | `obs-recovery-viewer-portability` |
| `obs-pm-win-viewer-harness-log-handle` | `obs-recovery-viewer-portability` |
| `obs-pm-win-stage-target-separator` | `obs-recovery-publishkit-callers` |

Close a finding only after the applicable fix is verified, or record why it does not apply to the recovery head. Frozen branches remain untouched.

## Stage 3: enable Windows and finish the sweep

`obs-recovery-windows-ci-sweep` follows all stage-1/2 scopes. Enable Windows execution in the retained existing CI workflows for Rust workspace/OTLP feature tests, standalone Tauri, Python runtime, installed packages, and already-approved importer/publish-kit and fixture-unit platform behavior. Exclude the full installed-wheel/CLI/real-viewer telemetry qualification from automatic CI and phase-D completion. Keep platform-independent duplicate checks single; do not add a new job for each finding.

Run one coordinated Windows sweep that collects independent failures rather than letting one early failure hide downstream suites. Repair each residual in its owning layer, rebase descendants, and rerun affected checks. Completion requires all required Windows checks to execute and pass at the same final head, with existing Linux/macOS coverage and local xwin still passing. Enabled YAML, queued jobs, skipped required checks, and a plan approval are not completion evidence.

## Post-phase-D integration

The retained `tests/telemetry-e2e` installed-wheel/CLI/viewer suite belongs under the planned `just integrate` **after phase D**, not on every sprint PR. Remove its automatic CI trigger in the existing dedupe layer; preserve its tests and pinned fixtures. `obs-post-phase-d-just-integrate`, owned by team-lead under the backlog epic, depends on phase D finishing. It owns the recipe integration, execution, and retirement of obsolete workflow glue; phase D does not depend on it. No new CI job is authorized.

## Scope approval record

The first six `obs-recovery-*` scopes—API parity, OTLP retry tests, schema portability, importer timezone, publish-kit triage, and wheel toolchain—have scope approval from `obs-phase-d-plan-qa-8`. The remaining eight recovery scopes were approved by quality-mgr after verification of five carried scope corrections (ATM receipt `01M40214N9N6NPZES9SHSK18QT`). This records plan approval only; it makes no claim that any implementation or the recovery as a whole is complete. Implementation receipts remain with the owning beads and their verification records.
