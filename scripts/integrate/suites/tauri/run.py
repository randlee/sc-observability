#!/usr/bin/env python3
"""Run the existing packaged Tauri IPC qualification in a CI-only suite.

The qualification helper owns the actual frontend/host proof.  This runner only
builds the immutable inputs for the selected checkout, establishes the
platform's headless/native prerequisites, and retains the helper's evidence.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
from pathlib import Path, PureWindowsPath


ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "scripts" / "ci"))
from tauri_npm_artifact import produce as produce_npm_artifact

SETUP_TIMEOUT_SECONDS = 10 * 60
QUALIFICATION_TIMEOUT_SECONDS = 30 * 60


def command(arguments: list[str], *, cwd: Path, timeout: int, step: str,
            env: dict[str, str] | None = None) -> None:
    """Run one bounded command, killing its POSIX process group on expiry."""
    executable = "npm.cmd" if os.name == "nt" and arguments[0] == "npm" else arguments[0]
    process = subprocess.Popen(
        [executable, *arguments[1:]], cwd=cwd, env=env, start_new_session=os.name != "nt"
    )
    try:
        exit_code = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        if os.name == "nt":
            process.kill()
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise RuntimeError(f"{step} timed out after {timeout}s") from error
    if exit_code:
        raise subprocess.CalledProcessError(exit_code, arguments)


def platform_name() -> str:
    name = platform.system()
    if name not in {"Darwin", "Linux", "Windows"}:
        raise RuntimeError(f"unsupported Tauri integration platform: {name}")
    return name


def require_ci() -> None:
    if os.environ.get("CI") != "1":
        raise RuntimeError("the Tauri integration suite runs only in CI; it never launches a desktop app locally")


def verify_source(source_sha: str) -> None:
    if not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise RuntimeError("source-sha must be a lowercase 40-hex commit")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != source_sha:
        raise RuntimeError(f"checkout HEAD {head} does not match source-sha {source_sha}")


def prepare_platform(name: str) -> None:
    """Install only the CI runner dependencies required by the existing proof."""
    if name != "Linux":
        return
    command(["sudo", "apt-get", "update"], cwd=ROOT, timeout=SETUP_TIMEOUT_SECONDS,
            step="apt-get update")
    command([
        "sudo", "apt-get", "install", "-y", "libgtk-3-dev", "libwebkit2gtk-4.1-dev",
        "libayatana-appindicator3-dev", "librsvg2-dev", "patchelf", "xvfb", "bubblewrap",
    ], cwd=ROOT, timeout=SETUP_TIMEOUT_SECONDS, step="apt-get install")
    apparmor = Path("/proc/sys/kernel/apparmor_restrict_unprivileged_userns")
    if apparmor.exists():
        command(["sudo", "sysctl", "-w", "kernel.apparmor_restrict_unprivileged_userns=0"], cwd=ROOT,
                timeout=SETUP_TIMEOUT_SECONDS, step="AppArmor user namespace setup")


def prepare_artifacts(source_sha: str, artifacts: Path) -> tuple[Path, Path, Path]:
    """Build the exact npm archive and Rust source bundle consumed by the helper."""
    npm = artifacts / "npm"
    bundle = artifacts / "rust-bundle"
    package = ROOT / "bindings" / "typescript"
    command(["npm", "ci", "--ignore-scripts"], cwd=package,
            timeout=SETUP_TIMEOUT_SECONDS, step="npm ci")
    command(["npm", "run", "build"], cwd=package,
            timeout=SETUP_TIMEOUT_SECONDS, step="npm build")
    archive, manifest = produce_npm_artifact(
        source_sha, npm, package,
        lambda arguments, cwd: command(arguments, cwd=cwd, timeout=SETUP_TIMEOUT_SECONDS, step="npm pack"),
    )
    command([
        sys.executable, "scripts/ci/build_binding_source_bundle.py",
        "--root-manifest", "bindings/tauri/Cargo.toml", "--output", str(bundle),
    ], cwd=ROOT, timeout=SETUP_TIMEOUT_SECONDS, step="Rust source bundle")
    return archive, manifest, bundle


def qualification_command(name: str) -> list[str]:
    base = ["bash", "scripts/ci/validate_typescript_bindings.sh", "--platform"]
    if name == "Linux":
        return ["xvfb-run", "-a", *base]
    if name == "Windows":
        git_bash = str(PureWindowsPath(os.environ.get("ProgramFiles", r"C:\Program Files")) /
                       "Git" / "bin" / "bash.exe")
        return [sys.executable, "scripts/ci/supervise_windows_proof.py", "--", git_bash, *base[1:]]
    return base


def retain_qualification_evidence(evidence: Path, output_dir: Path) -> None:
    """Copy helper diagnostics before a failed qualification escapes the runner."""
    if evidence.is_dir():
        shutil.copytree(evidence, output_dir / "qualification", dirs_exist_ok=True)


def run_qualification(name: str, env: dict[str, str], evidence: Path, output_dir: Path) -> None:
    """Run the helper and retain its finalizer-owned evidence on every outcome.

    The platform wrappers synchronously own their child process cleanup (notably
    the Windows proof supervisor); this outer finally preserves their cleanup
    report and diagnostics even when the helper exits non-zero.
    """
    try:
        command(qualification_command(name), cwd=ROOT, env=env,
                timeout=QUALIFICATION_TIMEOUT_SECONDS, step="Tauri qualification")
    finally:
        retain_qualification_evidence(evidence, output_dir)


def run(source_sha: str, output_dir: Path) -> None:
    require_ci()
    verify_source(source_sha)
    name = platform_name()
    output_dir.mkdir(parents=True, exist_ok=True)
    artifacts = output_dir / "artifacts"
    artifacts.mkdir()
    prepare_platform(name)
    archive, manifest, bundle = prepare_artifacts(source_sha, artifacts)
    env = os.environ.copy()
    env.update({
        "TAURI_NPM_ARCHIVE": str(archive),
        "TAURI_NPM_MANIFEST": str(manifest),
        "TAURI_RUST_BUNDLE": str(bundle),
    })
    evidence = ROOT / "target" / "tauri-qualification"
    shutil.rmtree(evidence, ignore_errors=True)
    run_qualification(name, env, evidence, output_dir)
    if not evidence.is_dir():
        raise RuntimeError("Tauri qualification did not produce retained evidence")
    (output_dir / "runner-report.json").write_text(json.dumps({
        "status": "passed",
        "source_commit": source_sha,
        "platform": name,
        "npm_archive": archive.name,
        "npm_archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
        "rust_bundle": "artifacts/rust-bundle",
        "qualification": "qualification",
    }, indent=2) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        run(args.source_sha, args.output_dir.resolve())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"tauri integration: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
