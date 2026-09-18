#!/usr/bin/env python3
"""Keep the five CI file-digest consumers on the shared hashing helper."""

from __future__ import annotations

import ast
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONSUMERS = (
    "scripts/ci/validate_binding_generators.py",
    "scripts/ci/build_binding_source_bundle.py",
    "scripts/ci/validate_tauri_platform_evidence.py",
    "scripts/ci/validate_binding_runtime.py",
    "scripts/ci/_python_distribution.py",
)


def main() -> int:
    helper = ROOT / "scripts/ci/_hashing.py"
    if not helper.is_file():
        raise SystemExit("shared SHA-256 helper is missing: scripts/ci/_hashing.py")
    for relative in CONSUMERS:
        path = ROOT / relative
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        local_digest = [
            node for node in ast.walk(tree)
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == "digest"
        ]
        if local_digest:
            raise SystemExit(f"duplicate local digest helper remains: {relative}")
        imported = any(
            isinstance(node, ast.ImportFrom)
            and node.module == "_hashing"
            and any(alias.name == "digest" for alias in node.names)
            for node in tree.body
        )
        if not imported:
            raise SystemExit(f"shared digest import missing: {relative}")
    print(f"SHARED_DIGEST_HELPER_CHECK_PASSED: {len(CONSUMERS)} consumers")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
