#!/usr/bin/env python3
"""Exercise every external Rust consumer against candidate package artifacts."""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]


def candidate_version() -> str:
    """Return the version placed in candidate package manifests."""
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return str(manifest["workspace"]["package"]["version"])


def verify_source(source_sha: str) -> None:
    """Reject a direct suite invocation for a checkout other than its candidate."""
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != source_sha:
        raise ValueError(f"checkout HEAD {head} does not match source-sha {source_sha}")


def record_case(name: str, commands: list[list[str]], output_dir: Path) -> bool:
    """Run every command for one consumer and retain complete command output."""
    log_path = output_dir / f"{name}.log"
    records = []
    passed = True
    with log_path.open("w", encoding="utf-8") as log:
        for command in commands:
            try:
                result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=900)
            except subprocess.TimeoutExpired as error:
                result = subprocess.CompletedProcess(
                    command,
                    124,
                    error.stdout or "",
                    error.stderr or f"command exceeded 900 seconds: {command}\n",
                )
            log.write("$ " + " ".join(command) + "\n")
            log.write(result.stdout)
            log.write(result.stderr)
            records.append({"command": command, "exit_code": result.returncode})
            passed = passed and result.returncode == 0
    (output_dir / f"{name}.json").write_text(
        json.dumps({"case": name, "status": "passed" if passed else "failed", "commands": records}, indent=2)
        + "\n",
        encoding="utf-8",
    )
    return passed


def run(source_sha: str, output_dir: Path) -> int:
    """Run all four independent consumer cases, even after a prior failure."""
    verify_source(source_sha)
    output_dir.mkdir(parents=True, exist_ok=True)
    version = candidate_version()
    python = sys.executable
    cases = {
        "core": [
            [python, "scripts/ci/build_binding_source_bundle.py", "--root-manifest", "crates/sc-observability-dto/Cargo.toml", "--output", str(output_dir / "core-bundle")],
            [python, "scripts/ci/validate_binding_bundle.py", "--bundle", str(output_dir / "core-bundle"), "--evidence", str(output_dir / "core-evidence.json")],
        ],
        "binding-bridge": [
            [python, "scripts/ci/validate_binding_runtime.py", "--consumer-only", "--consumer-evidence", str(output_dir / "binding-runtime-evidence.json")],
        ],
        "runtime-level": [
            [python, "scripts/ci/validate_runtime_level_staged_consumer.py", "--version", version, "--result-file", str(output_dir / "runtime-level-evidence.json")],
        ],
        "log-bridge": [
            [python, "scripts/ci/validate_log_staged_consumer.py", "--version", version, "--result-file", str(output_dir / "log-bridge-evidence.json")],
        ],
    }
    outcomes = {name: record_case(name, commands, output_dir) for name, commands in cases.items()}
    (output_dir / "summary.json").write_text(
        json.dumps({"source_sha": source_sha, "candidate_version": version, "outcomes": outcomes}, indent=2) + "\n",
        encoding="utf-8",
    )
    return 0 if all(outcomes.values()) else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        return run(args.source_sha, args.output_dir.resolve())
    except (OSError, ValueError, subprocess.SubprocessError, tomllib.TOMLDecodeError) as error:
        print(f"rust-consumers: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
