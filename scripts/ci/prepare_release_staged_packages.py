#!/usr/bin/env python3
"""Stage the complete unpublished root-workspace release closure locally.

This is a non-publishing preflight.  It deliberately separates the root
workspace from the standalone Tauri workspace: Cargo can select several root
packages in one invocation, while Tauri must be verified against extracted
archives through a temporary ``[patch.crates-io]`` consumer.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

from _log_staging import inspect_archive as inspect_shared_archive

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = "https://crates.io/api/v1/crates"
COMMAND_TIMEOUT_SECONDS = 900


def inspect_archive(
    archive: Path,
    name: str,
    version: str,
    source_commit: str,
    package_names: tuple[str, ...] = (),
) -> dict:
    """Compatibility wrapper over the shared archive inspector."""
    return inspect_shared_archive(
        archive,
        name,
        version,
        source_commit,
        package_names=package_names,
        private_package=None,
        require_macro_pin=False,
    )


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(source: Path, *arguments: str) -> str:
    return subprocess.check_output(["git", *arguments], cwd=source, text=True, timeout=COMMAND_TIMEOUT_SECONDS).strip()


def metadata(source: Path) -> dict:
    return json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=source, timeout=COMMAND_TIMEOUT_SECONDS,
    ))


def release_roster(source: Path) -> list[dict]:
    manifest = tomllib.loads((source / "release/publish-artifacts.toml").read_text())
    return sorted(manifest["crates"], key=lambda item: item["publish_order"])


def root_packages(source: Path, cargo_metadata: dict, roster: list[dict]) -> dict[str, dict]:
    members = set(cargo_metadata["workspace_members"])
    packages = {
        item["name"]: item for item in cargo_metadata["packages"]
        if item["id"] in members
    }
    root: dict[str, dict] = {}
    for item in roster:
        manifest = (source / item["cargo_toml"]).resolve()
        package = packages.get(item["package"])
        # The standalone Tauri manifest is intentionally not in root metadata.
        if package is None:
            if item["package"] == "sc-observability-tauri":
                continue
            raise ValueError(f"release crate missing from root metadata: {item['package']}")
        if manifest != Path(package["manifest_path"]).resolve():
            raise ValueError(f"manifest mismatch for {item['package']}")
        if package.get("publish") == []:
            raise ValueError(f"release crate is private: {item['package']}")
        root[item["package"]] = package
    return root


def path_graph(packages: dict[str, dict]) -> dict[str, set[str]]:
    names = set(packages)
    graph = {name: set() for name in names}
    for name, package in packages.items():
        for dependency in package["dependencies"]:
            target = dependency.get("package", dependency["name"])
            if dependency.get("path") and target in names:
                graph[name].add(target)
    return graph


def closure(graph: dict[str, set[str]], roots: set[str]) -> set[str]:
    result = set(roots)
    pending = list(roots)
    while pending:
        current = pending.pop()
        for dependency in graph[current]:
            if dependency not in result:
                result.add(dependency)
                pending.append(dependency)
    return result


def registry_version_exists(name: str, version: str) -> bool:
    request = urllib.request.Request(f"{REGISTRY}/{name}/{version}", headers={"User-Agent": "sc-observability-c2-preflight"})
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            record = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise RuntimeError(f"registry lookup indeterminate for {name}@{version}: HTTP {error.code}") from error
    except (OSError, ValueError) as error:
        raise RuntimeError(f"registry lookup indeterminate for {name}@{version}") from error
    actual = record.get("version", {}).get("num")
    if actual != version or record.get("version", {}).get("crate") != name:
        raise ValueError(f"registry identity mismatch for {name}@{version}")
    return True


def ordered_closure(roster: list[dict], selected: set[str]) -> list[str]:
    order = {item["package"]: item["publish_order"] for item in roster}
    missing = selected - set(order)
    if missing:
        raise ValueError(f"dependency closure contains unlisted release crates: {sorted(missing)}")
    return sorted(selected, key=lambda name: order[name])


def package_command(packages: list[str], target_dir: Path) -> list[str]:
    command = ["cargo", "package", "--locked", "--target-dir", str(target_dir)]
    for package in packages:
        command.extend(["-p", package])
    return command


def inspect_stage(stage: Path, version: str) -> dict:
    evidence = json.loads((stage / "stage-manifest.json").read_text())
    if evidence.get("schema_version") != 1 or evidence.get("candidate_version") != version:
        raise ValueError("stage schema/version mismatch")
    source_commit = evidence.get("source_commit", "")
    if len(source_commit) != 40 or any(character not in "0123456789abcdef" for character in source_commit):
        raise ValueError("stage source commit is not a full SHA")
    for item in evidence.get("packages", []):
        archive = stage / item["archive"]
        if not archive.resolve().is_relative_to(stage.resolve()) or sha256(archive) != item["archive_sha256"]:
            raise ValueError(f"stage archive checksum mismatch: {item.get('name')}")
        inspected = inspect_shared_archive(
            archive,
            item["name"],
            version,
            source_commit,
            package_names=tuple(record["name"] for record in evidence.get("packages", [])),
            private_package=None,
            require_macro_pin=False,
        )
        for key in ("files", "normalized_manifest", "manifest_sha256"):
            if inspected[key] != item[key]:
                raise ValueError(f"stage archive inspection mismatch: {item['name']}")
    return evidence


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=ROOT)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--published-at-target", action="append", default=[], metavar="CRATE")
    parser.add_argument("--verify-existing", action="store_true")
    args = parser.parse_args()
    source, output = args.source.resolve(), args.output.resolve()
    if args.verify_existing:
        inspect_stage(output, args.version)
        print(f"verified immutable root release stage: {output}")
        return 0
    if output.exists():
        raise SystemExit(f"refusing to overwrite immutable stage: {output}")
    if git(source, "status", "--porcelain"):
        raise SystemExit("candidate source must be committed and clean before staging")
    source_commit = git(source, "rev-parse", "HEAD")
    workspace = tomllib.loads((source / "Cargo.toml").read_text())
    if workspace["workspace"]["package"]["version"] != args.version:
        raise SystemExit("requested version differs from the committed workspace train")
    roster = release_roster(source)
    cargo_metadata = metadata(source)
    packages = root_packages(source, cargo_metadata, roster)
    if set(args.published_at_target) - set(packages):
        raise SystemExit("--published-at-target names a non-root release crate")
    published = set(args.published_at_target)
    if not args.published_at_target:
        published = {name for name in packages if registry_version_exists(name, args.version)}
    unpublished = set(packages) - published
    selected = closure(path_graph(packages), unpublished)
    ordered = ordered_closure(roster, selected)
    if not ordered:
        raise SystemExit("all root release crates are already published; no stage is needed")
    output.mkdir(parents=True)
    target_dir = (args.target_dir or output / "cargo-target").resolve()
    command = package_command(ordered, target_dir)
    log_path = output / "cargo-package.log"
    with log_path.open("w", encoding="utf-8") as log:
        result = subprocess.run(
            command, cwd=source, stdout=log, stderr=subprocess.STDOUT, text=True,
            timeout=COMMAND_TIMEOUT_SECONDS,
        )
    if result.returncode:
        raise SystemExit(f"Cargo package verification failed; see {log_path}")
    archives = output / "archives"
    archives.mkdir()
    records = []
    for name in ordered:
        archive = target_dir / "package" / f"{name}-{args.version}.crate"
        if not archive.is_file():
            raise SystemExit(f"cargo package did not produce expected archive: {archive}")
        destination = archives / archive.name
        shutil.copyfile(archive, destination)
        inspected = inspect_shared_archive(
            destination,
            name,
            args.version,
            source_commit,
            package_names=tuple(item["package"] for item in roster),
            private_package=None,
            require_macro_pin=False,
        )
        records.append({"name": name, "version": args.version, "archive": destination.relative_to(output).as_posix(),
                        "archive_sha256": sha256(destination), **inspected})
    evidence = {
        "schema_version": 1,
        "candidate_version": args.version,
        "source_commit": source_commit,
        "published_at_target": sorted(published),
        "unpublished_closure": ordered,
        "package_command": command,
        "package_log_sha256": sha256(log_path),
        "publication": "preflight_only",
        "packages": records,
    }
    (output / "stage-manifest.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    inspect_stage(output, args.version)
    print(f"staged and verified {len(records)} root release packages at {source_commit}: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
