#!/usr/bin/env python3
"""Create reproducible, local-only B.P2 `.crate` candidates; never publish."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import re
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

from _runtime_level_common import release_packages, sha256, validate_version


PACKAGES = release_packages()
WORKSPACE_VALUES = {
    "edition": '"2024"', "license": '"MIT"', "rust-version": '"1.94.1"',
    "repository": '"https://github.com/randlee/sc-observability"',
    "homepage": '"https://github.com/randlee/sc-observability"',
}
EXTERNAL_DEPENDENCIES = {
    "serde": '{ version = "1", features = ["derive"] }', "serde_json": '"1"',
    "thiserror": '"2"', "time": '{ version = "0.3", features = ["formatting", "parsing", "serde"] }',
}


def source_tree_sha256(root: Path) -> str:
    """Hash the complete candidate source tree, excluding generated Git/build state."""
    digest = hashlib.sha256()
    for path in sorted(item for item in root.rglob("*") if item.is_file() and ".git" not in item.parts and "target" not in item.parts):
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(sha256(path).encode())
    return digest.hexdigest()


def checked_run(command: list[str], cwd: Path) -> str:
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True)
    if result.returncode != 0:
        raise SystemExit(
            f"command failed ({result.returncode}): {' '.join(command)}\n{result.stderr}"
        )
    return result.stdout


def normalized_manifest(path: Path, version: str) -> bytes:
    """Resolve workspace inheritance and remove local paths for a standalone crate."""
    rendered = path.read_text()
    rendered = re.sub(r"(?m)^version\.workspace = true$", f'version = "{version}"', rendered)
    for field, value in WORKSPACE_VALUES.items():
        rendered = re.sub(rf"(?m)^{re.escape(field)}\.workspace = true$", f"{field} = {value}", rendered)
    for name, value in EXTERNAL_DEPENDENCIES.items():
        rendered = re.sub(rf"(?m)^{re.escape(name)}\.workspace = true$", f"{name} = {value}", rendered)
    for name in PACKAGES:
        rendered = re.sub(rf"(?m)^{re.escape(name)}\.workspace = true$", f'{name} = "={version}"', rendered)
    rendered = re.sub(r"\n\[lints\]\nworkspace = true\n?", "\n", rendered)
    if ".workspace = true" in rendered or "path =" in rendered:
        raise SystemExit(f"normalization left workspace or path inheritance in {path}")
    return rendered.encode()


def normalized_lock(path: Path, version: str) -> bytes:
    """Make the generated package lock agree with this staged release train."""
    names = "|".join(re.escape(name) for name in PACKAGES)
    return re.sub(
        rf'(?ms)(name = "(?:{names})"\nversion = )"\d+\.\d+\.\d+"',
        rf'\g<1>"{version}"',
        path.read_text(),
    ).encode()


def write_archive(package: str, version: str, workspace: Path, file_list: list[str], destination: Path) -> None:
    root = workspace / "crates" / package
    archive_root = f"{package}-{version}"
    with destination.open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode="w", format=tarfile.PAX_FORMAT) as archive:
            for relative in sorted(set(file_list)):
                source = root / relative
                # Cargo's package inventory includes the generated package lock
                # file even though workspace members share the root lockfile.
                if relative == "Cargo.lock" and not source.exists():
                    source = workspace / "Cargo.lock"
                if relative == "Cargo.toml.orig" and not source.exists():
                    source = root / "Cargo.toml"
                if not source.is_file():
                    raise SystemExit(f"cargo package list referenced missing file: {source}")
                if relative == "Cargo.toml":
                    content = normalized_manifest(source, version)
                elif relative == "Cargo.lock":
                    content = normalized_lock(source, version)
                else:
                    content = source.read_bytes()
                info = tarfile.TarInfo(f"{archive_root}/{relative}")
                info.size, info.mode, info.mtime = len(content), source.stat().st_mode & 0o777, 0
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                archive.addfile(info, io.BytesIO(content))


def candidate_workspace_manifest(content: str, version: str) -> str:
    """Advance plain and exact local pins from the actual source workspace version."""
    baseline = tomllib.loads(content)["workspace"]["package"]["version"]
    rendered = re.sub(r'(?m)^version = "' + re.escape(baseline) + r'"$', f'version = "{version}"', content, count=1)
    for prefix in ("", "="):
        rendered = rendered.replace(f'version = "{prefix}{baseline}", path =', f'version = "{prefix}{version}", path =')
    return rendered


def verify_stage(output: Path) -> None:
    evidence = json.loads((output / "stage-manifest.json").read_text())
    if evidence.get("schema_version") != 2:
        raise SystemExit("unsupported stage manifest schema")
    for package in evidence["packages"]:
        archive = output / package["archive"]
        if sha256(archive) != package["archive_sha256"]:
            raise SystemExit(f"changed archive bytes: {archive}")
        with tarfile.open(archive, "r:gz") as contents:
            names = sorted(member.name for member in contents.getmembers() if member.isfile())
            if names != package["checked_contents"]:
                raise SystemExit(f"archive content inventory changed: {archive}")
            manifest = contents.extractfile(f'{package["archive_root"]}/Cargo.toml')
            if manifest is None:
                raise SystemExit(f"archive lacks Cargo.toml: {archive}")
            rendered = manifest.read()
            if b".workspace = true" in rendered or b"path =" in rendered:
                raise SystemExit(f"archive has an unnormalized manifest: {archive}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--verify-existing", action="store_true")
    args = parser.parse_args()
    validate_version(args.version)
    source, output = args.source.resolve(), args.output.resolve()
    if args.verify_existing:
        verify_stage(output)
        print(output / "stage-manifest.json")
        return 0
    if output.exists():
        raise SystemExit(f"refusing to overwrite existing stage: {output}")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=source, text=True).strip():
        raise SystemExit("refusing dirty source provenance; commit or stash source changes first")
    source_sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip()
    workspace = output / "workspace"
    shutil.copytree(source, workspace, ignore=shutil.ignore_patterns(".git", "target", ".DS_Store"))
    root_toml = workspace / "Cargo.toml"
    root_toml.write_text(candidate_workspace_manifest(root_toml.read_text(), args.version))
    archives, extracted = output / "archives", output / "extracted"
    archives.mkdir(parents=True)
    extracted.mkdir()
    packages = []
    for package in PACKAGES:
        file_list = [line for line in checked_run(["cargo", "package", "--list", "-p", package], workspace).splitlines() if line]
        (output / "package-lists").mkdir(exist_ok=True)
        (output / "package-lists" / f"{package}.txt").write_text("\n".join(file_list) + "\n")
        archive = archives / f"{package}-{args.version}.crate"
        write_archive(package, args.version, workspace, file_list, archive)
        with tarfile.open(archive, "r:gz") as contents:
            contents.extractall(extracted, filter="data")
            checked_contents = sorted(member.name for member in contents.getmembers() if member.isfile())
        packages.append({"name": package, "version": args.version, "archive": archive.relative_to(output).as_posix(), "archive_root": f"{package}-{args.version}", "extracted_root": (extracted / f"{package}-{args.version}").relative_to(output).as_posix(), "archive_sha256": sha256(archive), "checked_contents": checked_contents, "package_file_list": f"package-lists/{package}.txt"})
    evidence = {"schema_version": 2, "candidate_version": args.version, "source_commit": source_sha, "source_tree_sha256": source_tree_sha256(source), "publication": "deferred_until_phase_end", "packages": packages}
    (output / "stage-manifest.json").write_text(json.dumps(evidence, indent=2) + "\n")
    verify_stage(output)
    print(output / "stage-manifest.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
