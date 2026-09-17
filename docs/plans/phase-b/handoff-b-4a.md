---
id: B.4a-python-packaging-handoff
status: in_progress
branch: feature/phase-b-4a-python-packaging
parent: feature/phase-b-4-python
---

# Python distribution qualification

The distribution implementation stages a self-contained sdist through the sole
B.3 source-bundle helper. It retains the helper's original archives, extracted
packages, reviewed lock, vendored registry sources and complete inventory.
The outer extension and Rust host each freeze their own lock against that
layout. Every selected registry identity must retain its reviewed version,
source and checksum; dependencies used only by another package's workspace
dev tests may be pruned from these outer locks. The original complete bundle
and its independently verified lock remain unchanged.

Normal package dependencies retain version requirements. Only the private
artifact root applies bundled first-party patches. No package is published;
B.2's six 1.4.0 candidates are consumed as verified staged archives. B.7 owns
publication and its separate override-free registry proof.

## Execution interface

The existing source gate remains available unchanged:

```sh
bash scripts/ci/validate_python_bindings.sh
```

The same entry point also dispatches immutable artifact qualification:

```sh
python scripts/ci/prepare_python_distributions.py --output /tmp/new-python-stage
bash scripts/ci/validate_python_bindings.sh --distribution build \
  --sdist /tmp/new-python-stage/dist/sc_observability-1.4.0.tar.gz \
  --platform macos-arm64 --checkout "$PWD" --output /tmp/new-python-build
bash scripts/ci/validate_python_bindings.sh --distribution cell \
  --sdist /tmp/new-python-stage/dist/sc_observability-1.4.0.tar.gz \
  --wheel /tmp/new-python-build/sc_observability-1.4.0-cp310-abi3-macosx_11_0_arm64.whl \
  --checkout "$PWD" --output /tmp/new-python-cell
```

Output directories must be fresh. Build tools are provisioned before isolation
using `scripts/ci/python-packaging-requirements.txt` and Rust 1.94.1. Linux
builds use native manylinux_2_28 containers; all test interpreters run natively.
`release/python-platform-policy.json` defines five platforms and five GIL
CPython versions. One wheel per platform is shared across its five cells.

The `qualification-suite.json` contract belongs to the runtime owner. Its
`pytest_paths` must select the complete `tests` directory; `typing_paths` select
installed-package type fixtures. `runtime_complete: true` is required for final
qualification. Optional boolean `asyncio_debug` and `warnings_as_errors` enable
`-X dev` and `-W error`, with the corresponding environment flags, without
removing isolated Python mode. Optional boolean `embedding_in_each_cell` runs
the actual bundled host on every cell interpreter with its own fresh Cargo home
and target, verifies the metadata and linker features, and requires retained
interpreter-matched execution in the aggregate. B.4a defaults this option off;
later runtime suites opt in and reuse the same runner.

Each cell installs into a fresh external venv, verifies that both facade and
native module come from that venv, denies checkout/cache/network access, and
executes the complete installed tests and strict type checks. JUnit must contain
actual tests and no skips or failures. The aggregate cross-checks raw JUnit,
commands, artifact hashes, source SHA, executable headers, wheel tags and the
same test identities in every cell. Separate host builds reject extension-only
linker features. Negative artifacts are disposable copies, never modifications
to the retained candidate.

## Current evidence and remaining gates

The approved production/fault-companion split is implemented in the sole runner.
Production retains one immutable wheel per platform, executes all public tests
and typing fixtures, and checks private hooks are absent. Explicit fault-only
files execute against a separately installed companion from the same sdist;
its recorded feature set adds only `test-hooks`. Both suites reject skips.
The aggregate verifies distinct hashes, exact source feature identities and
separate JUnit evidence, then emits `production-artifacts.json` containing only
the five production wheels. Actual companion qualification awaits the parent
fault-file contract and the final combined candidate.

Source assembly now copies Git-tracked Python and embedding inputs, so runtime
tests can leave ignored bytecode without contaminating the sdist inventory.
Generated files remain untouched. Windows network denial is scoped to each
artifact subprocess, with bounded execution and firewall cleanup between
commands; checkout/cache denial remains active for the proof.
Timed-out artifact commands terminate their process tree, including descendants
holding captured output pipes; a real child-process regression verifies the
bounded failure. Windows execution still needs the final combined CI proof.

This handoff is incomplete. Full runtime qualification awaits the active B.4
owner's completed contract and the final 25-cell execution. Explicit development
runs label every result `development_only`; final aggregation rejects those
results even when their currently available tests pass.

- Local development source `19ab74fa73a387d8a9e647c7253ef4c97a918abe`
  produced sdist SHA-256
  `ebf106e3b62a49d3f5b7b6aae956a5a9d436916caa5caade77c252d873a38947`.
  Its external macOS arm64 wheel and separate embedding executable passed with
  checkout, network and Cargo-cache denial. The subsequent local check also
  rejected all nine negative cases, including actual wheel/host build failures
  after removing an unpublished dependency, registry dependency or target entry.
- [Development run 35208870501](https://github.com/randlee/sc-observability/actions/runs/35208870501)
  at `fb3aa49779eb853f9a4dee02dbf6242476621ace` passed the sdist and both macOS
  native builds. Linux failed its bubblewrap loopback setup, and Windows failed
  a negative fixture's platform-dependent path spelling and PE handle cleanup.
  Those failures are retained as development history and do not qualify cells.
- The corrections add container network-namespace administration, POSIX inventory
  paths, explicit UTF-8 command capture and closing the Windows inspection handle.
  [Development run 35209259060](https://github.com/randlee/sc-observability/actions/runs/35209259060)
  tests those corrections plus the latest inherited parent source.

Final immutable artifact inventory, platform/interpreter results, required
validation logs, two-pass checklist and lead completeness decision remain open.
