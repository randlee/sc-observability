# Phase E: independently selectable integration testing

This new stack adds an on-demand integration workflow and `just integrate`. It does not modify Phase D recovery or specify sc-lint. Sprint content lives in Beads; `sprints.jsonl` is the authored dependency graph.

## Execution and ownership

`e-1` owns the shared workflow, small dispatch command and declared suite/viewer interfaces. Results use GitHub job conclusions and existing test reports; no new result schema or collector is required. After its sanity PASS, `e-2` through `e-7` can all start from `sprint/e-1-workflow`. They have no sibling dependencies and own separate suite directories. Each builds its own artifacts, including wheels where needed. They never require another suite's job to produce artifacts.

The six suite branches stack in completion order; the coordinator records the actual predecessor and layer before final publication. Parent branch movement affects finalization, not starting scoped work. Each sprint has its own dev, sanity and QA bead, all under the phase root. QA blocks on its sanity bead; keeping these as siblings avoids an open QA child preventing dev completion. Sanity reviews every numbered deliverable; PASS dispatches QA immediately and failure returns to its dev owner. QA pins the actual sanity PASS commit before dispatch; no commit is invented during planning.

Implementation starts from the approved initial workflow head. Existing Phase D test sources are inventory, not authorization to merge Phase D. Consume them from develop after their normal landing; do not import the frozen stack or introduce a phase-D dependency edge.

## Dispatch contract

```text
just integrate <branch>
just integrate <branch> --suite wheel-cli-viewer --os macos
just integrate <branch> --suite wheels,collector --os windows,linux
```

The workflow has only `workflow_dispatch`, with equivalent suite and OS inputs. The default selects all six suites on macOS, Windows and Linux: 18 independent suite/OS jobs. Validate selection before dispatch. All jobs use the branch SHA pinned by that run. Job matrix fail-fast is disabled; assertion failure does not stop other runnable tests within the job. Setup or compilation failure is reported explicitly and cannot be represented as successful coverage.

The command reports the run URL; native GitHub tooling exposes every selected job result. Missing selected results fail the aggregate. Partial runs are clearly labelled; only the complete selection can provide phase-ending integration coverage. Workflow dispatch requires the workflow on the repository's default branch; e-1 documents the bootstrap without silently adding PR/push triggers.

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
| e-3 | Remove the viewer job from `otlp-conformance.yml`. |
| e-4 | Remove the collector job from `otlp-conformance.yml`; whichever replacement lands last deletes the empty workflow. |
| e-7 | Remove the replaced `packaged-consumer` job in `binding-runtime.yml`; preserve unrelated native-contract coverage and required release aggregation. |

Shared-file retirement edits are reconciled at publication; they add no sibling development dependencies. e-5 and e-6 reuse the narrow existing assertions while preserving the distinct B.4a and TypeScript publisher qualification interfaces. Do not import their whole release pipelines, API extraction, unit tests or lint into integration jobs. `telemetry-platforms.yml` is a six-target build check, not duplicate integration execution, and is not retired by this plan. The importer suite is not silently added to these six suites.
