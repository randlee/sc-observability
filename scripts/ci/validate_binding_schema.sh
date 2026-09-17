#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
python_bin="$(command -v python3.12 || true)"
if [[ -z "$python_bin" ]]; then
  echo "python3.12 is required by bindings/generation-toolchain.toml" >&2
  exit 1
fi
[[ "$($python_bin --version 2>&1)" == "Python 3.12.10" ]] || {
  echo "generator requires Python 3.12.10; found $($python_bin --version 2>&1)" >&2
  exit 1
}
"$python_bin" scripts/generate_typescript_bindings.py \
  --schema bindings/schema/v1.json \
  --output-dir bindings/typescript/src/generated --check
echo "binding schema validation passed"
