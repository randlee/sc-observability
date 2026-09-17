#!/usr/bin/env python3
"""Run a clean Cargo consumer against all six verified B.2 archives, never registry candidates."""
from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path

from _log_staging import ASSERTIONS, PACKAGES, PRIVATE_PACKAGE, extract_verified, sha256, verify_stage

ROOT = Path(__file__).resolve().parents[2]


def validate_resolution(metadata: dict, paths: dict[str, Path], version: str) -> dict:
    resolved = {}
    for package in metadata["packages"]:
        name = package["name"]
        if name == PRIVATE_PACKAGE:
            raise ValueError("private consumer leaked into resolution")
        if name in PACKAGES:
            expected = paths[name] / "Cargo.toml"
            if (package["version"] != version or package.get("source") is not None
                    or Path(package["manifest_path"]).resolve() != expected.resolve()):
                raise ValueError(f"ambient checkout/registry or wrong version resolved for {name}")
            if name in resolved:
                raise ValueError(f"duplicate first-party resolution: {name}")
            resolved[name] = {"version": package["version"], "manifest_path": str(expected), "source": package.get("source")}
    if set(resolved) != set(PACKAGES):
        raise ValueError("not all six candidate packages were consumed")
    return resolved


def run_consumer(stage: Path, version: str, result_path: Path, source_commit: str | None) -> None:
    evidence = verify_stage(stage, version, source_commit)
    result_path.parent.mkdir(parents=True, exist_ok=True)
    host = {"Darwin": "macos", "Linux": "ubuntu", "Windows": "windows"}.get(platform.system())
    if host is None:
        raise ValueError("unsupported consumer host platform")
    with tempfile.TemporaryDirectory(prefix="b2-isolated-consumer-") as temporary:
        isolated = Path(temporary).resolve()
        paths = extract_verified(stage, evidence, isolated / "packages")
        project = isolated / "consumer"
        (project / "src").mkdir(parents=True)
        patches = "\n".join(f'{name} = {{ path = {json.dumps(path.as_posix())} }}' for name, path in paths.items())
        dependencies = "\n".join(f'{name} = "={version}"' for name in PACKAGES)
        (project / "Cargo.toml").write_text(
            '[package]\nname = "b2-staged-consumer"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n'
            f'\n[workspace]\n\n[dependencies]\n{dependencies}\nserde_json = "1"\n\n[patch.crates-io]\n{patches}\n')
        (project / "src/main.rs").write_bytes((ROOT / "scripts/ci/fixtures/log-staged-consumer/main.rs").read_bytes())
        env = {key: value for key, value in os.environ.items() if not key.startswith("CARGO_") and key not in ("RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER")}
        env["CARGO_HOME"] = str(isolated / "cargo-home")
        env["CARGO_TARGET_DIR"] = str(isolated / "target")
        commands = []
        log_path = result_path.with_suffix(".log")
        with log_path.open("w") as log:
            def run(command: list[str], capture: bool = False) -> str:
                commands.append(command)
                log.write("$ " + " ".join(command) + "\n")
                log.flush()
                result = subprocess.run(command, cwd=project, env=env, text=True,
                                        stdout=subprocess.PIPE if capture else log, stderr=log)
                if result.returncode:
                    raise ValueError(f"consumer command failed ({result.returncode}); see {log_path}")
                return result.stdout if capture else ""
            metadata = json.loads(run(["cargo", "metadata", "--format-version", "1"], True))
            resolution = validate_resolution(metadata, paths, version)
            run(["cargo", "run", "--locked", "--", str(isolated / "logs")])
            # The compiled consumer never reads the original workspace or cached extractions.
            for item in evidence["packages"]:
                for relative, expected in item["files"].items():
                    if sha256(isolated / "packages" / relative) != expected:
                        raise ValueError(f"consumed package bytes changed: {relative}")
        output = {"status": "passed", "platform": host, "candidate_version": version,
                  "source_commit": evidence["source_commit"], "stage_manifest_sha256": sha256(stage / "stage-manifest.json"),
                  "archives": {item["name"]: item["archive_sha256"] for item in evidence["packages"]},
                  "staged_locations": {item["name"]: item["archive"] for item in evidence["packages"]},
                  "dependency_resolution": resolution, "assertions": list(ASSERTIONS), "commands": commands,
                  "log_sha256": sha256(log_path), "publication": "pending_B.7"}
        result_path.write_text(json.dumps(output, indent=2) + "\n")
    print(f"B.2 staged consumer passed ({host}); evidence: {result_path}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--stage", type=Path)
    parser.add_argument("--source-commit")
    parser.add_argument("--result-file", type=Path, default=ROOT / "target/b2-consumer/result.json")
    args = parser.parse_args()
    if args.stage:
        run_consumer(args.stage.resolve(), args.version, args.result_file.resolve(), args.source_commit)
    else:
        with tempfile.TemporaryDirectory(prefix="b2-auto-stage-") as temporary:
            stage = Path(temporary) / "stage"
            subprocess.run([sys.executable, str(ROOT / "scripts/ci/prepare_log_staged_packages.py"), "--version", args.version, "--output", str(stage)], check=True)
            run_consumer(stage, args.version, args.result_file.resolve(), args.source_commit)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
