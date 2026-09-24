---
id: D.6
status: proposed
branch: feature/phase-d-6-python-open-ended-guard
base: develop
---

# D.6 — Open-ended Python distribution regression guard

## Goal and dependency

After D.5, make CI fail if the Python distribution stops being open-ended.
The required contract is PyO3 `abi3-py310`, built wheels tagged `cp310-abi3`,
and `requires-python = ">=3.10"` with no upper bound.

## Deliverables

1. Add a hermetic CI validator that parses—not greps—the Python
`pyproject.toml`, `Cargo.toml`, and built wheel metadata/tags. It asserts
`requires-python` has lower bound 3.10 and no upper/exclusion cap, PyO3 and
maturin features select `abi3-py310`, and wheel tags are stable-ABI
`cp310-abi3` for every policy platform.
2. Run the validator in the normal Python source/distribution workflow before
   publishing eligibility, and add unit fixtures that fail for `>=3.10,<3.13`,
   non-abi3/cp311 ABI tags, missing abi3 feature, and a wrong platform tag.
3. Emit a concise contract receipt identifying parsed metadata, source SHA,
   policy platform list, and validator version; retain it with the existing
   production distribution evidence.
4. Document the invariant and explicit change-control rule: raising the floor
   or adding an upper bound requires a separately approved compatibility
   decision, updated supported-interpreter matrix, and this guard's expected
   values—not an incidental packaging edit.

## Acceptance criteria

- CI passes on the declared `>=3.10` / `abi3-py310` / `cp310-abi3` contract
  and fails deterministically for every negative fixture.
- A wheel must satisfy both source configuration and produced metadata/tag
  checks; changing only one cannot pass.
- The final receipt lists all six D.5 platforms and has no Python-version
  upper bound hidden in PEP 440 specifier parsing.

## Required validation

- Validator unit/negative tests, source-build validation, and a full B.4a
  reusable workflow run at an immutable SHA.
- Docs consistency plus source/artifact receipt verification.

## Non-closure

This guard does not add future Python versions, alter the minimum version, or
implement #88/OTEL functionality.
