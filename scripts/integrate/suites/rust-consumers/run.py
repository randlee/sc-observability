#!/usr/bin/env python3
"""Exercise every external Rust consumer against candidate package artifacts."""
from __future__ import annotations

import argparse
import json
import platform
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
WINDOWS_SUPERVISOR = "scripts/ci/supervise_windows_proof.py"
WINDOWS_SANDBOX_VALIDATORS = {
    "scripts/ci/validate_binding_bundle.py",
    "scripts/ci/validate_binding_runtime.py",
}
SANDBOX_TOOLCHAIN = "1.94.1"


def candidate_version() -> str:
    """Return the version placed in candidate package manifests."""
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return str(manifest["workspace"]["package"]["version"])


def verify_source(source_sha: str) -> None:
    """Reject a direct suite invocation for a checkout other than its candidate."""
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != source_sha:
        raise ValueError(f"checkout HEAD {head} does not match source-sha {source_sha}")
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)
    if dirty:
        raise ValueError("checkout has uncommitted changes; candidate source must be clean")


def windows_supervised_command(command: list[str], evidence_dir: Path) -> list[str]:
    """Run Sandbox-dependent validator commands under the existing Windows supervisor."""
    if platform.system() != "Windows" or len(command) < 2 or command[1] not in WINDOWS_SANDBOX_VALIDATORS:
        return command
    if command[1].endswith("validate_binding_runtime.py") and "--consumer-only" not in command:
        return command
    return [sys.executable, WINDOWS_SUPERVISOR, "--evidence", str(evidence_dir), "--", *command]


def prepare_sandbox_prerequisites() -> None:
    """Provision the pinned toolchain and reject missing native isolation support."""
    subprocess.run(["rustup", "toolchain", "install", SANDBOX_TOOLCHAIN, "--profile", "minimal"], check=True)
    system = platform.system()
    if system == "Linux" and not shutil.which("bwrap"):
        raise RuntimeError("rust-consumers requires bwrap on Linux for Sandbox isolation")
    if system == "Windows" and not shutil.which("powershell"):
        raise RuntimeError("rust-consumers requires Windows identity prerequisites (powershell)")


def output_text(value: str | bytes | None) -> str:
    """Normalize timeout output before writing the text-mode case log."""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value or ""


def checked_origin(evidence_path: Path, source_sha: str) -> dict[str, object]:
    """Reject consumer evidence that does not prove staged first-party origins."""
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    if evidence.get("status") != "passed":
        raise ValueError(f"{evidence_path.name}: evidence status is not passed")
    if evidence.get("source_commit") != source_sha:
        raise ValueError(f"{evidence_path.name}: source commit does not match candidate")
    archives = evidence.get("archives")
    if not isinstance(archives, dict) or not archives:
        raise ValueError(f"{evidence_path.name}: expected staged archives are missing")
    provenance = evidence.get("dependency_provenance")
    if provenance is None:
        provenance = evidence.get("dependency_resolution")
    if isinstance(provenance, list):
        resolved = {item.get("name"): item for item in provenance if isinstance(item, dict)}
    elif isinstance(provenance, dict):
        resolved = provenance
    else:
        raise ValueError(f"{evidence_path.name}: dependency origin evidence is missing")
    missing = sorted(set(archives) - set(resolved))
    if missing:
        raise ValueError(f"{evidence_path.name}: expected archives are not resolved: {', '.join(missing)}")
    outside = sorted(name for name in archives if not isinstance(resolved[name], dict) or resolved[name].get("source") is not None)
    if outside:
        raise ValueError(f"{evidence_path.name}: first-party dependencies resolved outside artifacts: {', '.join(outside)}")
    return {
        "status": "passed",
        "source_commit": source_sha,
        "archives": dict(archives),
        "first_party_resolution": {name: resolved[name] for name in sorted(archives)},
    }


def record_case(name: str, commands: list[list[str]], output_dir: Path, evidence_path: Path, source_sha: str) -> tuple[bool, dict[str, object]]:
    """Run every command for one consumer and retain complete command output."""
    log_path = output_dir / f"{name}.log"
    records = []
    passed = True
    with log_path.open("w", encoding="utf-8") as log:
        for command in commands:
            executed_command = windows_supervised_command(command, output_dir / f"{name}-windows-supervisor")
            try:
                result = subprocess.run(executed_command, cwd=ROOT, text=True, capture_output=True, timeout=900)
            except subprocess.TimeoutExpired as error:
                result = subprocess.CompletedProcess(
                    executed_command,
                    124,
                    output_text(error.stdout),
                    output_text(error.stderr) or f"command exceeded 900 seconds: {command}\n",
                )
            log.write("$ " + " ".join(executed_command) + "\n")
            log.write(result.stdout)
            log.write(result.stderr)
            records.append({"command": executed_command, "exit_code": result.returncode})
            passed = passed and result.returncode == 0
    origin: dict[str, object]
    try:
        origin = checked_origin(evidence_path, source_sha)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        passed = False
        origin = {"status": "failed", "error": str(error)}
    (output_dir / f"{name}.json").write_text(
        json.dumps({"case": name, "status": "passed" if passed else "failed", "commands": records, "origin": origin}, indent=2)
        + "\n",
        encoding="utf-8",
    )
    return passed, origin


def run(source_sha: str, output_dir: Path) -> int:
    """Run all four independent consumer cases, even after a prior failure."""
    verify_source(source_sha)
    prepare_sandbox_prerequisites()
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
            [python, "scripts/ci/build_binding_source_bundle.py", "--root-manifest", "crates/sc-observability/Cargo.toml", "--output", str(output_dir / "runtime-level-bundle")],
            [python, "scripts/ci/validate_binding_bundle.py", "--bundle", str(output_dir / "runtime-level-bundle"), "--evidence", str(output_dir / "runtime-level-evidence.json"), "--consumer-source", "scripts/ci/fixtures/runtime-level-consumer/candidate.rs"],
        ],
        "log-bridge": [
            [python, "scripts/ci/validate_log_staged_consumer.py", "--version", version, "--result-file", str(output_dir / "log-bridge-evidence.json")],
        ],
    }
    evidence_paths = {
        "core": output_dir / "core-evidence.json",
        "binding-bridge": output_dir / "binding-runtime-evidence.json",
        "runtime-level": output_dir / "runtime-level-evidence.json",
        "log-bridge": output_dir / "log-bridge-evidence.json",
    }
    results = {
        name: record_case(name, commands, output_dir, evidence_paths[name], source_sha)
        for name, commands in cases.items()
    }
    outcomes = {name: result[0] for name, result in results.items()}
    origins = {name: result[1] for name, result in results.items()}
    (output_dir / "summary.json").write_text(
        json.dumps({"source_sha": source_sha, "candidate_version": version, "outcomes": outcomes, "origins": origins}, indent=2) + "\n",
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
