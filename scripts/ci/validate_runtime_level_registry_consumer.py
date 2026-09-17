#!/usr/bin/env python3
"""Run isolated published-baseline and extracted-candidate consumer legs."""

from __future__ import annotations

import argparse
import json
import subprocess
import tarfile
import tempfile
from pathlib import Path

from _runtime_level_common import (
    PLATFORM_ASSERTIONS,
    ROOT,
    qualification,
    release_packages,
    sha256,
    validate_version,
)


METADATA = qualification()
BASELINE_VERSION = METADATA["baseline_version"]
BASELINE_SOURCE_SHA = METADATA["baseline_source_commit"]
EXPECTED_PACKAGES = release_packages()
FIXTURES = ROOT / "scripts" / "ci" / "fixtures" / "runtime-level-consumer"


def run(command: list[str], cwd: Path) -> None:
    subprocess.run(command, cwd=cwd, check=True)


def verified_package(stage: Path, package: dict[str, object], root: Path) -> Path:
    """Verify immutable archive bytes, then consume only a fresh extraction."""
    relative = Path(str(package["archive"]))
    if relative.is_absolute() or ".." in relative.parts:
        raise SystemExit(f"archive path escapes stage: {relative}")
    archive = (stage / relative).resolve()
    if stage not in archive.parents or not archive.is_file() or sha256(archive) != package["archive_sha256"]:
        raise SystemExit(f"archive checksum mismatch: {archive}")
    archive_root = str(package["archive_root"])
    with tarfile.open(archive, "r:gz") as contents:
        names = sorted(member.name for member in contents.getmembers() if member.isfile())
        if names != package["checked_contents"] or any(not name.startswith(f"{archive_root}/") or ".." in Path(name).parts for name in names):
            raise SystemExit(f"archive inventory mismatch: {archive}")
        contents.extractall(root, filter="data")
    extracted = root / archive_root
    declared_relative = Path(str(package["extracted_root"]))
    if declared_relative.is_absolute() or ".." in declared_relative.parts:
        raise SystemExit(f"extracted path escapes stage: {declared_relative}")
    declared = (stage / declared_relative).resolve()
    if stage not in declared.parents or not declared.is_dir():
        raise SystemExit(f"declared extracted content differs from archive: {declared}")
    for name in names:
        relative = name.removeprefix(f"{archive_root}/")
        declared_file, verified_file = declared / relative, extracted / relative
        if not declared_file.is_file() or declared_file.read_bytes() != verified_file.read_bytes():
            raise SystemExit(f"declared extracted content differs from archive: {declared_file}")
    return extracted.resolve()


def cargo_project(root: Path, version: str, patches: str, source: str) -> None:
    (root / "Cargo.toml").write_text(
        "[package]\nname = \"bp2-runtime-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n"
        "\n[workspace]\n\n[dependencies]\n"
        f'sc-observability = "={version}"\nsc-observability-types = "={version}"\nserde_json = "1"\n\n{patches}\n'
    )
    (root / "src").mkdir()
    (root / "src/main.rs").write_text(source)


def baseline_source() -> str:
    return (FIXTURES / "baseline.rs").read_text()


def candidate_source() -> str:
    return (FIXTURES / "candidate.rs").read_text()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--mode", choices=("staged", "live"), default="staged")
    parser.add_argument("--stage", type=Path)
    parser.add_argument("--result-file", type=Path)
    parser.add_argument("--platform", choices=("macos", "ubuntu", "windows"))
    args = parser.parse_args()
    validate_version(args.version)
    if args.mode == "live" and args.stage:
        raise SystemExit("live mode rejects local stage overrides")
    if args.mode == "staged" and not args.stage:
        raise SystemExit("staged mode requires extracted --stage (use the staged-consumer wrapper for auto-staging)")

    provenance: dict[str, object] = {"candidate": {"version": args.version, "mode": args.mode}, "baseline": {"version": BASELINE_VERSION, "source_commit": BASELINE_SOURCE_SHA}, "assertions": list(PLATFORM_ASSERTIONS)}
    patches = ""
    verified = None
    if args.stage:
        manifest = args.stage / "stage-manifest.json"
        evidence = json.loads(manifest.read_text())
        if evidence.get("candidate_version") != args.version or evidence.get("schema_version") != 2:
            raise SystemExit("stage manifest does not match candidate version/schema")
        if tuple(item.get("name") for item in evidence.get("packages", [])) != EXPECTED_PACKAGES or any(item.get("version") != args.version for item in evidence["packages"]):
            raise SystemExit("stage package identities or versions are incomplete")
        verified = tempfile.TemporaryDirectory(prefix="bp2-verified-stage-")
        lines = ["[patch.crates-io]"]
        for package in evidence["packages"]:
            extracted = verified_package(args.stage.resolve(), package, Path(verified.name))
            lines.append(f'{package["name"]} = {{ path = "{extracted.as_posix()}" }}')
        patches = "\n".join(lines)
        provenance["candidate"] = {"version": args.version, "mode": args.mode, "stage_manifest": str(manifest.resolve()), "source_commit": evidence["source_commit"], "archives": {item["name"]: item["archive_sha256"] for item in evidence["packages"]}}

    with tempfile.TemporaryDirectory(prefix="bp2-runtime-baseline-") as temporary:
        baseline = Path(temporary)
        cargo_project(baseline, BASELINE_VERSION, "", baseline_source())
        run(["cargo", "run"], baseline)
        run(["cargo", "run", "--locked"], baseline)
    with tempfile.TemporaryDirectory(prefix="bp2-runtime-candidate-") as temporary:
        candidate = Path(temporary)
        cargo_project(candidate, args.version, patches, candidate_source())
        run(["cargo", "run"], candidate)
        run(["cargo", "run", "--locked"], candidate)
    if verified:
        verified.cleanup()
    if args.result_file:
        args.result_file.parent.mkdir(parents=True, exist_ok=True)
        args.result_file.write_text(json.dumps({"status": "passed", "platform": args.platform, **provenance}, indent=2) + "\n")
    print(json.dumps(provenance, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
