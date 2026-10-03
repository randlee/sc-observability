#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/public_api_common.sh"
ensure_repo_root
ensure_cargo_subcommand public-api cargo-public-api
status=0
python3 scripts/ci/validate_public_api.py diff || status=$?
record_public_api_diff_cache "$status"
exit "$status"
