#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/public_api_common.sh"

ensure_repo_root
ensure_cargo_subcommand semver-checks cargo-semver-checks

cache_dir="$(public_api_cache_dir)"
mkdir -p "$cache_dir"
report_path="$cache_dir/public-api-semver.txt"

: >"$report_path"

while IFS=$'\t' read -r package manifest_path; do
    [[ -n "$package" ]] || continue
    echo "=== ${package} ===" >>"$report_path"
    cargo semver-checks --manifest-path "$manifest_path" --release-type patch >>"$report_path"
    echo >>"$report_path"
done < <(workspace_public_crates)

cat "$report_path"
echo "public API semver validation passed"
