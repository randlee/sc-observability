---
id: D.11
status: planned
branch: feature/phase-d-11-python-open-ended-guard
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-11-python-open-ended-guard
depends_on: [D.10]
relation: must_follow
assignee: cobs
model_class: terra
owned_docs: [docs/project-plan.md, release/python-platform-policy.json]
---

# D.11 — Open-ended Python distribution regression guard

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

## Non-closure

This guard does not add future Python versions, alter the minimum version, or
implement #88/OTEL functionality.
