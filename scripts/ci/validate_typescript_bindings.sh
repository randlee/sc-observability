#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
bash scripts/ci/validate_binding_schema.sh
package_dir="$(mktemp -d -t sc-observability-package.XXXXXX)"
pushd bindings/typescript >/dev/null
npm ci --ignore-scripts
npm run build
npm test
npm pack --pack-destination "$package_dir" >/dev/null
popd >/dev/null

package_file="$(find "$package_dir" -maxdepth 1 -type f -name '*.tgz' -print -quit)"
test -n "$package_file"
consumer_dir="$package_dir/consumer"
mkdir "$consumer_dir"
pushd "$consumer_dir" >/dev/null
npm init --yes >/dev/null
npm install --ignore-scripts --no-save "$package_file" >/dev/null
node <<'NODE'
const { createClient, encodeEvent } = require("@sc-observability/client");
const event = encodeEvent({ level: "info", target: "package-consumer", action: "smoke", fields: { count: 3n } });
if (event.kind !== "ok" || event.value.fields.count.value !== "3") throw new Error("package event encoding failed");
const operations = [];
const created = createClient({
  request(operation, request) {
    operations.push([operation, request]);
    return Promise.resolve({ kind: "ok", value: { schema_version: 1, kind: "ok", value: { kind: "accepted", operation: "try_log" } } });
  },
});
if (created.kind !== "ok") throw new Error("package client construction failed");
created.value.tryLog(event.value).then((result) => {
  if (result.kind !== "ok" || operations.length !== 1 || operations[0][0] !== "try_log") process.exit(1);
  console.log("INSTALLED_PACKAGE_CONSUMER_PASSED");
}).catch(() => process.exit(1));
NODE
popd >/dev/null
cargo check --manifest-path bindings/tauri/Cargo.toml --locked
cargo check --manifest-path examples/tauri-logging/src-tauri/Cargo.toml --locked
echo "TypeScript/Tauri binding validation passed"
