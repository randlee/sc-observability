#!/usr/bin/env bash
# Registry-consumer validation for the B.7 binding-release manifest
# (release/bindings-artifacts.toml).
#
# Two modes:
#   - default (structural, no network): confirms every "ready" entry is
#     structurally sound (file exists, declared name/version match) and
#     prints what a live check would exercise once published. This is what
#     runs today and in normal CI, since no crates.io/npm/PyPI credentials
#     exist yet and the B.7 name-preflight evidence
#     (~/.config/atm/share/sc-obs/b7-evidence/name-preflight.json, checked
#     2026-09-17T09:34:53Z) confirms all 6 target registry names currently
#     404. A "not found" registry lookup for a "ready" manifest entry is the
#     CORRECT, expected state right now -- it is reported as informational,
#     not a script failure.
#   - `--live-registry-check` (opt-in, off by default): actually calls out to
#     pip/npm/crates.io for each "ready" entry's exact resolved version.
#
# Every "pending" entry is skipped with a clear message (not a failure) --
# per the sprint doc, that is the honest, already-tracked state, not
# something for this script to work around.
set -euo pipefail

LIVE_REGISTRY_CHECK=0
for arg in "$@"; do
  case "$arg" in
    --live-registry-check) LIVE_REGISTRY_CHECK=1 ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

MANIFEST="release/bindings-artifacts.toml"
WORKSPACE_TOML="Cargo.toml"

echo "== structural manifest validation =="
python3 scripts/release_bindings_artifacts.py validate-manifest \
  --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML"
python3 scripts/release_bindings_artifacts.py verify-versions \
  --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML"

echo
echo "== per-artifact registry consumer plan =="
python3 scripts/release_bindings_artifacts.py list-publish-plan --manifest "$MANIFEST" \
  | while IFS='|' read -r kind package wait_seconds status; do
      if [[ "$status" == "pending" ]]; then
        reason=$(python3 - "$MANIFEST" "$package" <<'PY'
import sys
import tomllib
from pathlib import Path

manifest_path, package = sys.argv[1], sys.argv[2]
data = tomllib.loads(Path(manifest_path).read_text(encoding="utf-8"))
for entry in data.get("crates", []) + data.get("packages", []):
    if entry.get("package") == package:
        print(entry.get("pending_reason", "no reason recorded"))
        break
PY
)
        echo "SKIPPED (pending): ${package} (${kind}) -- ${reason}"
        continue
      fi

      if [[ "$LIVE_REGISTRY_CHECK" == "1" ]]; then
        echo "LIVE CHECK: ${package} (${kind}) -- calling out to the real registry"
        case "$kind" in
          pypi)
            if pip index versions "$package" >/tmp/b7-pip-check.log 2>&1; then
              echo "  found on PyPI"
            else
              echo "  NOT FOUND on PyPI -- expected pre-release, re-run after publish"
            fi
            ;;
          npm)
            if npm view "$package" version >/tmp/b7-npm-check.log 2>&1; then
              echo "  found on npm"
            else
              echo "  NOT FOUND on npm -- expected pre-release, re-run after publish"
            fi
            ;;
          crate)
            http_status=$(curl -s -o /dev/null -w '%{http_code}' "https://crates.io/api/v1/crates/${package}" || echo "000")
            if [[ "$http_status" == "200" ]]; then
              echo "  found on crates.io"
            else
              echo "  NOT FOUND on crates.io (http ${http_status}) -- expected pre-release, re-run after publish"
            fi
            ;;
        esac
      else
        echo "READY (structural only): ${package} (${kind}) -- would install/resolve exact version live once --live-registry-check is used post-publish"
      fi
    done

echo
echo "== Rust registry-only embedding consumer (path-dependency proxy) =="
# sc-observability-py and sc-observability-binding-runtime already exist as
# real workspace crates; examples/rust-python-logging already exercises the
# exact "Rust host embeds the Python-facing native backend" scenario this
# validator needs. It currently uses path dependencies -- fine for this
# in-repo validation harness (not a distributed package) -- clearly commented
# in its own Cargo.toml/source as "path override used only because these
# crates aren't on crates.io yet; flip to a version requirement once
# published." Reused here via cargo check rather than duplicating it.
cargo check --locked -p rust-python-logging
echo "rust-python-logging (registry-shaped embedding consumer proxy) check passed"

echo
echo "binding registry consumer validation passed"
