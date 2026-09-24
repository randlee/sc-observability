---
id: D.5
status: proposed
branch: feature/phase-d-5-windows-arm64-wheel
base: develop
---

# D.5 — Windows ARM64 Python wheel support

## Goal and dependency

After D.4, extend the qualified Python distribution platform matrix from five
to six wheels by adding native Windows ARM64 support. The Rust target is
`aarch64-pc-windows-msvc`; the wheel tag is `win_arm64`.

## Deliverables

1. Add a `windows-arm64` policy row with a GitHub-hosted ARM64 Windows runner,
   `machine: ARM64`, `wheel_platform: win_arm64`, and explicit target-triple
   handling. Update every policy cardinality/assertion and artifact inventory
   from five/25 to six/30 cells without weakening duplicate/missing checks.
2. Update the reusable B.4a workflow so the ARM64 wheel is built from the same
   immutable sdist/source commit and installed on native Windows ARM64 for all
   currently supported Python interpreters. Preserve Windows supervision and
   bounded async proof behavior.
3. Extend distribution validators to require a `cp310-abi3-win_arm64` wheel,
   correct architecture/linkage metadata, one wheel per platform, the same
   production feature set, and one same-source sdist across all six platforms.
4. Add documentation and release inventory entries describing the six-platform
   matrix and target triple; retain current five platforms unchanged.

## Acceptance criteria

- An immutable-source CI run has one successful native wheel build and five
  successful installed-suite cells for `windows-arm64`; all six wheel builds
  and 30 cells pass aggregate validation.
- The ARM64 wheel installs in an isolated ARM64 Windows environment and passes
  the same public suite, typing, embedding, negative/offline, and private
  companion checks as the other Windows wheel where applicable.
- Aggregate validation rejects a missing, duplicate, wrong-tag, wrong-target,
  or cross-built-but-not-native-executed ARM64 evidence record.

## Required validation

- Focused policy/validator tests including negative ARM64 fixtures.
- Reusable B.4a workflow dispatched at an exact SHA; retain wheel hashes,
  evidence inventory, and CI URLs for six wheels/30 cells.
- Existing workspace/Python packaging gates remain green.

## Non-closure

No Windows ARM64 registry publication, universal Windows wheel, or support for
another Python ABI baseline.
