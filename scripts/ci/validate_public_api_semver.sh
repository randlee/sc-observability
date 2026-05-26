#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/public_api_common.sh"

ensure_repo_root
ensure_cargo_subcommand semver-checks cargo-semver-checks

cache_dir="$(public_api_cache_dir)"
mkdir -p "$cache_dir"
report_path="$cache_dir/public-api-semver.txt"

: >"$report_path"
semver_failed=0

while IFS=$'\t' read -r package manifest_path; do
    [[ -n "$package" ]] || continue
    echo "=== ${package} ===" >>"$report_path"
    if ! cargo semver-checks --manifest-path "$manifest_path" --release-type patch >>"$report_path"; then
        semver_failed=1
    fi
    echo >>"$report_path"
done < <(workspace_public_crates)

cat "$report_path"

if [[ "$semver_failed" == "0" ]]; then
    echo "public API semver validation passed"
    exit 0
fi

if find docs/api-approvals -maxdepth 1 -type f -name '*.md' ! -name 'README.md' | grep -q .; then
    echo "public API semver validation passed with approved breaking changes"
    exit 0
fi

echo "public API semver validation failed" >&2
exit 1
