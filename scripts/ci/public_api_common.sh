#!/usr/bin/env bash
set -euo pipefail

ensure_repo_root() {
    if [[ ! -f Cargo.toml || ! -d scripts/ci || ! -d docs ]]; then
        echo "ERROR: run from repo root" >&2
        exit 1
    fi
}

ensure_cargo_subcommand() {
    local subcommand="$1"
    local crate="$2"
    if ! cargo "$subcommand" --version >/dev/null 2>&1; then
        cargo +stable install "$crate" --locked
    fi
}

workspace_public_crates() {
    python3 - <<'PY'
import json
import subprocess

metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        text=True,
    )
)

for package in metadata["packages"]:
    publish = package.get("publish", None)
    if publish is False or publish == []:
        continue
    targets = package.get("targets", [])
    if not any(
        "lib" in target.get("kind", []) or "proc-macro" in target.get("kind", [])
        for target in targets
    ):
        continue
    print(f'{package["name"]}\t{package["manifest_path"]}')
PY
}

detect_api_base_ref() {
    local head_rev
    head_rev="$(git rev-parse HEAD)"

    if [[ -n "${SC_OBSERVABILITY_API_BASE_REF:-}" ]]; then
        echo "${SC_OBSERVABILITY_API_BASE_REF}"
        return 0
    fi

    if [[ -n "${GITHUB_BASE_REF:-}" ]] && git rev-parse --verify "origin/${GITHUB_BASE_REF}" >/dev/null 2>&1; then
        echo "origin/${GITHUB_BASE_REF}"
        return 0
    fi

    local candidate
    for candidate in origin/develop origin/main origin/integrate/phase-a; do
        if git rev-parse --verify "$candidate" >/dev/null 2>&1; then
            if [[ "$(git rev-parse "$candidate")" == "$head_rev" ]]; then
                continue
            fi
            echo "$candidate"
            return 0
        fi
    done

    echo "HEAD^"
}

public_api_cache_dir() {
    echo "target/public-api"
}

public_api_diff_cache_is_current() {
    local cache current_revision recorded_revision recorded_status
    cache="$(public_api_cache_dir)"
    current_revision="$(git rev-parse HEAD)"

    [[ -f "$cache/public-api-diff.json" ]] || return 1
    [[ -f "$cache/public-api-diff.revision" ]] || return 1
    [[ -f "$cache/public-api-diff.exit" ]] || return 1

    recorded_revision="$(<"$cache/public-api-diff.revision")"
    recorded_status="$(<"$cache/public-api-diff.exit")"
    [[ "$recorded_revision" == "$current_revision" ]] || return 1
    [[ "$recorded_status" == "0" || "$recorded_status" == "1" ]]
}

record_public_api_diff_cache() {
    local status="$1" cache
    cache="$(public_api_cache_dir)"
    mkdir -p "$cache"
    git rev-parse HEAD > "$cache/public-api-diff.revision"
    printf '%s\n' "$status" > "$cache/public-api-diff.exit"
}
