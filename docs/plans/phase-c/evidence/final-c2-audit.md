# Final C.2 acceptance audit — no inferred signoff

Audited production source: frozen PR190
`09e0ce3bc03bed4ffc78e5467f6c6ec6093b6eb8`. This child edits only documentation
and retained evidence; its source/build/installer inputs remain identical.
The authoritative C.2 sprint has **six numbered deliverables**. The earlier
seven-item QA denominator was a reviewer checklist and was explicitly corrected
by [quality-mgr](https://github.com/randlee/sc-observability/pull/190#issuecomment-5744023212); it is not a seventh sprint requirement.

## Deliverable audit

| Deliverable | Current result | Concrete evidence / remaining condition |
| --- | --- | --- |
| 1 inventory | PASS | Locked root/standalone Cargo metadata cross-check ten Rust crates; manifest separately lists Python sdist + five wheel targets and npm client. `inventory.json` records the exact surface. |
| 2 complete packages | PASS | `nine-root-stage.json` and full package log prove 9 root archives at 09e0ce3; `tauri-package.json`/log prove standalone verification against that stage; `npm-build-pack.txt` lists 16 files after build. Python run 35458975629 passes all 5 builds / 25 cells; downloaded production sdist/five-wheel hashes verified. |
| 3 secret scopes | PASS names-only | `secret-names.json`: repository CARGO_REGISTRY_TOKEN; npm/NPM_TOKEN; pypi/PYPI_API_TOKEN; testpypi/TEST_PYPI_API_TOKEN. Missing npm environment is resolved. No secret value read or printed. |
| 4 platforms | PASS | PR190 run 35423459204 passes Linux/macOS/Windows real IPC/artifact gates at 09e0ce3. Current Python 35458975629 passes all 33 jobs, including the 5-platform/25-cell aggregate, at the same 09e0ce3 source. |
| 5 retries | PASS | `retry-tests.txt`: 11 staged-package/retry tests. Tests mock subprocesses and forbid sockets; they establish orchestration argument/exit/existence decisions, not internals of third-party clients. |
| 6 later read-only acceptance | Specified; no publish | `npm view @sc-observability/client versions`; per-crate crates.io version API or `cargo search <crate> --limit 1`; `python3 -m pip index versions sc-observability`. Registry absence before release is expected and is not success after release. Earlier scoped QA accepted the command probes; no write credentials are required. |

**Required package/platform evidence is now PASS; lead acceptance remains pending.**
The current Python aggregate and downloaded artifacts have been verified.
Root/aobs must still explicitly review the full audit and record lead completeness.
No lead/owner signoff is invented. No separate owner signoff is invented as a sprint
closure requirement. The existing C.1-before-C.2 merge dependency remains a
merge/completion condition, not permission for this child to merge anything.

## Actual preflight and CI scope

[35423505198](https://github.com/randlee/sc-observability/actions/runs/35423505198)
passed at 09e0ce3, actual summary `failed=[] blocked=[]`. Raw selected summary
and GitHub job metadata are retained in `final-audit/preflight-summary.txt` and
`preflight-run.json`. All 28 checks on PR190 are SUCCESS in `pr190-checks.json`.
These results are current for source-equivalent documentation changes; they do
not substitute for the separately required full Python packaging matrix.

The owner authorized the single candidate-tag exception:
`release-candidate-v1.4.0`, annotated object
`facc6064c14ef3c0059c3a29b1dfe36805b1fbc5`, peeling to
`2656017c036bc42686bf12100051911d908e5449`. Publisher created only that ref.
Preflight verified its ancestry. No release tag v1.4.0, registry publication,
GitHub Release, publish dispatch or merge occurred. This exception does not
approve candidate-to-release drift or later publication.

## Full package proof, not the six-crate historical stage

All 9 root crates are unpublished at target 1.4.0 per actual registry probes
recorded by the staging command; one metadata-derived closure is packaged in
publish order and inspected, followed by standalone Tauri against the extracted
archives. Python Rust-crate packaging is distinct from Python wheel/sdist proof.

```sh
python3 scripts/ci/prepare_release_staged_packages.py --source /Users/randlee/github/sc-observability-worktrees/fix/phase-c-generation-metadata --version 1.4.0 --output /tmp/phase-c-final-nine-stage --target-dir /tmp/phase-c-final-nine-target
python3 scripts/ci/validate_tauri_staged_package.py --source /Users/randlee/github/sc-observability-worktrees/fix/phase-c-generation-metadata --stage /tmp/phase-c-final-nine-stage --version 1.4.0 --output /tmp/phase-c-final-tauri-proof.crate > /tmp/phase-c-final-tauri-driver.txt 2>&1
# In bindings/typescript:
npm ci --ignore-scripts && npm run build && npm pack --dry-run --json
```

Nine-root stage-manifest SHA256:
`f066b83764746407b513f0ed02f313fcd31d6aa6d5e46acb04b7d8dce73e4f5b`.
Tauri archive SHA256:
`3825a1ddfbed1d9fff46164a6edca7f30a534e806e89abb62ac44ab471b9a06d`.
Archives remain at the commands' temporary output paths; raw logs, exact source
commit, archive hashes and commands are retained here. The previous bare npm
pack's 4-file listing excluded the declared dist entries and was incomplete;
the actual clean install/build now lists 16 files, including index.js/index.d.ts.

## Python evidence provenance

Historical run 35408912984 at 340f8aae522c9cd10296748aa6d82c764bc196e5
passed 33 jobs, including 5 builds and 25 installed cells. Downloaded inventory
SHA256 is `d986bf059561d2115cd6babb4135444f60d7ae7aa2f41edd838dfa6d307b0ef1`,
matching the recorded aggregate digest. It is retained as
`historical-python-production-artifacts.json` and is **not** relabeled current.

`qualified-path-comparisons.json` contains the actual diff command/results.
Cargo metadata additions are consumed by `prepare_python_distributions.py`
through Cargo packaging, bundle manifests and the embedding manifest. Therefore
unchanged runtime source alone is insufficient for the literal qualified-path
identity criterion. The lead dispatched non-publishing production run
[35458975629](https://github.com/randlee/sc-observability/actions/runs/35458975629)
with source_commit 09e0ce3 and development=false. It completed SUCCESS: all
33 jobs, including 5 builds, 25 installed full-suite cells and the aggregate.
The raw aggregate log records `B4A_QUALIFIED`; its production-only artifact
10589791842 was downloaded and all six archive hashes verified against the
retained `python-production-artifacts.json`. Inventory SHA256 computed from
those exact bytes is
`97716f4363997422f9bd0a69feb8e5031be89cb7407f8b64d7b73e92cc2d2c70`;
sdist SHA256 is
`341f1a12050753f7afafe71494c6828fc34913daf9b26d0386a4e3067641dade`.
The hosted log records calculation of the inventory output, not its value;
`python-qualification-proof.json` distinguishes that inventory digest from the
uploaded ZIP digest and records all wheel hashes. `python-qualification-run.json`
retains actual job outcomes. The downloaded five build results and 25 cell
results also pass a local replay of the unchanged aggregate validator, including
raw JUnit and production/companion separation checks. Every cell has 83 production
tests, strict typing, production hooks absent and per-interpreter embedding PASS;
`python-cell-build-summary.json` records cell results and raw result hashes.

The qualified source remains 09e0ce3. A recorded Git diff from that source to
the docs child d14943a returns zero changed paths outside docs, conservatively
covering all packaging, runtime, manifest, lock, policy and workflow inputs.
This follow-up also edits only docs/evidence. No artifact source SHA is relabeled.

## Source installer and diagnostic disposition

Linux/Windows source receipts from 35422997225 at 3fb321a are retained with exact
platform binary/wheel hashes. Installer/verifier inputs have zero diff to 09e0ce3.
That reuse is distinct from later consumer diagnostics: the hosted historical
stdout has 41 findings; later manifest correction leaves 18, status=fail.
No source workflow modification/redundant installation is needed for docs only.

23 fixed reports are 22 missing inherited metadata fields and 1 private consumer
path-dependency-version report, across 12 Cargo manifests. Commit aec16d6 also
adds workspace authors and changes consumer dependency to workspace inheritance:
same resolved path/options/features, existing workspace version 1.4.0. The exact
Phase C adaptation record proves this dependency edit and metadata additions;
it does not weaken immutable original Phase B source/release snapshots.

`sc-lint-current-18.json` is the unchanged raw result; `sc-lint-dispositions.json`
retains each diagnostic with its own rationale and source/config citations.
The six SCCs are intra-crate type-owner graphs; the declared architecture uses
crate dependency boundaries with outside_owner_crate reference scope. The
callback graph has transient strong retention with explicit bounded drain/cancel
paths, unlike the five builder/guard construction pairs. All 12 self-loop reports
are advisory under pinned upstream 002/003 policy, and their bodies distinguish
value construction and typed forwarding from recursive execution.

| Finding | Tool rule | Source-specific interpretation | First source citation |
| --- | --- | --- | --- |
| PHC-LINT-01 | SCB-CYCLE-001 | bounded callback retention | `crates/sc-observability-binding-runtime/src/callback.rs:8` |
| PHC-LINT-02 | SCB-CYCLE-001 | borrowed admission guard | `crates/sc-observability-binding-runtime/src/coordinator.rs:50` |
| PHC-LINT-03 | SCB-CYCLE-001 | observer registration guard | `crates/sc-observability-binding-runtime/src/operation.rs:32` |
| PHC-LINT-04 | SCB-CYCLE-001 | borrowed context guard | `crates/sc-observability-log/src/context.rs:4` |
| PHC-LINT-05 | SCB-CYCLE-001 | test-only release guard | `crates/sc-observability/src/maintenance.rs:836` |
| PHC-LINT-06 | SCB-CYCLE-001 | consuming builder | `crates/sc-observe/src/lib.rs:178` |
| PHC-LINT-07 | SCB-CYCLE-002 | inherent/trait forwarding identity | `crates/sc-observability-otlp/src/lib.rs:255` |
| PHC-LINT-08 | SCB-CYCLE-002 | inherent construction forwarding | `crates/sc-observability/src/runtime.rs:352` |
| PHC-LINT-09 | SCB-CYCLE-003 | Clone value construction | `crates/sc-observability-binding-runtime/src/lib.rs:136` |
| PHC-LINT-10 | SCB-CYCLE-003 | enum value construction | `crates/sc-observability-log/src/handle.rs:37` |
| PHC-LINT-11 | SCB-CYCLE-003 | enum value construction | `crates/sc-observability-log/src/handle.rs:51` |
| PHC-LINT-12 | SCB-CYCLE-003 | typed/legacy projector forwarding | `crates/sc-observability-otlp/src/projectors.rs:132` |
| PHC-LINT-13 | SCB-CYCLE-003 | typed/legacy projector forwarding | `crates/sc-observability-otlp/src/projectors.rs:205` |
| PHC-LINT-14 | SCB-CYCLE-003 | typed/legacy projector forwarding | `crates/sc-observability-otlp/src/projectors.rs:167` |
| PHC-LINT-15 | SCB-CYCLE-003 | arithmetic value construction | `crates/sc-observability-types/src/primitives.rs:80` |
| PHC-LINT-16 | SCB-CYCLE-003 | arithmetic value construction | `crates/sc-observability-types/src/primitives.rs:88` |
| PHC-LINT-17 | SCB-CYCLE-003 | typed/legacy sink forwarding | `crates/sc-observability/src/sinks.rs:471` |
| PHC-LINT-18 | SCB-CYCLE-003 | typed/legacy sink forwarding | `crates/sc-observability/src/sinks.rs:306` |

The tool's001 hard-failure classification is retained, not overridden. This
review finds no demonstrated consumer rule violation from these graph reports;
lead/quality owns closure. No global suppression, runtime redesign or sc-lint
PR160 adoption/review is part of this work. Architecture rules: docs/architecture.md
section 6; docs/requirements.md OBS-024 and PHB-011/013; the cited boundary files.
The structured record links pinned upstream policy/analysis and relevant tests.

## Remaining evidence before closure

- Explicit root/aobs lead acceptance of all six deliverables and ACs; until then
  frontmatter stays in_progress. The documented C.1 merge prerequisite also
  remains in force for sprint completion/mergeability.
- Scoped independent QA of this evidence layer. No additional upstream sc-lint
  review or owner-signoff requirement is invented.
