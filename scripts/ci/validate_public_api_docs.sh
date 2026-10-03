#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/public_api_common.sh"
ensure_repo_root
# Reuse only a successful/current diff run; otherwise generate actionable diffs.
if ! public_api_diff_cache_is_current; then
    status=0
    bash scripts/ci/validate_public_api_diff.sh || status=$?
    if [[ "$status" -gt 1 ]]; then exit "$status"; fi
fi
python3 scripts/ci/validate_public_api.py docs
