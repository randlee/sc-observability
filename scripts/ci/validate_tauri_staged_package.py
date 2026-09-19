#!/usr/bin/env python3
"""Verify the standalone Tauri crate against locally staged root crates."""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

from prepare_release_staged_packages import inspect_stage

ROOT = Path(__file__).resolve().parents[2]
PATCHED_DEPENDENCIES = {
    "sc-observability-types",
    "sc-observability-dto",
    "sc-observability-binding-runtime",
}


def patched_manifest(manifest: str, names: set[str]) -> str:
    rendered = manifest
    for name in names:
        rendered = re.sub(
            rf'({re.escape(name)}\s*=\s*\{{[^}}]*?)\s*,\s*path\s*=\s*"[^"]+"([^}}]*\}})',
            r'\1\2',
            rendered,
        )
    return rendered


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    stage, source, output = args.stage.resolve(), args.source.resolve(), args.output.resolve()
    evidence = inspect_stage(stage, args.version)
    staged = {item["name"]: item for item in evidence["packages"]}
    missing = PATCHED_DEPENDENCIES - staged
    if missing:
        raise SystemExit(f"root stage lacks Tauri dependencies: {sorted(missing)}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="c2-tauri-package-") as temporary:
        scratch = Path(temporary)
        checkout = scratch / "tauri"
        shutil.copytree(source / "bindings/tauri", checkout)
        (checkout / "Cargo.toml").write_text(
            patched_manifest((checkout / "Cargo.toml").read_text(), PATCHED_DEPENDENCIES),
            encoding="utf-8",
        )
        extracted = scratch / "staged"
        extracted.mkdir()
        for name, item in staged.items():
            archive = stage / item["archive"]
            subprocess.run(["tar", "-xzf", str(archive), "-C", str(extracted)], check=True)
        patch_lines = ["[patch.crates-io]"]
        for name in sorted(PATCHED_DEPENDENCIES):
            patch_lines.append(f'{name} = {{ path = "{(extracted / f"{name}-{args.version}").as_posix()}" }}')
        config = checkout / ".cargo" / "config.toml"
        config.parent.mkdir()
        config.write_text("\n".join(patch_lines) + "\n", encoding="utf-8")
        log_path = output.with_suffix(".log")
        command = ["cargo", "package", "--locked", "--manifest-path", str(checkout / "Cargo.toml"),
                   "--target-dir", str(scratch / "target")]
        with log_path.open("w", encoding="utf-8") as log:
            result = subprocess.run(command, cwd=scratch, stdout=log, stderr=subprocess.STDOUT, text=True)
        if result.returncode:
            raise SystemExit(f"standalone Tauri package verification failed; see {log_path}")
        archives = sorted((scratch / "target" / "package").glob("sc-observability-tauri-*.crate"))
        if len(archives) != 1:
            raise SystemExit("Tauri package verification did not produce exactly one archive")
        shutil.copyfile(archives[0], output)
    report = {
        "status": "passed",
        "package": "sc-observability-tauri",
        "version": args.version,
        "source_commit": evidence["source_commit"],
        "patched_dependencies": sorted(PATCHED_DEPENDENCIES),
        "command": command,
        "archive_sha256": __import__("hashlib").sha256(output.read_bytes()).hexdigest(),
        "log_sha256": __import__("hashlib").sha256(log_path.read_bytes()).hexdigest(),
        "publication": "preflight_only",
    }
    output.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"verified standalone Tauri package against staged root archives: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
