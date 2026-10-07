#!/usr/bin/env bash
set -euo pipefail

for crate in sc-observability-types sc-observability sc-observe sc-observability-otlp sc-observability-log sc-observability-log-macros sc-observability-dto sc-observability-binding-runtime; do
  cargo rustdoc -p "$crate" -- -Dmissing-docs >/dev/null
done

echo "rustdoc missing-docs validation passed"
