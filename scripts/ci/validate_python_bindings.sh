#!/usr/bin/env bash
# Validates the installed B.4/B.5/B.6 Python runtime on CPython 3.10.
set -euo pipefail

cd "$(dirname "$0")/../.."

# Distribution jobs share the source validator entry point but consume immutable
# artifacts. The source validation path below remains the complete B.4 gate.
if [[ "${1:-}" == --distribution ]]; then
  shift
  exec "${B4A_PYTHON:-python3}" scripts/ci/validate_python_distribution.py "$@"
fi

B4_PYTHON="${B4_PYTHON:-$(uv python find 3.10)}"
"$B4_PYTHON" -c 'import sys; assert sys.version_info[:2] == (3, 10), "B.4 requires CPython 3.10"'
B4_GENERATOR_PYTHON="${B4_GENERATOR_PYTHON:-$(uv python find 3.12.10)}"
"$B4_GENERATOR_PYTHON" -c 'import sys; assert sys.version_info[:3] == (3, 12, 10), "schema generation requires Python 3.12.10"'

# The shared validator owns the selected interpreter, PYTHONHOME, and loader
# path updates for every Rust-hosted binding command below.
B4_EMBEDDED_ENV=()
while IFS= read -r assignment; do
  B4_EMBEDDED_ENV+=("$assignment")
done < <("$B4_PYTHON" scripts/ci/python_binding_validator.py embedded-environment "$B4_PYTHON")

B4_TEMP_DIR="$(mktemp -d -t sc-observability-b4.XXXXXX)"
trap 'rm -rf "$B4_TEMP_DIR"' EXIT

cargo clippy --locked -p sc-observability-py --all-targets -- -D warnings
env "${B4_EMBEDDED_ENV[@]}" cargo test --locked -p sc-observability-py
"$B4_GENERATOR_PYTHON" scripts/generate_python_bindings.py \
  --schema bindings/schema/v1.json \
  --output-dir bindings/python/sc-observability-py/python/sc_observability/generated \
  --check

MYPYPATH=bindings/python/sc-observability-py/python \
  uv run --no-project --python "$B4_PYTHON" --with mypy==2.3.1 python -m mypy \
  --strict --python-version 3.10 \
  bindings/python/sc-observability-py/tests/typing/test_result_narrowing.py \
  bindings/python/sc-observability-py/tests/typing/test_async_narrowing.py
uv run --no-project --python "$B4_PYTHON" --with pytest==9.1.1 python -m pytest \
  bindings/python/sc-observability-py/tests/test_facade.py

uvx --from "maturin==$("$B4_PYTHON" -c 'from pathlib import Path; from scripts.ci.python_binding_validator import maturin_version; print(maturin_version(Path(".")))')" maturin build --locked \
  --manifest-path bindings/python/sc-observability-py/Cargo.toml \
  --features test-hooks \
  --interpreter "$B4_PYTHON" \
  --out "$B4_TEMP_DIR/wheels"
uv venv --python "$B4_PYTHON" "$B4_TEMP_DIR/venv"
uv pip install --python "$B4_TEMP_DIR/venv/bin/python" pytest==9.1.1 mypy==2.3.1 "$B4_TEMP_DIR"/wheels/*.whl
cp -R bindings/python/sc-observability-py/tests "$B4_TEMP_DIR/tests"
"$B4_PYTHON" scripts/ci/stage_python_conformance.py --source . --tests "$B4_TEMP_DIR/tests"
cp -R bindings/python/sc-observability-py/examples "$B4_TEMP_DIR/examples"
SC_OBSERVABILITY_RUNTIME_TEST=1 PYTHONASYNCIODEBUG=1 PYTHONWARNINGS=error \
"$B4_TEMP_DIR/venv/bin/python" -I -X dev -W error -m pytest \
  "$B4_TEMP_DIR/tests" -ra
"$B4_TEMP_DIR/venv/bin/python" -I -m mypy --strict --python-version 3.10 \
  "$B4_TEMP_DIR/tests/typing/test_result_narrowing.py" "$B4_TEMP_DIR/examples/standard_logging.py" \
  "$B4_TEMP_DIR/tests/typing/test_async_narrowing.py" "$B4_TEMP_DIR/examples/async_logging.py"
"$B4_TEMP_DIR/venv/bin/python" -I "$B4_TEMP_DIR/examples/standard_logging.py"
"$B4_TEMP_DIR/venv/bin/python" -I -X dev -W error "$B4_TEMP_DIR/examples/async_logging.py"
if [[ -z "${SC_OBSERVABILITY_ATTACHED_PACKAGE+x}" ]]; then
  SC_OBSERVABILITY_ATTACHED_PACKAGE="$("$B4_TEMP_DIR/venv/bin/python" -I -c 'import sc_observability; print(sc_observability.__file__)')"
fi
export SC_OBSERVABILITY_ATTACHED_PACKAGE
env "${B4_EMBEDDED_ENV[@]}" PYTHONASYNCIODEBUG=1 PYTHONWARNINGS=error \
  cargo run --locked -p rust-python-logging

if rg -n '\braise\b' bindings/python/sc-observability-py/python/sc_observability/{__init__,logging,context,async_logging}.py; then
  exit 1
fi
if rg -n 'panic!|\.unwrap\(' bindings/python/sc-observability-py/src; then
  exit 1
fi

printf '%s\n' 'python binding runtime validation passed'
