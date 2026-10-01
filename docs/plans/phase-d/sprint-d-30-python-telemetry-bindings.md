# d-30: python telemetry bindings

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Stack / layer: `phase-d-wave5` stack, layer 3 (d-29 → d-33 → d-30 → d-31 → d-32; wave-5 ruling R12)
- Assignee / model: cobs / terra
- Difficulty: `normal` (`docs/plans/phase-d/difficulty.csv`)
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
  - `bindings/python/sc-observability-py/tests_telemetry/**` (new; outside `tests/`, so `validate_python_bindings.sh` does not collect it)
  - `bindings/python/sc-observability-py/tests/typing/test_telemetry_typing.py`
  - `docs/plans/phase-d/sprint-d-30-python-telemetry-bindings.md`
- Not owned: `bindings/python/sc-observability-py/Cargo.toml` (d-29) and
  `python/sc_observability/generated/**` (d-19, read-only).

## Relations

- `must_follow` d-29: consumes the `otlp-telemetry` Cargo feature,
  `SubmissionEnvelope::from_json`, `TelemetryClient`, `resolve_config` and
  `load_telemetry_file`, the flush result rules, the error codes,
  `InMemoryTelemetryClient` and `DoubleScript` (`sc-observability-types`
  feature `test-double`, enabled by the py crate's `test-hooks`), the golden
  fixtures, the committed dependency set (no dependency is added here) and
  the list of GIL-releasing calls.
- Sibling note (prose only; the bead relation is `must_follow` d-29): d-30,
  d-33 and d-31 can run in parallel, because their owned paths are disjoint.
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
   `DurableTelemetryClient`. In test-hooks builds only, the private
   constructor `Telemetry._with_test_double(script_json: str | None)` opens
   `InMemoryTelemetryClient::with_script` with a `DoubleScript` JSON
   document. Python input is passed to Rust as JSON (`json.dumps`) and goes
   through `SubmissionEnvelope::from_json`; Python does no validation and no
   value conversion (plain attribute maps are converted in Rust, d-29 "Input
   value forms"). [PHD-005, PHD-009]
2. Add the Python API in `sc_observability.telemetry`: `Telemetry(config=None,
   *, store_path=None, endpoint=None, service_name=None)`, a context manager,
   `emit(input: Mapping[str, Any]) -> AdmissionReceipt`, plus
   `flush(timeout_s=None)`, `flush_submission(submission_id, timeout_s=None)`,
   `shutdown(timeout_s=None)`, `status(...)` and the module function
   `build_envelope(input: Mapping[str, Any]) -> str` (canonical JSON, no
   store). `input` is one d-29 `SubmissionInput` document, the same shape as
   `sc-otel validate --stdin` and every golden `input.json` (including
   `version`). There are no per-signal keyword arguments. Log-only,
   span-only, metric-only, profile-only and combined input are accepted.
   [PHD-005, PHD-006, PHD-009]
3. Add typed errors: `TelemetrySubmissionError`, `TelemetryAdmissionError`,
   `TelemetryDeliveryError` and `TelemetryConfigError`, all subclasses of
   `TelemetryError(Exception)` defined in `telemetry.py` (the existing
   logging API returns `Ok`/`Err` values and has no exception base). Each
   exposes `.code`. The receipt, status
   and flush-report types are frozen dataclasses. `flush`,
   `flush_submission` and `shutdown` raise `TelemetryDeliveryError` exactly
   when the d-29 flush result rules give `Err`, with the report attached.
   `__exit__` calls `shutdown` and re-raises a delivery or shutdown failure;
   it is never swallowed.
   [PHD-008, PHD-009]
4. Release the GIL in `open`, `emit`, `flush`, `flush_submission`,
   `shutdown` and `status`.
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

## Design

The contract types and signatures are in the d-29 doc ("Submission
contract", "Errors and codes", "Configuration and precedence",
"TelemetryClient") and are not restated here. This section covers only what
d-30 writes.

### Module layout

| File | Contents |
| --- | --- |
| `src/lib.rs` | Two added lines, both under `#[cfg(feature = "otlp-telemetry")]`: `mod telemetry;` and `telemetry::register(module)?;` inside the existing `_native` module function. Nothing else in the file changes. |
| `src/telemetry/mod.rs` | `register`, the `NativeTelemetry` class, the `open` and `build_envelope` functions, and the test-hooks `_open_test_double` function. |
| `src/telemetry/client.rs` | `ClientHandle`: `Durable(DurableTelemetryClient)` and, under `test-hooks` only, `Double(InMemoryTelemetryClient)`. It forwards every `TelemetryClient` call to the inner client. |
| `src/telemetry/config.rs` | `ConstructorArgs` (serde form of the Python constructor arguments) → `ConfigOverrides`. Calls `load_telemetry_file` when `config` is set and `resolve_config(ConfigSources::new(&overrides, file, &env))` with `env = \|k\| std::env::var(k).ok()`. |
| `src/telemetry/dto.rs` | `TelemetryResult<T>` with the existing `{"kind": "ok", "value"}` / `{"kind": "error", "error"}` shape, and `TelemetryErrorDto { class, code, message, report: Option<FlushReport> }`, where `class` is `submission`, `admission`, `delivery` or `config`. Values serialize with the d-29 `Serialize` impls. |
| `python/sc_observability/telemetry.py` | `Telemetry`, `build_envelope`, the error classes and the frozen dataclasses (`AdmissionReceipt`, `FlushReport`, `SignalCounts`, `StoreStatus`, `DeliveryStatus`, `LeaseInfo`). It decodes `TelemetryResult` JSON, then either builds a dataclass or raises. |
| `telemetry.pyi`, `__init__.py`, `__init__.pyi` | Stubs and re-exports of the names above. |

### Native surface (PyO3)

This follows the existing binding pattern: JSON strings cross the boundary,
every entry point is wrapped in `contained_json` so a panic becomes an
`internal` error result, and constructors return `(handle | None, result_json)`
like `create_owned`.

```rust
#[pyclass(frozen)]
struct NativeTelemetry { client: ClientHandle }   // ClientHandle: Send + Sync

#[pyfunction] fn open(py: Python<'_>, args_json: &str)
    -> PyResult<(Option<Py<NativeTelemetry>>, String)>;          // GIL released for open
#[pyfunction] fn build_envelope(input_json: &str) -> String;      // from_json(SystemIds) + to_canonical_json; no store, GIL held
#[cfg(feature = "test-hooks")]
#[pyfunction] fn _open_test_double(py: Python<'_>, args_json: &str, script_json: Option<&str>)
    -> PyResult<(Option<Py<NativeTelemetry>>, String)>;          // DoubleScript::from_json + with_script

#[pymethods]
impl NativeTelemetry {
    fn emit(&self, py: Python<'_>, input_json: &str) -> String;   // from_json(SystemIds) then emit
    fn flush(&self, py: Python<'_>, timeout_ms: Option<u64>) -> String;
    fn flush_submission(&self, py: Python<'_>, submission_id: &str, timeout_ms: Option<u64>) -> String;
    fn shutdown(&self, py: Python<'_>, timeout_ms: Option<u64>) -> String;
    fn status(&self, py: Python<'_>, query_json: &str) -> String; // {"summary"} | {"submissions": [..]} | {"record_keys": [..]}
}
```

A `None` timeout uses the resolved `flush_deadline`. `submission_id` and the
query keys are parsed with the d-29 `FromStr` impls, and a parse failure is a
`submission` error.

### GIL release points

Each call in the d-29 GIL list runs its client call inside `py.detach(...)`,
the same call the logging binding uses:

| Python call | Released section |
| --- | --- |
| `Telemetry(...)` | `load_telemetry_file`, `resolve_config` and `TelemetryClient::open` (store open and migration check) |
| `emit` | `SubmissionEnvelope::from_json` and `TelemetryClient::emit` (durable commit) |
| `flush` / `flush_submission` / `shutdown` | the client call, up to its deadline |
| `status` | `TelemetryClient::status` |
| `__exit__` | `shutdown`, as above |

`build_envelope` keeps the GIL, because it does no I/O. The JSON input string
is copied into Rust before `detach`, so no Python object is touched while
the GIL is released.

### Python surface

```python
class Telemetry:
    def __init__(self, config: str | os.PathLike[str] | None = None, *,
                 store_path: str | os.PathLike[str] | None = None,
                 endpoint: str | None = None, service_name: str | None = None) -> None: ...
    def __enter__(self) -> Telemetry: ...
    def __exit__(self, *exc: object) -> None: ...          # shutdown(); re-raises its failure
    def emit(self, input: Mapping[str, Any]) -> AdmissionReceipt: ...
    def flush(self, timeout_s: float | None = None) -> FlushReport: ...
    def flush_submission(self, submission_id: str, timeout_s: float | None = None) -> FlushReport: ...
    def shutdown(self, timeout_s: float | None = None) -> FlushReport: ...
    def status(self, *, submissions: Sequence[str] | None = None,
               record_keys: Sequence[str] | None = None) -> StoreStatus: ...
    @classmethod
    def _with_test_double(cls, script_json: str | None = None, **kwargs: Any) -> Telemetry: ...  # test-hooks only

def build_envelope(input: Mapping[str, Any]) -> str: ...
```

`emit` and `build_envelope` call `json.dumps(input)` and pass the string
through unchanged. A `TypeError` from `json.dumps` raises
`TelemetrySubmissionError` with the `SC_OBSERVABILITY_SUBMIT_INVALID_JSON`
code. `status` with both keyword arguments raises `ValueError`. Error results
map by `class` to `TelemetrySubmissionError`, `TelemetryAdmissionError`,
`TelemetryDeliveryError` (with `.report`, a `FlushReport`) or
`TelemetryConfigError`. An `internal` result raises `TelemetryError`.
`timeout_s` is converted to milliseconds, rounding up.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScObservabilityPy (D6): a wheel built with the release
  config (`maturin build --release --manifest-path bindings/python/sc-observability-py/Cargo.toml`,
  using the `pyproject.toml` features) installs into a fresh venv.
  `import sc_observability.telemetry` succeeds, and
  `sc_observability.telemetry.Telemetry` exists in that installed wheel.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D1, D2): `tests_telemetry/test_telemetry_golden.py`
  runs against the installed release-config wheel (Required validation,
  "release-config wheel" block). For every d-29 golden fixture,
  `build_envelope(json.load(input.json))` equals `expected.envelope.json`
  (with placeholder equality), or raises the error class and `.code` named in
  `expected.error.json`.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D2, D3): `tests_telemetry/test_telemetry_double.py`
  runs against the test-hooks wheel (Required validation, "test-hooks wheel"
  block), using `Telemetry._with_test_double`. It covers each single-signal
  and combined `emit`; a duplicate `record_key` → `receipt.duplicate`; a
  scripted `fail` raised from `flush` and from `__exit__`; a scripted `stall`
  raised from `flush` as a deadline; a scripted admission rejection; and
  every error class with its `.code`.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D4): `tests_telemetry/test_telemetry_gil.py`
  runs against the test-hooks wheel with `{"flush_delay_ms": 500}`. The main
  thread sets a `threading.Event` and calls `flush`; a background thread
  waits on that event and sets a second event; the main thread asserts the
  second event is set as soon as `flush` returns. The test has no sleeps and
  no elapsed-time assertions.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D3, D4): in both pytest runs a
  skipped test is a failure; the `-rs` summary must report 0 skipped.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D5):
  `MYPYPATH=bindings/python/sc-observability-py/python .venv-d30-hooks/bin/mypy --strict --python-version 3.10 bindings/python/sc-observability-py/tests/typing/test_telemetry_typing.py`
  passes. The existing logging tests and stubs pass unchanged.
- [ ] boundary:BOUNDARY-ScObservabilityPy: `bash scripts/ci/validate_python_bindings.sh`
  and the boundary validator pass. That script builds its own test-hooks
  wheel and runs `tests/` only; it is not expected to run the telemetry
  tests, which live in `tests_telemetry/` and run in the two named builds.
  Every build command below passes the complete feature list, so no result
  depends on whether maturin merges CLI `--features` with the pyproject
  `features` list.
- [ ] boundary:ADR-021 (D6, platform matrix): `b4a-python-distributions.yml`
  is dispatched on the d-30 head SHA
  (`gh workflow run b4a-python-distributions.yml --ref sprint/d-30-python-telemetry-bindings -f source_commit=<head> -f development=false`;
  `development=true` only if the d-10 runtime is still incomplete on the
  base, in which case the wheel cells are the evidence). Every `wheel` matrix
  cell, which builds with the `pyproject.toml` features including
  `otlp-telemetry`, is green, and the run URL is recorded in the PR body.
- [ ] boundary:ADR-020 (public surface frozen; wave-5 ruling R17): at the
  d-30 head, `python3 scripts/ci/validate_public_api.py diff --crate sc-observability-py`
  reports the same `api_sha256` as the signed d-29 approval record, and
  `cargo public-api --manifest-path bindings/python/sc-observability-py/Cargo.toml -sss --features otlp-telemetry | shasum -a 256`
  equals its `feature_api_sha256["otlp-telemetry"]`.

## Required validation

```sh
cargo clippy -p sc-observability-py --all-targets --features test-hooks,otlp-telemetry -- -D warnings
cargo test -p sc-observability-py --features otlp-telemetry --locked
bash scripts/ci/validate_python_bindings.sh
bash scripts/ci/validate_repo_boundaries.sh

# release-config wheel: pyproject features (pyo3/abi3-py310, pyo3/extension-module, otlp-telemetry)
maturin build --release --manifest-path bindings/python/sc-observability-py/Cargo.toml \
  --features pyo3/abi3-py310,pyo3/extension-module,otlp-telemetry --out dist-release
python3 -m venv .venv-d30-release && .venv-d30-release/bin/pip install dist-release/*.whl pytest
.venv-d30-release/bin/python -m pytest -rs bindings/python/sc-observability-py/tests_telemetry/test_telemetry_golden.py

# test-hooks wheel: release features plus test-hooks
maturin build --manifest-path bindings/python/sc-observability-py/Cargo.toml \
  --features pyo3/abi3-py310,pyo3/extension-module,otlp-telemetry,test-hooks --out dist-hooks
python3 -m venv .venv-d30-hooks && .venv-d30-hooks/bin/pip install dist-hooks/*.whl pytest mypy
.venv-d30-hooks/bin/python -m pytest -rs \
  bindings/python/sc-observability-py/tests_telemetry/test_telemetry_double.py \
  bindings/python/sc-observability-py/tests_telemetry/test_telemetry_gil.py
MYPYPATH=bindings/python/sc-observability-py/python .venv-d30-hooks/bin/mypy --strict --python-version 3.10 \
  bindings/python/sc-observability-py/tests/typing/test_telemetry_typing.py

python3 scripts/ci/validate_public_api.py diff --crate sc-observability-py
cargo public-api --manifest-path bindings/python/sc-observability-py/Cargo.toml -sss --features otlp-telemetry | shasum -a 256
```

The D6 release-config criterion also builds with the plain pyproject
configuration (`maturin build --release --manifest-path bindings/python/sc-observability-py/Cargo.toml`,
no `--features`) and imports `sc_observability.telemetry` from that wheel,
which proves `pyproject.toml` itself enables the feature.
