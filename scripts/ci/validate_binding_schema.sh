#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
python_bin="${BINDING_GENERATOR_PYTHON:-$(command -v python3.12 || command -v python || true)}"
if [[ -z "$python_bin" ]]; then
  echo "python3.12 is required for binding generation" >&2
  exit 1
fi
[[ "$($python_bin --version 2>&1)" == "Python 3.12.10" ]] || {
  echo "generator requires Python 3.12.10; found $($python_bin --version 2>&1)" >&2
  exit 1
}
[[ "$(rustc --version)" == 'rustc 1.94.1 '* ]]
case "${1:-}" in
  ""|--schema-only|--bundle-only) ;;
  *) echo "usage: $0 [--schema-only|--bundle-only]" >&2; exit 2 ;;
esac
if [[ ${1:-} != --bundle-only ]]; then
  cargo test --locked --no-fail-fast -p sc-observability-dto
  cargo test --locked --no-fail-fast --manifest-path bindings/schema-generator/Cargo.toml
  cargo run --locked --manifest-path bindings/schema-generator/Cargo.toml --bin sc-observability-schema -- \
    --output bindings/schema/v1.json --errors-output bindings/schema/errors-v1.json --check
  "$python_bin" scripts/ci/validate_binding_generators.py
  BINDING_PYTHON="$python_bin" bash scripts/ci/validate_binding_python_typing.sh
  "$python_bin" -m unittest discover -s scripts/ci/tests -p test_binding_source_bundle.py
fi
if [[ ${1:-} != --schema-only ]]; then
  binding_bundle_dir="$(mktemp -d -t binding-source-bundle.XXXXXX)/artifact"
  trap 'rm -rf "$(dirname "$binding_bundle_dir")"' EXIT
  "$python_bin" scripts/ci/build_binding_source_bundle.py \
    --root-manifest crates/sc-observability-dto/Cargo.toml --output "$binding_bundle_dir"
  "$python_bin" scripts/ci/validate_binding_bundle.py \
    --bundle "$binding_bundle_dir" --evidence target/b3-bundle-evidence.json
fi
if [[ ${1:-} != --bundle-only ]]; then
  "$python_bin" scripts/generate_typescript_bindings.py \
    --schema bindings/schema/v1.json \
    --output-dir bindings/typescript/src/generated --check
fi
echo "binding schema validation passed"
