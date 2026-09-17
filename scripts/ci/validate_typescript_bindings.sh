#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
bash scripts/ci/validate_binding_schema.sh
pushd bindings/typescript >/dev/null
npm ci --ignore-scripts
npm run build
npm test
npm pack --dry-run >/dev/null
popd >/dev/null
cargo check --manifest-path bindings/tauri/Cargo.toml --locked
echo "TypeScript/Tauri binding validation passed"
