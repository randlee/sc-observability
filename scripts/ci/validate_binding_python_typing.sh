#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
BINDING_PYTHON="${BINDING_PYTHON:-python3}"
if "$BINDING_PYTHON" -c 'import mypy' 2>/dev/null; then
  BINDING_MYPY=("$BINDING_PYTHON" -m mypy)
else
  BINDING_MYPY=(uv run --no-project --python "$BINDING_PYTHON" --with mypy==2.3.1 python -m mypy)
fi
MYPYPATH=bindings/python/sc-observability-py/python "${BINDING_MYPY[@]}" --strict --python-version 3.10 scripts/ci/fixtures/python-generated-minimum.py
BINDING_RUNTIME_PYTHON="${BINDING_RUNTIME_PYTHON:-$(uv python find 3.10)}"
"$BINDING_RUNTIME_PYTHON" - <<'PY'
import importlib.util
import sys
assert sys.version_info[:2] == (3, 10), 'runtime compatibility gate requires Python 3.10'
path='bindings/python/sc-observability-py/python/sc_observability/generated/__init__.py'
spec=importlib.util.spec_from_file_location('generated_minimum',path)
module=importlib.util.module_from_spec(spec)
sys.modules[spec.name]=module
spec.loader.exec_module(module)
value=module.from_wire('OutputAdmissionDto',{'kind':'accepted'})
assert value.kind=='accepted'
query=module.InputLogQuery(schema_version=1)
assert query.limit==100 and query.levels==()
print('PYTHON_310_GENERATED_RUNTIME_AND_STUBS_PASSED')
PY
