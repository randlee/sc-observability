# CI scope and retirement policy

Effective 2026-09-26, under the user-directed `obs-t5h` / `obs-ci-trim`
cleanup. A job that runs on sprint or fix PRs must name its consumer, enforced
gate, observed defect or existing regression case, and retirement condition.
Without that record it does not run on a sprint PR. New checks require an
identified need; historical evidence alone is not a permanent product gate.

Public API governance is report-only for pull requests whose base is neither
`develop` nor `main`. Both semver and approval checks still run; findings
produce a warning, step-summary diff and uploaded artifact. PRs into `develop`
or `main`, push events and manual runs are strict. There is no head-branch
condition. API diff exit 1 means a change to assess; diff tool failures are
blocking in strict mode. Local `just public-api` remains an explicit strict check.

Platform qualification has a separate rule: PRs into `develop` or `main`,
explicit dispatch and reusable/non-PR qualification run all
platforms.
Relevant-path filters apply before jobs start. An intermediate PR, including one into `integrate/*`, retains
Ubuntu coverage; Windows/macOS and real IPC qualification run at integration.
Aggregators run under the same condition as their required platform proofs;
no aggregator accepts a partial platform inventory. Release workflows and
publication checks remain strict.

## Intermediate PR job inventory

Each row names the existing regression motivating the check, rather than
claiming a new production incident. The historical import identity failures
on PRs 233–235 are the observed reason for retiring those checks.

| Jobs | Consumer and gate | Observed defect / existing regression | Retirement condition |
| --- | --- | --- | --- |
| CI: `fmt`, `clippy` | Rust maintainers; formatting and compiler lint errors | Compiler/lint failures before tests; existing workspace gate | Compiler/build tooling replaces the gate with equivalent coverage |
| CI: `docs-consistency` | API consumers; normative docs and rustdoc agree | Existing docs consistency regression cases and missing-doc checks | Normative document generation replaces these checks |
| CI: `dependency-bans` | Lower-layer consumers; neutral dependency graph | Existing forbidden-dependency and binding-runtime boundary checks | Architectural dependency restrictions are retired |
| CI: `version-literals` | Package consumers; one coherent release train | Existing version and exact macro-pin mismatch rejection | Packages stop using a coordinated release train |
| CI: `public-api-governance` | Integration reviewer; visible API diffs, report-only for PRs except bases develop/main | Phase D missing scoped approvals before integration ownership closes | Integration no longer needs intermediate API reports |
| CI: `manifest-validation` | Release maintainer; publish inventory, install contract, retry correctness | `test_release_artifacts`, `test_prepare_release_staged_packages`, `test_publish_retry_idempotency` | Publish/install tooling is replaced and its coverage moves with it |
| CI: `test` (Ubuntu) | Rust crate consumers; workspace tests, doctests and log feature fixtures | Existing runtime/bridge regression tests | Consumer contract or supported platform is retired |
| Binding runtime: `native-contract` (Ubuntu) | Core/bridge hosts; debug and release native contract | Existing native runtime conversion, ownership and lifecycle fixtures | Native binding runtime is retired or superseded |
| Binding schema: `binding-schema` | Generated TS/Python model consumers; DTO, schema, typing and isolated bundle | Existing schema/conversion corpus, generator drift and isolated consumer negatives | These generated bindings are retired |
| Python binding runtime: `python-source-runtime` | Owned/attached Python users; source runtime contract | Existing Python ownership, context, timeout and teardown fixtures | Python binding or supported interpreter contract is retired |
| TypeScript/Tauri: `schema-and-contract` | Tauri adapter consumers; schema and packaged source/JS contract | Existing schema corpus, neutral boundaries and artifact build checks | Tauri binding is retired |
| B.2: `package-stage` | Published Cargo users; verified six-package candidate | Existing archive tampering, version, private-package and ambient-resolution regressions | Six-package qualification is replaced by equivalent release coverage |
| B.P2: `package-stage` | Runtime-level consumers; baseline/candidate package contract | Existing runtime-level metadata and evidence regression tests | Baseline migration qualification is retired |
| Python packaging boundaries: `boundaries` | Wheel/sdist consumers; package and platform policy | `test_python_distribution.py` | Python distribution contract is retired |
| sc-lint preflight: `source-consumer` (Ubuntu) | Install consumers; source installer and receipt contract | Existing receipt mismatch/rejection cases | Source-installed sc-lint is no longer supported |

## Integration-only platform work

- CI `test` and binding-runtime `native-contract`: Windows/macOS coverage
  qualifies the composed integration instead of every intermediate layer.
- Binding-runtime `packaged-consumer` and `complete-gate`: the macOS sandbox
  proof and all-platform aggregation need the full platform run.
- B.2 and B.P2 `staged-consumer` and `aggregate-platform-evidence`: platform
  package consumption and complete-evidence aggregation run together.
- TypeScript/Tauri `real-ipc-artifacts` and `all-platforms`: real webview IPC
  and all-platform aggregation qualify integration/release artifacts.
- sc-lint `source-consumer` on Windows: integration proves the second
  installer platform while intermediate PRs keep Ubuntu coverage.

## Retired historical gates

`log-bridge-import-integrity`, the BTIT clone, import/adaptation validators,
Phase B/C snapshot JSON and their dedicated regression suites are retired.
The import was accepted and the crates are now maintained here; source edits
are intentional. The source-revision pin and history prerequisites for binding
generation are also retired because stack rebases rewrite commits. The
existing `validate_binding_artifacts.py` remains the one input/output hash
checker; schema generation, typing, runtime tests and Cargo package checks
remain functional gates. Tests unrelated to these retired provenance checks
are unchanged.

Retired 2026-09-26: `validate_log_import.py`, `_log_metadata_adaptations.py`,
`_log_release_adaptations.py`, Phase B `import-provenance.json`,
`post-import-adaptations.json`, `release-adaptations-b-2.json` and Phase C
`manifest-metadata-adaptations.json`. References in historical sprint plans,
approvals and architecture records describe the acceptance gates at that time;
they do not require restoring these retired gates. OTLP-023 is now enforced by
D8 adapter tests using the retained Phase D `legacy-otlp-provenance.json` as
source authority.
