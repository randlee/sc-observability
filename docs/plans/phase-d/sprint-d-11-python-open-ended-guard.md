# d-11: Open-ended Python distribution regression guard

## Plan metadata

- Wave: 18
- Branch: `sprint/d-11-python-open-ended-guard`
- PR target: `sprint/d-9-otlp-conformance`
- Blocked by: `obs-d-18-sanity`
- Owned paths:
  - `scripts/ci/_python_distribution.py`
  - `scripts/ci/validate_python_distribution.py`
  - `scripts/ci/tests/test_python_distribution.py`
  - `docs/project-plan.md`

## Goal and dependency

After D.10, make CI fail if the Python distribution stops being open-ended.
The required contract is PyO3 `abi3-py310`, built wheels tagged `cp310-abi3`,
and `requires-python = ">=3.10"` with no upper bound.


## Deliverables

1. Extend the existing `_python_distribution.py` source inspection and
   `inspect_wheel` result with `expected_requires_python`. Parse—not grep—the
   Python `pyproject.toml`, `Cargo.toml`, and built wheel `METADATA`; assert
   `requires-python` has lower bound 3.10 and no upper/exclusion cap. Retain
   the existing PyO3/maturin and `cp310-abi3` tag validators; do not create a
   second wheel-tag validator.
2. Run the extension in `.github/workflows/b4a-python-distributions.yml`, in the
   existing aggregate distribution-validation job before
   publishing eligibility, and add unit fixtures that fail for `>=3.10,<3.13`,
   non-abi3/cp311 ABI tags, missing abi3 feature, and a wrong platform tag.
3. Add only the missing source/artifact metadata comparison; CI pass/fail is
   the consumer-facing result.
4. Document the invariant and explicit change-control rule: raising the floor
   or adding an upper bound requires a separately approved compatibility
   decision, updated supported-interpreter matrix, and this guard's expected
   values—not an incidental packaging edit.


## Non-closure

This guard does not add future Python versions, alter the minimum version, or
implement #88/OTEL functionality.


## Design



## Owned Paths and Exact Targets

- `.github/workflows/b4a-python-distributions.yml`
- `scripts/ci/_python_distribution.py`
- `scripts/ci/validate_python_distribution.py`
- `scripts/ci/tests/test_python_distribution.py`
- `docs/project-plan.md`
- `release/python-platform-policy.json`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.

`_python_distribution.py` owns the source/wheel inspection and architecture
checks; reuse its existing result and the existing B.4a aggregate job.
The Python Cargo/pyproject metadata is inspected, not changed by this sprint.

## Implementation targets


- `scripts/ci/_python_distribution.py`: implement open-ended platform inventory assertions (deliverable 1).
- `scripts/ci/validate_python_distribution.py`: expose the regression guard command (deliverable 2).
- `scripts/ci/tests/test_python_distribution.py`: cover discovered platform matrix changes (deliverable 3).
- `docs/project-plan.md`: document guard ownership (deliverable 4).

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


