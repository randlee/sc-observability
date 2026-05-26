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
    targets = package.get("targets", [])
    if not any("lib" in target.get("kind", []) for target in targets):
        continue
    print(f'{package["name"]}\t{package["manifest_path"]}')
PY
}

detect_api_base_ref() {
    if [[ -n "${SC_OBSERVABILITY_API_BASE_REF:-}" ]]; then
        echo "${SC_OBSERVABILITY_API_BASE_REF}"
        return 0
    fi

    if [[ -n "${GITHUB_BASE_REF:-}" ]] && git rev-parse --verify "origin/${GITHUB_BASE_REF}" >/dev/null 2>&1; then
        echo "origin/${GITHUB_BASE_REF}"
        return 0
    fi

    local candidate
    for candidate in origin/integrate/phase-a origin/develop origin/main; do
        if git rev-parse --verify "$candidate" >/dev/null 2>&1; then
            echo "$candidate"
            return 0
        fi
    done

    echo "HEAD^"
}

public_api_cache_dir() {
    echo "target/public-api"
}
