# d-10: Windows ARM64 Python wheel support

## Plan metadata

- Wave: 3
- Branch: `sprint/d-10-windows-arm64-wheel`
- PR target: `sprint/d-13-logging-contract`
- Blocked by: `obs-phase-d-plan-qa`
- Owned paths:
  - `.github/workflows/b4a-python-distributions.yml`
  - `docs/plans/phase-d/sprint-d-10-windows-arm64-wheel.md`
  - `scripts/ci/prepare_python_distributions.py`
  - `scripts/ci/python_arm64.py`
  - `scripts/ci/tests/test_python_arm64.py`

## Goal

Close the independent native Windows ARM64 build adapter boundary. D.18 owns release policy/inventory and D.11 qualifies the completed six-platform metadata matrix.

## Deliverables

1. Preflight a native ARM64 Windows runner and native CPython 3.10–3.14; reject x64 emulation/cross-build as native evidence. Add Windows ARM64 runner/target handling to the existing B.4a workflow using source-SHA-pinned jobs.

2. Update prepare_python_distributions.py for aarch64-pc-windows-msvc/win_arm64 and six-platform artifact preparation without editing release policy files.

3. Implement native PE 0xAA64 inspection in python_arm64.py with positive/x86/x64/malformed fixtures in test_python_arm64.py; expose the helper to D.11's shared inspector contract.

4. Document the six-platform policy row and target triple in this sprint doc for D.18's release/python-platform-policy.json and release-inventory update; existing five platforms remain unchanged.

## This Sprint Does Not Close

Release policy/inventory activation is D.18; source/wheel metadata guard and final six-build/30-cell evidence are D.11. No publication or new ABI baseline.


## Design

## Python build boundary

D.10 owns native ARM64 build/prepare behavior and a focused PE helper, avoiding edits to D.11's shared _python_distribution.py/validator/test files. The helper takes wheel bytes/tag and returns the existing typed validation shape; D.11 wires it into verify_native_architecture once. D.18 owns all release/** policy activation. Workflow jobs execute against one immutable sdist/source commit, never cross-built evidence disguised as native execution. Existing aggregate invocation remains the integration hook, requiring no D.11 workflow edit. This independent root sprint can preflight/build/test its adapter before release inventory activation; it cannot claim the final 30-cell aggregate.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.


## Acceptance criteria

- [ ] `python3 -m unittest discover -s scripts/ci/tests -p test_python_arm64.py` passes actual PE ARM64 and wrong-architecture cases (D3).
- [ ] The D.10-owned workflow/prepare path demonstrates a native Windows ARM64 wheel and installed-suite execution for 3.10–3.14 at an immutable source SHA; unavailable native cells keep D.10 open (D1/D2).
- [ ] Sprint policy handoff specifies windows-arm64, machine ARM64, win_arm64 and aarch64-pc-windows-msvc consistently; no release/** file is edited (D4).
- [ ] This sprint does not close final six-platform/30-cell aggregate qualification; D.11 does after D.18 activates policy.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.

