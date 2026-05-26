#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/public_api_common.sh"

ensure_repo_root

approvals_dir="docs/api-approvals"
readme_path="${approvals_dir}/README.md"

if [[ ! -d "$approvals_dir" ]]; then
    echo "missing ${approvals_dir}" >&2
    exit 1
fi

if [[ ! -f "$readme_path" ]]; then
    echo "missing ${readme_path}" >&2
    exit 1
fi

python3 - <<'PY'
from pathlib import Path

approvals_dir = Path("docs/api-approvals")
required_headings = ["## Scope", "## Approval", "## Affected Artifacts"]

for path in sorted(approvals_dir.glob("*.md")):
    if path.name == "README.md":
        continue
    text = path.read_text(encoding="utf-8")
    missing = [heading for heading in required_headings if heading not in text]
    if missing:
        raise SystemExit(
            f"{path} missing required heading(s): {', '.join(missing)}"
        )
PY

cache_dir="$(public_api_cache_dir)"
status_path="$cache_dir/public-api-diff.status"
set +e
bash scripts/ci/validate_public_api_diff.sh >/dev/null
diff_status=$?
set -e

if [[ $diff_status -ne 0 && $diff_status -ne 1 ]]; then
    exit $diff_status
fi

source "$status_path"

approval_files=()
while IFS= read -r path; do
    approval_files+=("$path")
done < <(find "$approvals_dir" -maxdepth 1 -type f -name '*.md' ! -name 'README.md' | sort)

if [[ "${PUBLIC_API_DIFF_FOUND}" == "0" ]]; then
    echo "public API docs validation passed (no API diff detected)"
    exit 0
fi

if [[ ${#approval_files[@]} -eq 0 ]]; then
    echo "public API diff detected but no approval artifact exists under ${approvals_dir}" >&2
    exit 1
fi

python3 - <<'PY' "" "${approval_files[@]}"
import sys
from pathlib import Path

approval_files = sys.argv[2:]

required_checklist = Path("docs/public-api-checklist.md")
normative_docs = [
    Path("docs/requirements.md"),
    Path("docs/architecture.md"),
    Path("docs/api-design.md"),
]

if not required_checklist.exists():
    raise SystemExit(
        "public API diff detected but docs/public-api-checklist.md does not exist"
    )

if not any(p.exists() for p in normative_docs):
    raise SystemExit(
        "public API diff detected but no normative API doc exists in "
        "docs/requirements.md, docs/architecture.md, or docs/api-design.md"
    )

if not approval_files:
    raise SystemExit(
        "public API diff detected but no approval artifact exists under docs/api-approvals/"
    )
PY

echo "public API docs validation passed"
