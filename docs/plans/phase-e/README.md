# Phase E: independently selectable integration testing

This new stack adds an on-demand integration workflow and `just integrate`. It does not modify Phase D recovery or specify sc-lint. Sprint content lives in Beads; `sprints.jsonl` is the authored dependency graph.

## Execution and ownership

`e-1` owns the shared workflow, small dispatch command and declared suite/viewer interfaces. Results use GitHub job conclusions and existing test reports; no new result schema or collector is required. After its sanity PASS, `e-2` through `e-7` can all start from `sprint/e-1-workflow`. They have no sibling dependencies and own separate suite directories. Each builds its own artifacts, including wheels where needed. They never require another suite's job to produce artifacts.

The six suite branches stack in completion order; the coordinator records the actual predecessor and layer before final publication. Parent branch movement affects finalization, not starting scoped work. Each sprint has separate dev, sanity and QA beads under the phase root; QA blocks on sanity. Keeping QA as a sibling avoids blocking dev closure on its own review. Sanity reviews every numbered deliverable; PASS dispatches QA immediately and failure returns to its dev owner. QA pins the actual sanity PASS commit before dispatch; no commit is invented during planning.

Phase D is merged to develop. Integration suite development consumes e-1 contracts from its approved head; e-8 through e-12 start directly from integrate/phase-e. Do not import the discarded frozen stack.

| Start wave | Sprints | Ownership / contract |
| --- | --- | --- |
| Immediately | e-1 and e-8–e-12 | Workflow/dispatch and five independent contract-history fences, respectively |
| After e-1 sanity | e-2–e-7 | Six separate suite directories consuming the runner contract below |

Twelve sprints; critical path two sprint stages; maximum logical width eleven (six suite sprints plus five independent contract sprints). These are dependency limits, not a promise of eleven simultaneous workers. e-3 may develop while e-2 builds the shared viewer support; final e-3 native viewer evidence needs that implementation. This is not a sibling start gate.

## Dispatch contract

```text
just integrate <branch>
just integrate <branch> --suite wheel-cli-viewer --os macos
just integrate <branch> --suite wheels,collector --os windows,linux
```

The workflow has only `workflow_dispatch`, with equivalent suite and OS inputs. The default selects all six suites on macOS, Windows and Linux: 18 independent suite/OS jobs. Validate selection before dispatch. All jobs use the branch SHA pinned by that run. Job matrix fail-fast is disabled; assertion failure does not stop other runnable tests within the job. Setup or compilation failure is reported explicitly and cannot be represented as successful coverage.

The command reports the run URL; native GitHub tooling exposes every selected job result. Missing selected results fail the aggregate. Partial runs are clearly labelled; only the complete selection can provide phase-ending integration coverage. The repository default branch is `main`. Before native dispatch, the minimal `integration.yml` registration must be published there through the planned e-1 bootstrap PR (normal review/merge, no direct push); subsequent runs select the implementation branch with `--ref`. Do not silently change the default branch, bypass branch policy or add PR/push triggers. e-1 can prove local selector/runner contracts before bootstrap, but cannot claim native suite execution. Bootstrap publication is required for native validation, not for starting parallel implementation.

## Small runner interface

`python3 scripts/integrate/run_suite.py --suite <selector> --source-sha <40hex> --output-dir <absolute-path>`
invokes `python3 scripts/integrate/suites/<selector>/run.py --source-sha <40hex> --output-dir <absolute-path>` from
the checked-out repository root. The suite verifies HEAD equals the supplied
SHA (the run's pinned `GITHUB_SHA`), builds its own artifacts and writes existing logs/reports beneath
the supplied directory. Exit zero means all selected assertions passed;
nonzero means failure. Missing runners fail explicitly. No result schema or
sibling artifact exchange is introduced. e-1 owns dispatch; each suite owns its
entrypoint and setup. CI uploads its output directory even on failure.

Both viewer suites reuse
`scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py` and its `release.json`.
The existing CLI contract is `start --binary PATH --binary-sha256 SHA --version
VERSION --state-dir DIR`, `status --state-dir DIR`, `assert-production
--state-dir DIR --backend sdk|sync-http`, and `stop --state-dir DIR`.
Each job has its own state directory/ports. e-2 extends platform acquisition
behind that interface; e-3 invokes the existing public-factory fixtures/readback
and must not duplicate acquisition or claim synthetic probes as factory proof.

## Suite boundaries

| Sprint | Selector | Existing coverage |
| --- | --- | --- |
| e-2 | wheel-cli-viewer | Installed Python wheel and actual CLI delivery/readback, equivalence, offline recovery, restart and partial delivery against pinned viewer |
| e-3 | rust-viewer | Two ignored public SDK and sync-http factory viewer tests; excludes the ordinary hermetic cases |
| e-4 | collector | Hermetic full-stack OTLP backend conformance and canonical ingress; excludes real-viewer cases; canonical ingress runs once per OS |
| e-5 | wheels | Candidate wheel installation, owned runtime, host-attached runtime and embedding from packaged artifacts |
| e-6 | tauri | Packaged npm/Rust artifacts through actual frontend/host IPC on CI runners |
| e-7 | rust-consumers | Candidate packaged core, binding bridge, runtime-level and log-bridge consumers outside workspace |

Existing wrappers often combine integration with unit, lint, schema, release or publication checks. Reuse the narrow integration entrypoints, not entire legacy workflows. Wheel builds in e-2/e-5 are intentional independent setup for different boundaries, not duplicated tests. Prefer small fixes to existing helpers over duplicate adapters. e-2 owns the shared viewer implementation; e-3 develops against the declared interface. Viewer availability affects only viewer suites, not completion of e-1 or the four unrelated suites.

## Concrete platform work

The existing viewer manifest contains only a macOS arm64 v0.5.0 artifact. e-2 extends the existing d-32 viewer_harness.py and release.json rather than introducing another fixture. It preserves the required v0.5.0 pin and supplies a macOS/Windows/Linux artifact URL, SHA-256 checksum and executable table for both viewer suites. Missing upstream assets require an explicit blocker/decision, never a mock or silent skip. e-2 fixes POSIX-only virtualenv and executable paths. e-7 adapts the bundled binding consumer's Darwin/Linux-only sandbox by minimally fixing the existing helper using existing cross-platform isolation support. No developer GUI launches are permitted; actual viewer/webview execution is confined to CI.

No new product feature, publication, API governance, exhaustive interpreter/architecture matrix, generic unit/lint duplication, or sc-lint design is included. Native Windows execution is evidence; xwin alone is not.

Reused runtime-level behavior tests run only in this on-demand integration workflow. ADR-013 publisher-run release qualification remains separate; these jobs do not become per-PR gates or replace that qualification.

## Retire replaced integration entrypoints

Retirement is part of each replacement sprint, not a later cleanup sprint. Remove the old job after its replacement demonstrates the same integration assertions on the supported OSes. Preserve tests, logs and distinct publisher qualification. Update callers/documentation in the same change; do not retain duplicate manual entrypoints.

| Owner | Retirement |
| --- | --- |
| e-2 | Delete `telemetry-e2e.yml` once `wheel-cli-viewer` replaces it. |
| e-4 | Sole editor of `otlp-conformance.yml`: remove each old job only after its corresponding e-3/e-4 replacement has native proof; delete the file when both are replaced. e-3 supplies its evidence without editing this file. |
| e-7 | Remove the replaced `packaged-consumer` job in `binding-runtime.yml`; preserve unrelated native-contract coverage and required release aggregation. |

e-4 owns the shared retirement edit at final publication; no other sprint edits that workflow and no sibling development dependency is added. e-5 and e-6 call existing narrow assertion helpers read-only where possible; any required helper/caller edits are explicitly fenced in their beads. They preserve the distinct B.4a and TypeScript publisher qualification interfaces. Do not import their whole release pipelines, API extraction, unit tests or lint into integration jobs. `telemetry-platforms.yml` is a six-target build check, not duplicate integration execution, and is not retired by this plan. The importer suite is not silently added to these six suites.

## Contract-versioning work

The user added e-8 through e-12: public API, binding/IPC, CLI, configuration and durable-store history. See [schema-versioning.md](schema-versioning.md). They run independently of the seven integration sprints and add no integration matrix cells or automatic workflows. The earlier API-governance exclusion applies to integration runners, not this newly authorized work.
