---
id: B.4a
status: proposed
branch: feature/phase-b-4a-python-packaging
base: develop
---

# B.4a — Python distributions and platform qualification

## Goal and dependencies

Deliver installable distributions of the complete B.4 runtime API.
`must_follow` B.4 for tested owned/attached behavior and build configuration;
B.5 `must_follow` B.4a for its qualified distribution baseline. Shared package
sources, lockfiles and CI artifacts preclude parallel_safe work. Parent pushes
trigger merge-forward before every child dev/fix round; parent PR merges first.

## Deliverables (authoritative)

1. Complete maturin packaging in `bindings/python/sc-observability-py/` using
   B.4's locked dependencies, `abi3-py310`, stubs and `py.typed`. Produce wheels
   plus an sdist containing required Rust/Python sources or resolvable registry
   dependencies. Keep extension linking separate from the Rust embedding target.
   Dependency build order follows the existing crate publication rules; no
   workspace-only path may leak into distributable metadata.
2. Add the wheel-build/install CI matrix: GIL-enabled CPython 3.10, 3.11, 3.12,
   3.13 and 3.14 on each of macOS arm64, macOS x86_64, Linux glibc x86_64,
   Linux glibc aarch64 and Windows x86_64. Use manylinux_2_28 on Linux;
   macOS minimum deployment targets are 11.0 arm64 and 10.13 x86_64. One abi3
   wheel per platform is exercised on every listed interpreter; cross-build
   success alone is not runtime evidence. Unsupported dependencies fail the
   gate and require an explicit plan correction, never silent matrix reduction.
3. Extend `scripts/ci/validate_python_bindings.sh` to run B.4's full API/error/
   lifecycle suite and type checks against clean installed artifacts on every
   matrix cell, run the Rust embedding example with its separate link settings,
   and rebuild/install from the sdist outside the checkout. Record exact wheel
   tags, interpreter/platform versions, deployment targets, artifact hashes,
   source revision and per-cell results in `docs/plans/phase-b/handoff-b-4a.md`.

## Artifact contract

No new trait, struct, enum or Python operation is introduced. The exact B.4
public signatures and shared binding contract remain the API. A wheel must
expose the same Result/Failure tags and stubs as a source build; packaging cannot
replace native functionality with a fallback or weaken error handling.
Distribution name remains proposed `sc-observability`; registry availability
and publication belong to B.7.

## Acceptance criteria (authoritative)

- AC1: All 25 interpreter/platform cells install and execute B.4's full owned
  API tests and stubs outside the source checkout, including failure injection,
  integer/diagnostic conversion and lifecycle corner cases. No skipped cell or
  import-only smoke test can close platform qualification.
- AC2: Wheel tags and linked-library requirements match abi3-py310 and the stated
  deployment targets; Linux wheels pass manylinux_2_28 compliance checks. Both
  extension and embedded-host executables link and run correctly on each target.
- AC3: An sdist rebuild in a clean environment produces an installable wheel
  with matching public API and required package data. Missing sources, stubs,
  py.typed, unresolved local dependencies or incorrect architecture fail CI.
- AC4: Handoff evidence identifies every tested immutable artifact and matrix
  result. Reuse of one ABI wheel across interpreters does not skip testing.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_python_bindings.sh
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_docs_consistency.sh
```

The extended validator builds with locked maturin/PyO3, inspects wheel contents
and platform tags, runs Linux compliance inspection, installs into clean venvs,
runs B.4 tests and type checks, then rebuilds the sdist outside the checkout.
CI must aggregate all 25 cell results and fail on absent evidence. Test missing
package data, invalid platform tags and accidental extension-only host linker
flags through packaging fixtures; do not corrupt release artifacts for tests.

## Paths to delete

None.

## Non-closure

No PyPI/crates.io publication (B.7), new runtime API, Handler/context (B.5), async
receipts (B.6), free-threaded Python, PyPy, musl or Windows arm64 support. Those
exclusions do not reduce the required GIL CPython matrix.
