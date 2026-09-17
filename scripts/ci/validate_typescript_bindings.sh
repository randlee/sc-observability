#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
if [[ $# -gt 1 || (${1:-} != '' && ${1:-} != '--platform') ]]; then
  echo 'usage: validate_typescript_bindings.sh [--platform]' >&2
  exit 2
fi
bash scripts/ci/validate_binding_schema.sh
python3 -m unittest discover -s scripts/ci/tests -p test_binding_source_bundle.py
cargo test --locked --manifest-path bindings/tauri/Cargo.toml --features test
python3 scripts/ci/validate_tauri_qualification.py --evidence target/tauri-qualification
if [[ ${1:-} != '--platform' ]]; then
  python3 scripts/ci/validate_tauri_platform_evidence.py target/tauri-platforms
fi
