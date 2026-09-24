---
id: D.5
status: complete
branch: feature/phase-d-5-windows-arm64-wheel
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-5-windows-arm64-wheel
depends_on: [D.7d]
relation: must_follow
owned_docs: [docs/python-distribution.md, release/python-platform-policy.json]
---

# D.5 — Windows ARM64 Python wheel support

## Goal and dependency

D.5 `must_follow`s D.7d (and therefore transitively follows the D.4 2.0
version baseline). Merge-forward the final pushed D.7d source/version baseline
before implementation and every fix round. Extend that qualified Python
distribution platform matrix from five to six wheels by adding native Windows
ARM64 support. The Rust target is `aarch64-pc-windows-msvc`; the wheel tag is
`win_arm64`.

## Deliverables

1. Preflight GitHub-hosted Windows ARM64 and native CPython ARM64 availability
   for every supported 3.10–3.14 interpreter. If any cell is unavailable, D.5
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

## Acceptance criteria

- An immutable-source CI run has one successful native wheel build and five
  successful installed-suite cells for `windows-arm64`; all six wheel builds
  and 30 cells pass aggregate validation from the final D.7d source/version
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
- Reusable B.4a workflow dispatched at an exact SHA; retain wheel hashes,
  evidence inventory, and CI URLs for six wheels/30 cells.
- Existing workspace/Python packaging gates remain green.

## Non-closure

No Windows ARM64 registry publication, universal Windows wheel, or support for
another Python ABI baseline.
