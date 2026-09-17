#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
if [[ $# -gt 1 || (${1:-} != '' && ${1:-} != '--platform') ]]; then
  echo 'usage: validate_typescript_bindings.sh [--platform]' >&2
  exit 2
fi
mkdir -p target/tauri-qualification
exec > >(tee target/tauri-qualification/gate.log) 2>&1
python3 scripts/ci/validate_typescript_control_flow.py
if [[ ${1:-} != '--platform' ]]; then
  bash scripts/ci/validate_binding_schema.sh
fi
python3 -m unittest discover -s scripts/ci/tests -p test_binding_source_bundle.py
cargo test --locked -p sc-observability-binding-runtime
cargo test --locked -p sc-observability-binding-runtime --release
cargo test --locked --manifest-path bindings/tauri/Cargo.toml --features test
python3 scripts/ci/validate_tauri_qualification.py --evidence target/tauri-qualification
if [[ ${1:-} != '--platform' ]]; then
  python3 scripts/ci/validate_tauri_platform_evidence.py target/tauri-platforms
fi
