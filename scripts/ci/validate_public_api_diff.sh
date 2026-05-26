#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/public_api_common.sh"

ensure_repo_root
ensure_cargo_subcommand public-api cargo-public-api

cache_dir="$(public_api_cache_dir)"
mkdir -p "$cache_dir"

report_path="$cache_dir/public-api-diff.txt"
status_path="$cache_dir/public-api-diff.status"
head_rev="$(git rev-parse HEAD)"
base_ref="$(detect_api_base_ref)"
has_diff=0

: >"$report_path"

while IFS=$'\t' read -r package manifest_path; do
    [[ -n "$package" ]] || continue
    echo "=== ${package} ===" >>"$report_path"
    tmp_output="$(mktemp -t public-api-diff-XXXXXX)"
    if cargo public-api --manifest-path "$manifest_path" -sss diff --deny all latest >"$tmp_output" 2>&1; then
        echo "no public API diff against latest published version" >>"$report_path"
        cat "$tmp_output" >>"$report_path"
    else
        status=$?
        if [[ $status -ne 1 ]]; then
            cat "$tmp_output" >&2
            rm -f "$tmp_output"
            exit $status
        fi
        has_diff=1
        cargo public-api --manifest-path "$manifest_path" -sss diff latest >>"$report_path"
    fi
    rm -f "$tmp_output"
    echo >>"$report_path"
done < <(workspace_public_crates)

cat >"$status_path" <<EOF
PUBLIC_API_DIFF_HEAD=${head_rev}
PUBLIC_API_DIFF_BASE_REF=${base_ref}
PUBLIC_API_DIFF_FOUND=${has_diff}
EOF

cat "$report_path"

if [[ $has_diff -eq 1 ]]; then
    echo "public API diff report generated (diffs detected)"
    exit 1
else
    echo "public API diff validation passed"
fi
