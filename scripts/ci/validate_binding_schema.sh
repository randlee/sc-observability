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
[[ "$(rustc --version)" == 'rustc 1.94.1 '* ]]
cargo test --locked -p sc-observability-dto
cargo test --locked --manifest-path bindings/schema-generator/Cargo.toml
cargo run --locked --manifest-path bindings/schema-generator/Cargo.toml --bin sc-observability-schema -- \
  --output bindings/schema/v1.json --errors-output bindings/schema/errors-v1.json --check
"$python_bin" scripts/ci/validate_binding_generators.py
"$python_bin" -m unittest discover -s scripts/ci/tests -p test_binding_source_bundle.py
"$python_bin" scripts/ci/validate_binding_artifacts.py
binding_bundle_dir="$(mktemp -d -t binding-source-bundle.XXXXXX)/artifact"
trap 'rm -rf "$(dirname "$binding_bundle_dir")"' EXIT
"$python_bin" scripts/ci/build_binding_source_bundle.py \
  --root-manifest crates/sc-observability-dto/Cargo.toml --output "$binding_bundle_dir"
"$python_bin" scripts/ci/validate_binding_bundle.py \
  --bundle "$binding_bundle_dir" --evidence target/b3-bundle-evidence.json
"$python_bin" scripts/generate_typescript_bindings.py \
  --schema bindings/schema/v1.json \
  --output-dir bindings/typescript/src/generated --check
echo "binding schema validation passed"
