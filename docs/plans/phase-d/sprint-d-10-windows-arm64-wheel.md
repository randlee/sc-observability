# d-10: Windows ARM64 Python distribution and open-ended guard

Generated projection of `obs-d-10`; the bead is authoritative.

## Plan metadata

- Wave: 1
- Layer: 4
- Assignee / model: lobs / luna
- Relation: `root`
- Closure: `boundary`
- Target boundary: Windows ARM64 Python distribution and open-ended guard
- Branch: `sprint/d-10-windows-arm64-wheel`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-10-windows-arm64-wheel`
- PR target (merge order only): `sprint/d-13-logging-contract`
- Blocked by: `obs-phase-d-plan-qa`
- Requirements: NFR-010, PHB-013, PHB-014, PHC-001, PHC-003, PHC-004, PHC-006
- ADRs: ADR-015, ADR-016
- Owned paths (metadata projection):
  - `.github/workflows/b4a-python-distributions.yml`
  - `docs/plans/phase-d/sprint-d-10-windows-arm64-wheel.md`
  - `scripts/ci/_python_distribution.py`
  - `scripts/ci/prepare_python_distributions.py`
  - `scripts/ci/python_arm64.py`
  - `scripts/ci/tests/test_python_arm64.py`
  - `scripts/ci/tests/test_python_distribution.py`
  - `scripts/ci/validate_python_distribution.py`

## Goal and dependency

Own the native Windows ARM64 build adapter and the open-ended Python distribution regression guard as one independent Python-distribution boundary. D.10 prepares native ARM64 artifacts; its folded D.11 work validates the source/wheel contract and the final six-platform matrix. D.18 owns release-policy/inventory activation and publication; this bead owns the adapter, guard, and qualification evidence. The immutable artifact proof is independently closable and does not wait on D.18 activation.

## Deliverables

1. Preflight a native ARM64 Windows runner and native CPython 3.10–3.14; reject x64 emulation/cross-build as native evidence. Add Windows ARM64 runner/target handling to the existing B.4a workflow using source-SHA-pinned jobs.

2. Update `prepare_python_distributions.py` for `aarch64-pc-windows-msvc`/`win_arm64` and six-platform artifact preparation without editing release policy files.

3. Implement native PE `0xAA64` inspection in `python_arm64.py` with positive/x86/x64/malformed fixtures in `test_python_arm64.py`; expose the helper through the shared native-architecture inspector contract.

4. Provide the exact D.18 policy handoff row: `platform=windows-arm64; machine=ARM64; wheel_tag=win_arm64; rust_target=aarch64-pc-windows-msvc; interpreters=3.10,3.11,3.12,3.13,3.14; native_cells=5`. Existing five platforms remain unchanged; D.18 writes the row in `release/python-platform-policy.json`.

5. Extend `_python_distribution.py` source inspection and `inspect_wheel` with `expected_requires_python`. Parse—not grep—the Python `pyproject.toml`, `Cargo.toml`, and built-wheel `METADATA`; require a lower bound of 3.10 with no upper or exclusion cap. Retain the existing PyO3/maturin and `cp310-abi3` tag validators; do not create a second wheel-tag validator.

6. Expose the guard through the validator already invoked by the existing B.4a aggregate job before publishing eligibility. Add unit fixtures that fail for `>=3.10,<3.13`, non-abi3/cp311 ABI tags, a missing abi3 feature, a wrong platform tag, and source/artifact metadata disagreement.

7. Wire the D.10 PE ARM64 helper into `verify_native_architecture`, require six wheels and 30 native installed-suite cells from one immutable source, and reject missing/duplicate/wrong-target records alongside source/artifact metadata comparison. This evidence closes independently of D.18 release-policy activation.

8. Provide a handoff/specification for aobs to record the invariant and explicit change-control rule in the shared plan: raising the floor or adding an upper bound requires a separately approved compatibility decision, an updated supported-interpreter matrix, and updated guard expected values, not an incidental packaging edit.

## This Sprint Does Not Close

D.18 still owns `release/**` policy/inventory activation and publication. This sprint does not add future Python versions, alter the minimum version, implement #88/OTEL functionality, or claim release publication without the immutable six-platform evidence.
## Design

## Python distribution boundary

D.10 owns native ARM64 build/prepare behavior, the focused PE helper, the source/wheel metadata guard, and the qualification tests in its combined fence. The helper consumes wheel bytes/tag and returns the existing typed validation shape; the validator wires it into `verify_native_architecture` once. Workflow jobs execute against one immutable sdist/source commit, never cross-built evidence disguised as native execution. The existing B.4a aggregate invocation remains the integration hook.

The Python guard parses source TOML and wheel `METADATA`, rejects upper/exclusion bounds, and retains `abi3-py310`/`cp310-abi3` validation. It preserves the original missing/duplicate/architecture/feature checks and requires six builds plus 30 installed-suite cells at one source/version. D.10 sends the invariant/change-control wording to aobs for the shared plan; `docs/project-plan.md` is D.12-owned and outside this fence. D.12 owns the 2.0 Cargo workspace bump and OTLP config contract. D.18 owns all `release/**` policy activation and release baseline/inventory; no release policy file is edited here. The native artifact proof and guard tests are independently closable before that activation.

ADR-015 constrains the embedded-Python/shared-binding assumptions of the Python distribution surface; ADR-016 constrains the B.4a/shared-publishing preflight boundary. D.10 performs preflight and artifact qualification only and does not publish.

### Handoff to obs-d-18

D.10 provides this exact policy row for D.18 to write in `release/python-platform-policy.json`: `platform=windows-arm64; machine=ARM64; wheel_tag=win_arm64; rust_target=aarch64-pc-windows-msvc; interpreters=3.10,3.11,3.12,3.13,3.14; native_cells=5`.

The only file fence is `metadata.owned_paths`; paths mentioned as dependencies are read-only unless that metadata grants ownership.

### Handoff to aobs

The supported Python invariant is `Requires-Python >=3.10`, with no upper bound or exclusion, and `abi3-py310`/`cp310-abi3` remains the artifact contract. Raising the floor or introducing an upper/exclusion bound requires a separately approved compatibility decision, an updated interpreter matrix, and updated guard expected values; it must not arrive as an incidental packaging edit. This specification is handed to aobs for the shared plan; `docs/project-plan.md` remains outside the D.10 fence.
## Acceptance criteria

## Acceptance criteria

- [ ] Deliverables 1–3: `python3 -m unittest discover -s scripts/ci/tests -p test_python_arm64.py` passes actual PE ARM64 and wrong-architecture/malformed cases, and the D.10 workflow/prepare path demonstrates a native Windows ARM64 wheel at an immutable source SHA.
- [ ] Deliverable 4: the sprint projection specifies `windows-arm64`, machine `ARM64`, `win_arm64`, and `aarch64-pc-windows-msvc` consistently and states that no `release/**` file is edited.
- [ ] Deliverables 5–6: `python3 -m unittest discover -s scripts/ci/tests -p test_python_distribution.py` passes the open-ended `>=3.10`/`abi3-py310`/`cp310-abi3` contract and fails deterministically for every negative fixture, including source/artifact metadata disagreement.
- [ ] Deliverable 7: validator tests prove the PE helper is used once by `verify_native_architecture` and reject missing, duplicate, wrong-target, or mixed-source records; qualification evidence covers six wheels and 30 native installed-suite cells and is independently closable before D.18 release-policy activation.
- [ ] Deliverable 8: the aobs handoff/specification records the invariant and change-control rule in the shared plan and agrees with the guard's expected values.
- [ ] The combined boundary does not add future Python versions, raise the minimum version, implement #88/OTEL functionality, activate `release/**`, or publish artifacts; D.18 owns policy activation/publication.
- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass as the lead's intermediate-workspace invariant.
