#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
BINDING_PYTHON="${BINDING_PYTHON:-python3}"
if ! "$BINDING_PYTHON" -c 'import sys; assert sys.version_info[:3] == (3,12,10)' 2>/dev/null; then
  BINDING_PYTHON="$(uv python find 3.12.10)"
fi
"$BINDING_PYTHON" -c 'import sys; assert sys.version_info[:3] == (3,12,10), "Python 3.12.10 required"'
[[ "$(rustc --version)" == 'rustc 1.94.1 '* ]]
cargo test --locked -p sc-observability-dto
cargo test --locked --manifest-path bindings/schema-generator/Cargo.toml
cargo run --locked --manifest-path bindings/schema-generator/Cargo.toml --bin sc-observability-schema -- --output bindings/schema/v1.json --errors-output bindings/schema/errors-v1.json --check
"$BINDING_PYTHON" scripts/ci/validate_binding_generators.py
BINDING_PYTHON="$BINDING_PYTHON" bash scripts/ci/validate_binding_python_typing.sh
"$BINDING_PYTHON" -m unittest discover -s scripts/ci/tests -p test_binding_source_bundle.py
"$BINDING_PYTHON" scripts/ci/validate_binding_artifacts.py
BINDING_BUNDLE_DIR="$(mktemp -d -t binding-source-bundle.XXXXXX)/artifact"
trap 'rm -rf "$(dirname "$BINDING_BUNDLE_DIR")"' EXIT
"$BINDING_PYTHON" scripts/ci/build_binding_source_bundle.py --root-manifest crates/sc-observability-dto/Cargo.toml --output "$BINDING_BUNDLE_DIR"
"$BINDING_PYTHON" scripts/ci/validate_binding_bundle.py --bundle "$BINDING_BUNDLE_DIR" --evidence target/b3-bundle-evidence.json
printf '%s\n' 'binding schema validation passed'
