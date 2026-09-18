#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
import tomllib
from pathlib import Path

data = tomllib.loads(Path("release/publish-artifacts.toml").read_text(encoding="utf-8"))
orders = [item["publish_order"] for item in data["crates"]]
if len(orders) != len(set(orders)):
    raise SystemExit("duplicate publish_order values detected")
if orders != sorted(orders):
    raise SystemExit("publish_order values must already be sorted in the manifest")
print("publish order validation passed")
PY

# release/bindings-artifacts.toml (B.7 binding-release manifest) covers a
# different, mixed shape (crates.io crates plus pypi/npm packages, some
# "pending"), so its order/dependency validation lives in
# scripts/release_bindings_artifacts.py's validate-manifest subcommand
# rather than being duplicated here. Both checks' output is visible in the
# same CI log.
python3 scripts/release_bindings_artifacts.py validate-manifest \
  --manifest release/bindings-artifacts.toml \
  --workspace-toml Cargo.toml
echo "bindings publish order/dependency validation passed"
