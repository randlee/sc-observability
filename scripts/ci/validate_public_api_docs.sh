#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/public_api_common.sh"
ensure_repo_root
# Generate actionable diffs; exit 2 and other failures are never approval waivers.
status=0
bash scripts/ci/validate_public_api_diff.sh || status=$?
if [[ "$status" -gt 1 ]]; then exit "$status"; fi
python3 scripts/ci/validate_public_api.py docs
