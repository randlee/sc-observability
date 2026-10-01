# d-30: python telemetry bindings

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Layer: 3 of the wave-5 stack (d-29 → d-33 → d-30 → d-31 → d-32)
- Assignee / model: cobs / terra (difficulty: normal)
- Closure: `boundary` (consumer)
- Target boundary: `BOUNDARY-ScObservabilityPy`
- Branch: `sprint/d-30-python-telemetry-bindings`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-30-python-telemetry-bindings`
- PR target: `sprint/d-33-durable-store-and-export` (stack order only; no code dependency on d-33)
- Blocked by: `obs-d-29-sanity`
- Requirements: PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-013
- ADRs: ADR-020, ADR-021
- Owned paths:
  - `bindings/python/sc-observability-py/src/lib.rs`
  - `bindings/python/sc-observability-py/src/telemetry/**`
  - `bindings/python/sc-observability-py/pyproject.toml`
  - `bindings/python/sc-observability-py/python/sc_observability/__init__.py`
  - `bindings/python/sc-observability-py/python/sc_observability/__init__.pyi`
  - `bindings/python/sc-observability-py/python/sc_observability/telemetry.py`
  - `bindings/python/sc-observability-py/python/sc_observability/telemetry.pyi`
  - `bindings/python/sc-observability-py/tests/test_telemetry*.py`
  - `bindings/python/sc-observability-py/tests/typing/test_telemetry_typing.py`
  - `docs/plans/phase-d/sprint-d-30-python-telemetry-bindings.md`
- Not owned: `bindings/python/sc-observability-py/Cargo.toml` (d-29) and
  `python/sc_observability/generated/**` (d-19, read-only).

## Relations

- `must_follow` d-29: consumes the `otlp-telemetry` Cargo feature,
  `SubmissionEnvelope::from_json`, `TelemetryClient`, `resolve_config` and
  `load_telemetry_file`, the error codes, `InMemoryTelemetryClient`
  (`sc-observability-types` feature `test-double`), the golden
  fixtures and the list of GIL-releasing calls.
- `parallel_safe` with d-33 and d-31: the owned paths are disjoint.
- Landed predecessors: d-20 (Python package, `src/lib.rs`, `pyproject.toml`
  and the existing tests), d-19 (generated DTO/stubs in
  `python/sc_observability/generated/**`) and d-10 (windows-arm64 wheel build
  in `.github/workflows/b4a-python-distributions.yml` and
  `scripts/ci/*python*`). d-30 takes over the d-20 files listed above, leaves
  d-19's generated files and d-10's scripts and workflow untouched, and
  consumes the d-10 wheel matrix read-only.
- Disjoint from in-flight d-18: d-18's fence contains no
  `bindings/python/**` path.

## Goal

Expose the shared submission contract through the installed Python package,
with matching type stubs, and ship it in release wheels.

## Deliverables

1. Add a native `telemetry` module (`src/telemetry/**`, registered from
   `src/lib.rs` under `#[cfg(feature = "otlp-telemetry")]`). It wraps
   `DurableTelemetryClient`. In test-hooks builds it can instead open
   `InMemoryTelemetryClient`. Python input is passed to Rust as JSON and goes
   through `SubmissionEnvelope::from_json`, never Python-side validation.
   [PHD-005, PHD-009]
2. Add the Python API in `sc_observability.telemetry`: `Telemetry(config=None,
   *, store_path=None, endpoint=None, service_name=None)`, a context manager,
   `emit(*, log=None, span=None, logs=(), spans=(), metrics=(), profiles=None,
   resource=None, scope=None, record_key=None) -> AdmissionReceipt`, plus
   `flush(timeout_s=None)`, `shutdown(timeout_s=None)`, `status(...)` and
   `build_envelope(...) -> str` (canonical JSON, no store). Log-only,
   span-only, metric-only, profile-only and combined input are accepted.
   [PHD-005, PHD-006, PHD-009]
3. Add typed errors: `TelemetrySubmissionError`, `TelemetryAdmissionError`,
   `TelemetryDeliveryError` and `TelemetryConfigError`, all subclasses of the
   package's existing error base. Each exposes `.code`. The receipt, status
   and flush-report types are frozen dataclasses. `__exit__` calls `shutdown`
   and re-raises a delivery or shutdown failure; it is never swallowed.
   [PHD-008, PHD-009]
4. Release the GIL in `open`, `emit`, `flush`, `shutdown` and `status`.
   [PHD-009]
5. Add the stubs `telemetry.pyi` and the `__init__.pyi` exports. Existing
   logging APIs and stubs stay unchanged. [PHD-009]
6. Change the wheel build config: in `pyproject.toml`, set
   `[tool.maturin] features = ["pyo3/abi3-py310", "pyo3/extension-module",
   "otlp-telemetry"]`, so that every release wheel (including the d-10
   windows-arm64 target) ships telemetry. [PHD-009]

## This Sprint Does Not Close

- Real collector delivery from the installed wheel, restart recovery against
  the real store, and viewer readback. d-32 owns them.
- Python/CLI equivalence. d-32 owns it.
- Store, drain and encoding behavior. d-33 owns them.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScObservabilityPy (D6): a wheel built with the release
  config (`maturin build --release --manifest-path bindings/python/sc-observability-py/Cargo.toml`,
  using the `pyproject.toml` features) installs into a fresh venv.
  `import sc_observability.telemetry` succeeds, and
  `sc_observability.telemetry.Telemetry` exists in that installed wheel.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D1, D2): `tests/test_telemetry_golden.py`
  runs against the installed release-config wheel. For every d-29 golden
  fixture, `build_envelope(**input)` equals `expected.envelope.json` (with
  placeholder ID equality), or raises the error class and `.code` named in
  `expected.error.json`.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D2, D3): `tests/test_telemetry_double.py`
  runs against a test-hooks wheel backed by `InMemoryTelemetryClient`. It
  covers each single-signal and combined `emit`; a duplicate `record_key` →
  `receipt.duplicate`; a scripted delivery failure raised from `flush` and
  from `__exit__`; and every error class with its `.code`.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D4): `tests/test_telemetry_gil.py`
  runs a Python thread that keeps advancing during a blocking `flush` on the
  test double, which is scripted to block for 500 ms.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D5):
  `tests/typing/test_telemetry_typing.py` passes the repository's stub-typing
  check. The existing logging tests pass unchanged.
- [ ] boundary:BOUNDARY-ScObservabilityPy: `bash scripts/ci/validate_python_bindings.sh`
  and the boundary validator pass.

## Required validation

```sh
cargo clippy -p sc-observability-py --all-targets --features otlp-telemetry -- -D warnings
cargo test -p sc-observability-py --features otlp-telemetry --locked
bash scripts/ci/validate_python_bindings.sh
bash scripts/ci/validate_repo_boundaries.sh
maturin build --release --manifest-path bindings/python/sc-observability-py/Cargo.toml --out dist
python3 -m venv .venv-d30 && .venv-d30/bin/pip install dist/*.whl pytest
.venv-d30/bin/python -m pytest bindings/python/sc-observability-py/tests -k telemetry
```
