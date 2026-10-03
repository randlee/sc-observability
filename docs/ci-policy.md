# CI scope and retirement policy

Effective 2026-09-26, under the user-directed `obs-t5h` / `obs-ci-trim`
cleanup. A job that runs on sprint or fix PRs must name its consumer, enforced
gate, observed defect or existing regression case, and retirement condition.
Without that record it does not run on a sprint PR. New checks require an
identified need; historical evidence alone is not a permanent product gate.

Public API governance is strict on every pull-request base, push event and
manual run. There is no head-branch condition. API diff exit 1 means a change
to assess; any other non-zero exit blocks. Local `just public-api` remains an
explicit strict check.

Platform qualification has a separate rule: PRs into `develop` or `main`,
explicit dispatch and reusable/non-PR qualification run all
platforms.
Relevant-path filters apply before jobs start. An intermediate PR, including one into `integrate/*`, retains
Ubuntu coverage; Windows/macOS and real IPC qualification run at integration.
For PR-triggered workflows, aggregators run under the same condition as their required platform proofs;
no aggregator accepts a partial platform inventory. Release workflows and
publication checks remain strict.

The CI workflow also targets PR bases `sprint/*` and `fix/*`: stacked sprint
PRs target the layer below them (`sprint/<lower>`), and quick-fix PRs target
`fix/*` or another layer. Without these bases, those stack layers get no CI
from this workflow. Retire this added base coverage when stacks stop using
`sprint/*` bases.

Dependency restrictions remain checked by `just lint` through
`scripts/ci/validate_dependency_bans.sh`.

## Intermediate PR job inventory

Each row names the existing regression motivating the check, rather than
claiming a new production incident. The historical import identity failures
on PRs 233–235 are the observed reason for retiring those checks.

| Jobs | Consumer and gate | Observed defect / existing regression | Retirement condition |
| --- | --- | --- | --- |
| CI: `fmt`, `clippy` | Rust maintainers; formatting and compiler lint errors | Compiler/lint failures before tests; existing workspace gate | Compiler/build tooling replaces the gate with equivalent coverage |
| CI: `docs-consistency` | API consumers; public Rust API documentation | Rustdoc missing-doc checks; this is the sole PR docs-consistency execution | Equivalent public API documentation validation replaces the check |
| CI: `version-literals` | Package consumers; one coherent release train | Existing version and exact macro-pin mismatch rejection | Packages stop using a coordinated release train |
| CI: `public-api-governance` | Integration reviewer; strict API diff, semver and approval checks on every PR base, push and manual run | Phase D missing scoped approvals before integration ownership closes | Integration no longer needs public API governance |
| CI: `manifest-validation` | Release maintainer; publish inventory, install contract, retry correctness | `test_release_artifacts`, `test_prepare_release_staged_packages`, `test_publish_retry_idempotency` | Publish/install tooling is replaced and its coverage moves with it |
| CI: `test` (Ubuntu) | Rust crate consumers; workspace tests, doctests and log feature fixtures | Existing runtime/bridge regression tests | Consumer contract or supported platform is retired |
| Binding runtime: `native-contract` (dispatch) | Core/bridge hosts; debug and release native contract | Existing native runtime conversion, ownership and lifecycle fixtures | Native binding runtime is retired or superseded |
| Binding schema: `binding-schema` (dispatch) | Generated TS/Python model consumers; DTO, schema, typing and isolated bundle | Existing schema/conversion corpus, generator drift and isolated consumer negatives | These generated bindings are retired |
| Python binding runtime: `python-source-runtime` (dispatch) | Owned/attached Python users; source runtime contract | Existing Python ownership, context, timeout and teardown fixtures | Python binding or supported interpreter contract is retired |
| TypeScript/Tauri: `schema-and-contract` (dispatch) | Tauri adapter consumers; schema and packaged source/JS contract; non-PR qualification also runs schema and docs consistency; dependency bans and repository boundaries are owned by CI manifest validation and do not run in this gate | Existing schema corpus, neutral boundaries and artifact build checks | Tauri binding is retired |
| Python packaging boundaries: `boundaries` | Wheel/sdist consumers; package and platform policy | `test_python_distribution.py` | Python distribution contract is retired |
| sc-lint preflight: `source-consumer` (dispatch) | Install consumers; source installer and receipt contract | Existing receipt mismatch/rejection cases | Source-installed sc-lint is no longer supported |

## Integration-only platform work

- CI `test`: Windows/macOS coverage qualifies the composed integration instead
  of every intermediate layer.
- Binding-runtime `native-contract`, `packaged-consumer`, and `complete-gate`:
  phase-end dispatch runs the macOS sandbox proof and all-platform aggregation.
- TypeScript/Tauri `real-ipc-artifacts` and `all-platforms`: phase-end dispatch
  runs real webview IPC and all-platform aggregation.
- sc-lint `source-consumer`: phase-end dispatch proves both installer platforms.
- Binding runtime, schema, Python runtime, TypeScript/Tauri, OTLP conformance,
  sc-lint source-preflight, and telemetry end-to-end are dispatch-only during
  Phase D; the phase-end `just integrate` owner runs their retained
  qualification. CI no longer runs the external OTLP SDK fixture or the
  desktop-viewer harness.

## Duplicate-execution ownership

The following map records the one retained owner for checks that formerly ran
more than once with the same operating system, event and configuration. A
different interpreter, profile, platform or produced artifact is not a
duplicate and stays in its consumer gate.

| Check | Retained owner | Removed same-configuration execution | Why the retained owner is distinct |
| --- | --- | --- | --- |
| Workspace formatting | CI `fmt` | Python binding source gate | One workspace formatting check is sufficient. |
| Binding-runtime unit contract | Binding runtime `native-contract` | Python and TypeScript/Tauri gates | It owns the native-contract suite; the TypeScript/Tauri platform gate independently produces its platform evidence. |
| DTO unit tests | CI workspace `test` | Binding-schema gate | The workspace suite owns the package test. |
| Schema generator `--check` | Binding-schema gate | Python binding source gate | The schema gate pins its generator interpreter and schema corpus. |
| Shared Python typing helper | Binding-schema gate | Python binding source gate | The schema gate owns the shared generated-surface typing proof; B.4 keeps its CPython 3.10-specific typing checks. |
| Binding source-bundle unit suite | Binding-schema gate on Linux | TypeScript/Tauri platform gate on macOS and Windows | Linux owns the schema-gate execution; macOS and Windows retain platform-local execution. |
| Tauri unit suite on Linux | TypeScript/Tauri `schema-and-contract` | TypeScript/Tauri real-IPC Linux leg | macOS and Windows retain platform-local unit coverage. |
| Dependency bans | CI manifest validation | Binding-schema and TypeScript/Tauri gates | CI remains the single PR owner. |
| Repository boundaries | CI manifest validation | TypeScript/Tauri gate | CI retains the executable proving-artifact check. |
| Binding-runtime dependency validator | Repository-boundary script | Dependency-ban script | The boundary script also runs its focused regression module. |
| Release-artifact manifest subset | CI's installed publish-kit suite | Later `-k manifest or publish_order` subset | The installed suite is the strict superset in the same job. |
| sc-lint receipt-rejection suite on Linux | CI's installed publish-kit suite | sc-lint preflight Ubuntu leg | Preflight retains the Windows execution that CI does not provide. |

## Release preflight

B.2 (`b2-staged-consumer.yml`) and B.P2 (`bp2-staged-consumer.yml`) staged
qualification are release preflight run on demand through `workflow_dispatch`
by the publisher before publishing, never on sprint or integration PRs or push
events. Their original 1.4.x release qualification is satisfied and those
packages are in use by BTIT; running their published-package checks on Phase D
sprint changes is inappropriate because the published 1.4.x dependencies lack
the new API. The package-stage, staged-consumer and complete-platform evidence
jobs, scripts, fixtures and tests remain available for release qualification.
The checked-in B.P2 authority is
`release/runtime-level-qualification.toml`, whose historical pair is candidate
`1.3.0` against baseline `1.2.0`. That pair is the recorded B.P2 qualification
used for the 1.4.x release; it is not a qualification for a later candidate.
Before a later release, the publisher must update and requalify the authority,
the frozen baseline fixture, the handoff, and the retained evidence before
dispatching B.P2 for that candidate. The publisher then dispatches the
applicable retained workflows on the candidate ref before publishing and
verifies successful completion: B.2 for the current six-package release train,
and B.P2 only when its candidate/baseline authority matches the release plan.
B.2 runs
`test_log_staging.py`; B.P2 runs
`test_validate_runtime_level_qualification_metadata.py`,
`test_validate_runtime_level_platform_evidence.py`,
`validate_runtime_level_qualification_metadata.py` and the `rustfmt --check`
check on `scripts/ci/fixtures/runtime-level-consumer/*.rs`. These checks run
only on dispatch in CI. No PR workflow discovers all of `scripts/ci/tests`:
`ci.yml` runs `test_prepare_release_staged_packages` and
`test_publish_retry_idempotency`, while other PR workflows select their own
binding, packaging and platform suites. Those suites do not run the six
preflight checks listed here. The preflight aggregators always run after their
platform consumers succeed, retaining complete-platform evidence requirements.

The binding source bundle uses B.2 qualified archives only when the B.2 candidate
version matches the workspace release train; otherwise it builds unpublished
Cargo packages from source (obs-ci-stage-bundle). A corrupt or unreadable B.2
stage still fails the bundle build; only a verified release-train mismatch
falls back to source.

## Retired historical gates

`log-bridge-import-integrity`, the BTIT clone, import/adaptation validators,
Phase B/C snapshot JSON and their dedicated regression suites are retired.
The import was accepted and the crates are now maintained here; source edits
are intentional. The source-revision pin and history prerequisites for binding
generation are also retired because stack rebases rewrite commits. The
committed generated-binding source/output hash inventory is also retired.
`validate_binding_generators.py` regenerates and compares the committed schema
and language bindings, checks determinism, and rejects stale generated output.
Pinned generator toolchains, typing, runtime tests, public API/semver checks and
Cargo package integrity checks remain functional gates. Tests unrelated to these
retired provenance checks are unchanged.

Retired 2026-09-26: `validate_log_import.py`, `_log_metadata_adaptations.py`,
`_log_release_adaptations.py`, Phase B `import-provenance.json`,
`post-import-adaptations.json`, `release-adaptations-b-2.json` and Phase C
`manifest-metadata-adaptations.json`. References in historical sprint plans,
approvals and architecture records describe the acceptance gates at that time;
they do not require restoring these retired gates. The Phase D OTLP
transplant provenance manifest and its validator were retired with the
`sync-http` rename; OTLP-023 no longer requires them.
