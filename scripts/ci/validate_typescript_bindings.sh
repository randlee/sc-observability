#!/usr/bin/env bash
set -euo pipefail
export PYTHONUTF8=1
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
python3 -m unittest discover -s scripts/ci/tests -p test_tauri_platform_evidence.py
python3 -m unittest discover -s scripts/ci/tests -p test_tauri_network_scope.py
python3 -m unittest discover -s scripts/ci/tests -p test_windows_proof_supervisor.py
python3 scripts/ci/validate_binding_runtime.py --platform-only --evidence target/tauri-qualification/native-runtime
cargo test --locked --manifest-path bindings/tauri/Cargo.toml --features test
qualification_args=(--evidence target/tauri-qualification)
if [[ -n ${TAURI_NPM_ARCHIVE:-} ]]; then
  qualification_args+=(--npm-archive "$TAURI_NPM_ARCHIVE" --npm-manifest "$TAURI_NPM_MANIFEST")
fi
if [[ -n ${TAURI_RUST_BUNDLE:-} ]]; then
  qualification_args+=(--bundle "$TAURI_RUST_BUNDLE")
fi
python3 scripts/ci/validate_tauri_qualification.py "${qualification_args[@]}"
if [[ ${1:-} != '--platform' ]]; then
  python3 scripts/ci/validate_tauri_platform_evidence.py target/tauri-platforms --source "$(git rev-parse HEAD)"
fi
