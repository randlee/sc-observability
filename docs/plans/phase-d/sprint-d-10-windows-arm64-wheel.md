# d-10: Windows ARM64 Python wheel support

## Plan metadata

- Wave: 1
- Branch: `sprint/d-10-windows-arm64-wheel`
- PR target: `integrate/phase-d`
- Blocked by: `obs-phase-d-plan-qa`
- Owned paths:
  - `.github/workflows/b4a-python-distributions.yml`
  - `scripts/ci/prepare_python_distributions.py`
  - `release/python-platform-policy.json`

## Goal and dependency

D.10 follows D.4 for release-inventory ownership and remains independent of D.5–D.9. Extend the qualified Python
distribution platform matrix from five to six wheels by adding native Windows
ARM64 support. The Rust target is `aarch64-pc-windows-msvc`; the wheel tag is
`win_arm64`.


## Deliverables

1. Preflight GitHub-hosted Windows ARM64 and native CPython ARM64 availability
   for every supported 3.10–3.14 interpreter. If any cell is unavailable, D.10
   remains open; x64 emulation or a cross-build is not native evidence. Then
   add a `windows-arm64` policy row with an ARM64 Windows runner,
   `machine: ARM64`, `wheel_platform: win_arm64`, and explicit target-triple
   handling. Update every policy cardinality/assertion and artifact inventory
   from five/25 to six/30 cells without weakening duplicate/missing checks.
2. Update the reusable B.4a workflow so the ARM64 wheel is built from the same
   immutable sdist/source commit and installed on native Windows ARM64 for all
   currently supported Python interpreters. Preserve Windows supervision and
   bounded async proof behavior.
3. Extend `_python_distribution.py::verify_native_architecture` with the PE
   ARM64 machine value `0xAA64` for `win_arm64`, and add positive/negative PE
   fixtures. Extend all policy JSON, platform maps, cardinality assertions,
   and distribution validators to require a `cp310-abi3-win_arm64` wheel,
   correct architecture/linkage metadata, one wheel per platform, the same
   production feature set, and one same-source sdist across all six platforms.
4. Add documentation and release inventory entries describing the six-platform
   matrix and target triple; retain current five platforms unchanged.


## Non-closure

No Windows ARM64 registry publication, universal Windows wheel, or support for
another Python ABI baseline.


## Design

## Owned Paths and Exact Targets

- `.github/workflows/b4a-python-distributions.yml`
- `scripts/ci/_python_distribution.py`
- `scripts/ci/prepare_python_distributions.py`
- `scripts/ci/validate_python_distribution.py`
- `scripts/ci/tests/test_python_distribution.py`
- `release/release-inventory.json`
- `docs/project-plan.md`
- `release/python-platform-policy.json`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.

Must follow D.4 because both update release/release-inventory.json and the version-bearing distribution baseline. It remains independent of D.5–D.9; Python policy remains D.10-owned.

`_python_distribution.py` owns the source/wheel inspection and architecture
checks; reuse its existing result and the existing B.4a aggregate job.
The Python Cargo/pyproject metadata is inspected, not changed by this sprint.



## Acceptance criteria

## Acceptance criteria

- An immutable-source CI run has one successful native wheel build and five
  successful installed-suite cells for `windows-arm64`; all six wheel builds
  and 30 cells pass aggregate validation from one immutable source/version
  baseline.
- The ARM64 wheel installs in an isolated ARM64 Windows environment and passes
  the same public suite, typing, embedding, negative/offline, and private
  companion checks as the other Windows wheel where applicable.
- Aggregate validation rejects a missing, duplicate, wrong-tag, wrong-target,
  or cross-built-but-not-native-executed ARM64 evidence record.
- `_python_distribution.py::verify_native_architecture` accepts an authentic
  PE `0xAA64` binary and rejects x86/x64/malformed payloads labeled ARM64.


## Required validation

- Focused policy/validator tests including negative ARM64 fixtures.
- Reusable B.4a workflow dispatched at an exact SHA and aggregate validation
  of the six wheels/30 cells.
- Existing workspace/Python packaging gates remain green.

Concrete validation commands (dispatch at the reviewed implementation SHA):

```bash
python3 -m unittest discover -s scripts/ci/tests -p test_python_distribution.py
bash scripts/ci/validate_docs_consistency.sh
gh workflow run b4a-python-distributions.yml --ref "$(git rev-parse HEAD)" -f source_commit="$(git rev-parse HEAD)"
```

A dispatch receipt is not a pass: the immutable-source workflow and aggregate
job must finish successfully with the matrix required above.


