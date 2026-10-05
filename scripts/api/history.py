"""Small shared helpers retained by the independent Python and TypeScript checks.

Rust no longer uses a row projection or compiler-metadata helper. Its native
surface is captured and read by ``scripts/ci/stock_public_api.py`` instead.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class ApiError(Exception):
    pass


def git(root: Path, *args: str) -> bytes:
    result = subprocess.run(["git", *args], cwd=root, capture_output=True)
    if result.returncode:
        raise ApiError(result.stderr.decode(errors="replace").strip())
    return result.stdout


def accepted_base(value: str | None = None, root: Path = ROOT) -> str:
    candidate = value or os.environ.get("SC_API_ACCEPTED_BASE")
    if candidate == "0000000000000000000000000000000000000000":
        candidate = None
    if not candidate:
        raise ApiError("accepted API history requires a PR base or prior revision")
    try:
        resolved = git(root, "rev-parse", "--verify", f"{candidate}^{{commit}}").decode().strip()
    except ApiError as error:
        raise ApiError("accepted API history requires a PR base or prior revision") from error
    if resolved == git(root, "rev-parse", "--verify", "HEAD^{commit}").decode().strip():
        raise ApiError("accepted API history base resolves to the candidate commit; supply a prior revision")
    return resolved


def accepted_history(accepted_base_value: str | None = None, root: Path = ROOT) -> dict[str, bytes]:
    """Return retained non-Rust snapshots after proving working-tree immutability."""
    base = accepted_base(accepted_base_value, root)
    accepted: dict[str, bytes] = {}
    for name in git(root, "ls-tree", "-r", "--name-only", base, "--", "schema/api").decode().splitlines():
        if not name.endswith(".json"):
            continue
        previous = git(root, "show", f"{base}:{name}")
        path = root / name
        if not path.is_file() or path.read_bytes() != previous:
            raise ApiError(f"immutable accepted API history changed: {name} (baseline {base})")
        accepted[name] = previous
    return accepted


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--accepted-base")
    parser.add_argument("--check-accepted-history", action="store_true")
    parser.add_argument("--row-differences", action="store_true")
    args = parser.parse_args()
    if args.check_accepted_history == args.row_differences:
        parser.error("choose exactly one operation")
    try:
        if args.check_accepted_history:
            print(json.dumps(sorted(accepted_history(args.accepted_base))))
        else:
            from scripts.api.api_rows import row_differences
            payload = json.load(sys.stdin)
            print(json.dumps(row_differences(payload["reference"], payload["candidate"])))
    except (ApiError, KeyError, TypeError, json.JSONDecodeError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
