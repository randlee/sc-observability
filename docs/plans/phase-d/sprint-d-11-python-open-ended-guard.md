# d-11: Open-ended Python distribution regression guard

## Plan metadata

- Wave: 18
- Branch: `sprint/d-11-python-open-ended-guard`
- PR target: `sprint/d-9-otlp-conformance`
- Blocked by: `obs-d-18-sanity`
- Owned paths:
  - `docs/plans/phase-d/sprint-d-11-python-open-ended-guard.md`
  - `docs/project-plan.md`
  - `scripts/ci/_python_distribution.py`
  - `scripts/ci/tests/test_python_distribution.py`
  - `scripts/ci/validate_python_distribution.py`

## Goal and dependency

After D.18 activates D.10's release policy, make CI fail if the Python distribution stops being open-ended.
The required contract is PyO3 `abi3-py310`, built wheels tagged `cp310-abi3`,
and `requires-python = ">=3.10"` with no upper bound.


## Deliverables

1. Extend the existing `_python_distribution.py` source inspection and
   `inspect_wheel` result with `expected_requires_python`. Parse—not grep—the
   Python `pyproject.toml`, `Cargo.toml`, and built wheel `METADATA`; assert
   `requires-python` has lower bound 3.10 and no upper/exclusion cap. Retain
   the existing PyO3/maturin and `cp310-abi3` tag validators; do not create a
   second wheel-tag validator.
2. Expose the extension through the validator already invoked by the existing B.4a aggregate job before
   publishing eligibility, and add unit fixtures that fail for `>=3.10,<3.13`,
   non-abi3/cp311 ABI tags, missing abi3 feature, and a wrong platform tag.
3. Wire D.10's PE ARM64 helper into verify_native_architecture, require six wheels/30 native cells from one immutable source, and reject missing/duplicate/wrong-target records alongside the source/artifact metadata comparison.
4. Document the invariant and explicit change-control rule: raising the floor
   or adding an upper bound requires a separately approved compatibility
   decision, updated supported-interpreter matrix, and this guard's expected
   values—not an incidental packaging edit.


## This Sprint Does Not Close

This guard does not add future Python versions, alter the minimum version, or
implement #88/OTEL functionality.


## Design

## Python qualification boundary

D.11 consumes D.18's six-platform release policy and D.10's ARM64 helper. Extend the existing _python_distribution.py/validate_python_distribution.py consumer and existing unit fixtures; do not create a second tag parser or change D.10's workflow. Its existing aggregate invocation picks up the validator behavior. Parse source TOML and wheel METADATA using existing parsers, reject upper/exclusion bounds, and retain abi3-py310/cp310-abi3. The final matrix is six builds and 30 installed-suite cells (Python 3.10–3.14) on native platforms at the same source/version. Preserve original missing/duplicate/architecture/feature checks. docs/project-plan.md records qualification and change control. The pr_target after D.9 is merge order only; obs-d-18-sanity is the actual blocker. Wave 4 awaits the root user ruling.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.


## Acceptance criteria

## Acceptance criteria

- CI passes on the declared `>=3.10` / `abi3-py310` / `cp310-abi3` contract
  and fails deterministically for every negative fixture.
- A wheel must satisfy both source configuration and produced metadata/tag
  checks; changing only one cannot pass.
- The six D.10 platforms have no Python-version upper bound hidden in PEP 440
  specifier parsing.


## Required validation

- Validator unit/negative tests, source-build validation, and a full B.4a
  reusable workflow run at an immutable SHA.
- Docs consistency plus source/artifact metadata verification.

Concrete validation commands (dispatch at the reviewed implementation SHA):

```bash
python3 -m unittest discover -s scripts/ci/tests -p test_python_distribution.py
bash scripts/ci/validate_docs_consistency.sh
gh workflow run b4a-python-distributions.yml --ref "$(git rev-parse HEAD)" -f source_commit="$(git rev-parse HEAD)"
```

A dispatch receipt is not a pass: the immutable-source workflow and aggregate
job must finish successfully with the matrix required above.


- Final immutable-source evidence covers six wheel builds and 30 native installed-suite cells; PE ARM64 is validated via D.10 helper and missing/duplicate/wrong-target evidence fails. A dispatch receipt never counts as pass.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.

