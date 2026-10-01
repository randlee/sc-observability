# d-30: python telemetry bindings

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Stack / layer: `phase-d-wave5` stack, layer 4 (d-29 → d-33 → d-34 → d-30 → d-31 → d-35 → d-32; wave-5 ruling R12)
- Assignee / model: cobs / terra
- Difficulty: `normal` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `boundary` (consumer)
- Target boundary: BOUNDARY-ScObservabilityPy
- Branch: `sprint/d-30-python-telemetry-bindings`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-30-python-telemetry-bindings`
- PR target: `sprint/d-34-otlp-submission-encoders` (stack order only; no code dependency on d-33 or d-34)
- Blocked by: `obs-d-29-sanity`
- Requirements: PHB-010, PHB-011, PHB-013, PHB-014, PHD-002, PHD-003, PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-010, PHD-013
- ADRs: ADR-002, ADR-009, ADR-011, ADR-012, ADR-014, ADR-015, ADR-016, ADR-020, ADR-021
- Owned paths:
  - bindings/python/sc-observability-py/src/lib.rs
  - bindings/python/sc-observability-py/src/telemetry/**
  - bindings/python/sc-observability-py/pyproject.toml
  - bindings/python/sc-observability-py/python/sc_observability/__init__.py
  - bindings/python/sc-observability-py/python/sc_observability/__init__.pyi
  - bindings/python/sc-observability-py/python/sc_observability/telemetry.py
  - bindings/python/sc-observability-py/python/sc_observability/telemetry.pyi
  - bindings/python/sc-observability-py/tests_telemetry/**
  - bindings/python/sc-observability-py/tests/typing/test_telemetry_typing.py

Ownership notes: `src/lib.rs` changes by two lines only (see Design).
`tests_telemetry/` is new and sits outside `tests/`, so
`validate_python_bindings.sh` does not collect it. Not owned:
`bindings/python/sc-observability-py/Cargo.toml` (d-29) and
`python/sc_observability/generated/**` (d-19, read-only).

## Relations

- `must_follow` d-29: consumes the `otlp-telemetry` Cargo feature,
  `SubmissionEnvelope::from_json`, `SystemIds`, `TelemetryClient`,
  `resolve_config` and the `load_telemetry_file` signature, the flush result
  rules, the error enums and codes, `InMemoryTelemetryClient` and
  `DoubleScript` (`sc-observability-types` feature `test-double`, enabled by
  the py crate's `test-hooks`), the golden fixtures, the committed dependency
  set (no dependency is added here) and the list of GIL-releasing calls.
- Sibling note (prose only; the bead relation is `must_follow` d-29): d-30,
  d-33, d-34 and d-31 can run in parallel, because their owned paths are
  disjoint. d-30 tests run against the d-29 double; the real store (d-33) and
  encoders (d-34) are composed in d-32.
- d-35 (the sanity-history importer) `must_follow` d-30 and consumes the
  test-hooks wheel and the tagged-result API defined here.
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
with matching type stubs and tagged results, and ship it in release wheels.

## Deliverables

1. Add a native `telemetry` module (`src/telemetry/**`, registered from
   `src/lib.rs` under `#[cfg(feature = "otlp-telemetry")]`). It wraps
   `DurableTelemetryClient`. In test-hooks builds only, the private factory
   `Telemetry._with_test_double(script_json: str | None)` opens
   `InMemoryTelemetryClient::with_script` with a `DoubleScript` JSON
   document. Python input is passed to Rust as JSON (`json.dumps`) and goes
   through `SubmissionEnvelope::from_json`; Python does no validation and no
   value conversion (plain attribute maps are converted in Rust, d-29 "Input
   value forms"). [PHD-005, PHD-009, PHB-010]
2. Add the Python API in `sc_observability.telemetry`: the factory
   `Telemetry.open(config=None, *, store_path=None, endpoint=None,
   service_name=None) -> TelemetryResult[Telemetry]`, a context manager, and
   the methods `emit(input: Mapping[str, Any]) -> TelemetryResult[AdmissionReceipt]`,
   `flush(timeout_s=None)`, `flush_submission(submission_id, timeout_s=None)`,
   `shutdown(timeout_s=None)` (each `-> TelemetryResult[FlushReport]`) and
   `status(...) -> TelemetryResult[StoreStatus]`, plus the module function
   `build_envelope(input: Mapping[str, Any]) -> TelemetryResult[str]`
   (canonical JSON, no store). `input` is one d-29 `SubmissionInput`
   document, the same shape as `sc-otel validate --stdin` and every golden
   `input.json` (including `version`). There are no per-signal keyword
   arguments. Log-only, span-only, metric-only, profile-only and combined
   input are accepted. [PHD-005, PHD-006, PHD-009, PHD-010, PHB-010]
3. Add tagged results, following the binding's existing rule (ADR-014,
   wave-5 ruling R18): expected failures return a value and never raise.
   `TelemetryResult[T] = Ok[T] | TelemetryErr`, reusing the package's `Ok`.
   `TelemetryErr.error` is a frozen `TelemetryFailure` whose `kind` is
   `submission`, `admission`, `delivery`, `config` or `internal`, one per
   d-29 `TelemetryClientError` variant (plus `internal` for a contained
   panic). It carries `variant` (the d-29 enum variant in snake case),
   `code`, `message`, `path` and, for `delivery`, the `FlushReport`.
   `flush`, `flush_submission` and `shutdown` return `TelemetryErr` with
   `kind = "delivery"` exactly when the d-29 flush result rules give `Err`.
   `__exit__` calls `shutdown`, retains its result in
   `Telemetry.last_shutdown`, and never raises or suppresses the caller's
   exception, so a delivery failure at context exit stays observable
   (PHB-013 retained shutdown result). Only programmer errors raise
   (`TypeError` for a non-`Mapping` input, `ValueError` for conflicting
   `status` selectors), as in the existing logging API. The receipt, status
   and flush-report types are frozen dataclasses.
   [PHD-007, PHD-008, PHD-009, PHB-010, PHB-011, PHB-013]
4. Release the GIL in `open`, `emit`, `flush`, `flush_submission`,
   `shutdown` and `status`. [PHD-009, PHB-011]
5. Add the stubs `telemetry.pyi` and the `__init__.pyi` exports. Existing
   logging APIs and stubs stay unchanged. [PHD-002, PHD-009]
6. Change the wheel build config: in `pyproject.toml`, set
   `[tool.maturin] features = ["pyo3/abi3-py310", "pyo3/extension-module",
   "otlp-telemetry"]`, so that every release wheel (including the d-10
   windows-arm64 target) ships telemetry, and prove it with the
   `b4a-python-distributions.yml` dispatch (downloadable wheel artifacts).
   [PHD-009, PHB-014]

## This Sprint Does Not Close

- Real collector delivery from the installed wheel, restart recovery against
  the real store, and viewer readback. d-32 owns them.
- Python/CLI equivalence. d-32 owns it.
- Store, drain and backpressure (d-33) and OTLP encoding (d-34).
- The sanity-history importer. d-35 owns it.

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
| `src/telemetry/config.rs` | `ConstructorArgs` (serde form of the `Telemetry.open` arguments) → `ConfigOverrides`. Calls `load_telemetry_file` when `config` is set and `resolve_config(ConfigSources::new(&overrides, file, &env))` with `env = \|k\| std::env::var(k).ok()`. |
| `src/telemetry/dto.rs` | `TelemetryResultDto<T>` with the existing `{"kind": "ok", "value"}` / `{"kind": "error", "error"}` shape, and `TelemetryFailureDto { kind, variant, code, message, path, report }`. Values serialize with the d-29 `Serialize` impls. |
| `python/sc_observability/telemetry.py` | `Telemetry`, `build_envelope`, `TelemetryErr`, `TelemetryFailure`, `TelemetryResult` and the frozen dataclasses (`AdmissionReceipt`, `FlushReport`, `SignalCounts`, `StoreStatus`, `DeliveryStatus`, `LeaseInfo`). It decodes `TelemetryResultDto` JSON into `Ok(...)` or `TelemetryErr(...)`. |
| `telemetry.pyi`, `__init__.py`, `__init__.pyi` | Stubs and re-exports of the names above. |

### Native surface (PyO3)

This follows the existing binding pattern (ADR-015): JSON strings cross the
boundary, every entry point is wrapped in `contained_json` so a panic becomes
an `internal` result, and factories return `(handle | None, result_json)` like
`create_owned`.

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
query keys are parsed with the d-29 `FromStr` impls; a parse failure is a
`submission` result.

### Error mapping

| d-29 error | `TelemetryFailure.kind` | `variant` examples | `report` |
| --- | --- | --- | --- |
| `TelemetryClientError::Submission(_)` | `submission` | `invalid_json`, `validation`, `correlation_conflict` | none |
| `TelemetryClientError::Admission(_)` | `admission` | `disk_bound_exceeded`, `schema_too_new`, `closed` | none |
| `TelemetryClientError::Delivery(_)` | `delivery` | `deadline_exceeded`, `terminal_failure` | the `FlushReport` |
| `TelemetryClientError::Config(_)` | `config` | `config_file`, `missing_field`, `unsupported_combination` | none |
| contained panic | `internal` | `panic` | none |

`code` is `TelemetryClientError::code()`. A `TypeError` from `json.dumps`
returns a `submission` result with `SC_OBSERVABILITY_SUBMIT_INVALID_JSON`.

### GIL release points

Each call in the d-29 GIL list runs its client call inside `py.detach(...)`,
the same call the logging binding uses:

| Python call | Released section |
| --- | --- |
| `Telemetry.open(...)` | `load_telemetry_file`, `resolve_config` and `TelemetryClient::open` (store open and migration check) |
| `emit` | `SubmissionEnvelope::from_json` and `TelemetryClient::emit` (durable commit) |
| `flush` / `flush_submission` / `shutdown` | the client call, up to its deadline |
| `status` | `TelemetryClient::status` |
| `__exit__` | `shutdown`, as above |

`build_envelope` keeps the GIL, because it does no I/O. The JSON input string
is copied into Rust before `detach`, so no Python object is touched while
the GIL is released.

### Python surface

```python
@dataclass(frozen=True)
class TelemetryFailure:
    kind: Literal["submission", "admission", "delivery", "config", "internal"]
    variant: str
    code: str
    message: str
    path: str | None = None
    report: FlushReport | None = None

@dataclass(frozen=True)
class TelemetryErr:
    error: TelemetryFailure
    kind: Literal["error"] = field(default="error", init=False)

TelemetryResult: TypeAlias = Ok[T] | TelemetryErr

class Telemetry:
    @classmethod
    def open(cls, config: str | os.PathLike[str] | None = None, *,
             store_path: str | os.PathLike[str] | None = None,
             endpoint: str | None = None, service_name: str | None = None) -> TelemetryResult[Telemetry]: ...
    last_shutdown: TelemetryResult[FlushReport] | None      # set by __exit__ and shutdown()
    def __enter__(self) -> Telemetry: ...
    def __exit__(self, *exc: object) -> Literal[False]: ...  # shutdown(); retains result; never raises
    def emit(self, input: Mapping[str, Any]) -> TelemetryResult[AdmissionReceipt]: ...
    def flush(self, timeout_s: float | None = None) -> TelemetryResult[FlushReport]: ...
    def flush_submission(self, submission_id: str, timeout_s: float | None = None) -> TelemetryResult[FlushReport]: ...
    def shutdown(self, timeout_s: float | None = None) -> TelemetryResult[FlushReport]: ...
    def status(self, *, submissions: Sequence[str] | None = None,
               record_keys: Sequence[str] | None = None) -> TelemetryResult[StoreStatus]: ...
    @classmethod
    def _with_test_double(cls, script_json: str | None = None, **kwargs: Any) -> TelemetryResult[Telemetry]: ...  # test-hooks only

def build_envelope(input: Mapping[str, Any]) -> TelemetryResult[str]: ...
```

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
  `build_envelope(json.load(input.json))` returns `Ok` whose value equals
  `expected.envelope.json` (with placeholder equality), or returns
  `TelemetryErr` with `kind = "submission"` and the `code` named in
  `expected.error.json`. No call raises.
- [ ] boundary:BOUNDARY-ScObservabilityPy (D2, D3): `tests_telemetry/test_telemetry_double.py`
  runs against the test-hooks wheel (Required validation, "test-hooks wheel"
  block), using `Telemetry._with_test_double`. It covers each single-signal
  and combined `emit` returning `Ok(receipt)`; a duplicate `record_key` →
  `receipt.duplicate`; a scripted `fail` returned by `flush` as
  `TelemetryErr(kind="delivery", variant="terminal_failure")` with the
  report; the same failure retained in `last_shutdown` after a `with` block,
  with no exception raised; a scripted `stall` returned by `flush` as
  `variant="deadline_exceeded"`; a scripted admission rejection returned by
  `emit` as `kind="admission"`; one case per `kind` with its `code`; and the
  two programmer-error raises (`TypeError`, `ValueError`).
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
  passes, including exhaustive `match` over `Ok`/`TelemetryErr`. The
  existing logging tests and stubs pass unchanged.
- [ ] boundary:BOUNDARY-ScObservabilityPy: `bash scripts/ci/validate_python_bindings.sh`
  and the boundary validator pass. That script builds its own test-hooks
  wheel and runs `tests/` only; it is not expected to run the telemetry
  tests, which live in `tests_telemetry/` and run in the two named builds.
  Every build command below passes the complete feature list, so no result
  depends on whether maturin merges CLI `--features` with the pyproject
  `features` list.
- [ ] boundary:ADR-016 (D6, platform matrix): `b4a-python-distributions.yml`
  is dispatched on the d-30 head SHA
  (`gh workflow run b4a-python-distributions.yml --ref sprint/d-30-python-telemetry-bindings -f source_commit=<head> -f development=false`;
  `development=true` only if the d-10 runtime is still incomplete on the
  base, in which case the wheel cells are the evidence). Every `wheel` matrix
  cell, which builds with the `pyproject.toml` features including
  `otlp-telemetry`, is green, its wheel artifact is downloadable, and the
  run URL is recorded in the PR body.
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
