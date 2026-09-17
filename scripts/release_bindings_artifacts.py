#!/usr/bin/env python3
"""Binding-aware release manifest helper, parallel to scripts/release_artifacts.py.

release_artifacts.py's cmd_validate_manifest hard-requires every crate's
cargo_toml directory to be a root-workspace member. That does not hold for
release/bindings-artifacts.toml: sc-observability-tauri is deliberately its
own standalone Cargo workspace (an empty `[workspace]` table in its own
Cargo.toml, not a member of the root workspace's `members` list), and the
`[[packages]]` entries (pypi/npm) are not Cargo crates at all. This script
implements the equivalent validation for that mixed manifest shape instead of
bending release_artifacts.py to fit it.

No entry in release/bindings-artifacts.toml carries a literal version field.
Versions are always resolved live from each artifact's own manifest
(Cargo.toml's `[package].version`, pyproject.toml's `[project].version`,
package.json's `.version`) at validate/verify time, exactly as
release_artifacts.py already does for the 6-crate manifest.
"""

from __future__ import annotations

import argparse
import json
import tomllib
from pathlib import Path
from typing import Any


class ManifestError(SystemExit):
    """Raised (as SystemExit) for any manifest validation failure."""


def load_raw_manifest(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise ManifestError(f"manifest not found: {path}")
    return tomllib.loads(path.read_text(encoding="utf-8"))


def all_entries(data: dict[str, Any]) -> list[dict[str, Any]]:
    """Return every [[crates]] and [[packages]] entry, tagged with its table."""
    entries: list[dict[str, Any]] = []
    for crate in data.get("crates", []):
        entries.append({**crate, "_table": "crates"})
    for package in data.get("packages", []):
        entries.append({**package, "_table": "packages"})
    if not entries:
        raise ManifestError("manifest must define at least one [[crates]] or [[packages]] entry")
    return entries


def workspace_members(workspace_toml: Path) -> set[str]:
    data = tomllib.loads(workspace_toml.read_text(encoding="utf-8"))
    return set(data.get("workspace", {}).get("members", []))


def workspace_version(workspace_toml: Path) -> str:
    data = tomllib.loads(workspace_toml.read_text(encoding="utf-8"))
    return data["workspace"]["package"]["version"]


def cargo_package_name(cargo_toml: Path) -> str:
    data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    return data["package"]["name"]


def cargo_package_version(cargo_toml: Path, workspace_version_value: str) -> str:
    data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    pkg_version = data["package"]["version"]
    if isinstance(pkg_version, str):
        return pkg_version
    if isinstance(pkg_version, dict) and pkg_version.get("workspace") is True:
        return workspace_version_value
    raise ManifestError(f"{cargo_toml}: unsupported version shape: {pkg_version!r}")


def pyproject_name_and_version(pyproject_toml: Path) -> tuple[str, str]:
    data = tomllib.loads(pyproject_toml.read_text(encoding="utf-8"))
    project = data["project"]
    return project["name"], project["version"]


def package_json_name_and_version(package_json: Path) -> tuple[str, str]:
    data = json.loads(package_json.read_text(encoding="utf-8"))
    return data["name"], data["version"]


def _manifest_kind(manifest_path: Path) -> str:
    if manifest_path.name == "pyproject.toml":
        return "pypi"
    if manifest_path.name == "package.json":
        return "npm"
    raise ManifestError(f"unrecognized package manifest file name: {manifest_path}")


def _require_nonempty_str(entry: dict[str, Any], key: str, artifact: str) -> str:
    value = entry.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ManifestError(f"{artifact}: {key!r} must be a non-empty string")
    return value


def validate(manifest_path: Path, workspace_toml: Path) -> tuple[list[dict[str, Any]], int, int]:
    """Run every structural/ordering/dependency check. Returns (entries, ready_count, pending_count)."""
    data = load_raw_manifest(manifest_path)
    entries = all_entries(data)
    members = workspace_members(workspace_toml)

    # Duplicate artifact names across [[crates]] and [[packages]].
    seen_artifacts: set[str] = set()
    for entry in entries:
        artifact = entry.get("artifact")
        if not artifact:
            raise ManifestError(f"entry missing 'artifact' field: {entry}")
        if artifact in seen_artifacts:
            raise ManifestError(f"duplicate artifact: {artifact}")
        seen_artifacts.add(artifact)

    by_artifact = {entry["artifact"]: entry for entry in entries}

    # Duplicate publish_order values (single shared numeric space, see the
    # manifest's own header comment).
    orders: dict[int, str] = {}
    for entry in entries:
        order = entry.get("publish_order")
        if not isinstance(order, int):
            raise ManifestError(f"{entry['artifact']}: publish_order must be an integer")
        if order in orders:
            raise ManifestError(
                f"duplicate publish_order {order}: {orders[order]} and {entry['artifact']}"
            )
        orders[order] = entry["artifact"]

    # depends_on must name defined artifacts, and publish_order must be
    # strictly greater than every dependency's publish_order.
    for entry in entries:
        artifact = entry["artifact"]
        depends_on = entry.get("depends_on", [])
        if not isinstance(depends_on, list):
            raise ManifestError(f"{artifact}: depends_on must be a list")
        for dependency in depends_on:
            if dependency not in by_artifact:
                raise ManifestError(f"{artifact}: depends_on references undefined artifact: {dependency}")
            dep_order = by_artifact[dependency]["publish_order"]
            if entry["publish_order"] <= dep_order:
                raise ManifestError(
                    f"{artifact}: publish_order ({entry['publish_order']}) must be greater than "
                    f"dependency {dependency}'s publish_order ({dep_order})"
                )

    status_values = {"ready", "pending"}
    ready_count = 0
    pending_count = 0

    for entry in entries:
        artifact = entry["artifact"]
        status = entry.get("status")
        if status not in status_values:
            raise ManifestError(f"{artifact}: status must be one of {sorted(status_values)}, got {status!r}")

        if status == "pending":
            _require_nonempty_str(entry, "pending_reason", artifact)
            pending_count += 1
            continue

        ready_count += 1

        if entry["_table"] == "crates":
            cargo_toml = Path(entry["cargo_toml"])
            if not cargo_toml.exists():
                raise ManifestError(f"{artifact}: status=ready but cargo_toml not found: {cargo_toml}")
            actual_name = cargo_package_name(cargo_toml)
            if actual_name != entry["package"]:
                raise ManifestError(
                    f"{artifact}: package name mismatch: manifest={entry['package']!r} actual={actual_name!r}"
                )
            if entry.get("workspace_member") is True:
                crate_dir = str(cargo_toml.parent).replace("\\", "/")
                if crate_dir not in members:
                    raise ManifestError(
                        f"{artifact}: workspace_member=true but {crate_dir!r} is not in root workspace members"
                    )
        else:  # packages
            manifest_file = Path(entry["manifest_path"])
            if not manifest_file.exists():
                raise ManifestError(f"{artifact}: status=ready but manifest_path not found: {manifest_file}")
            kind = entry.get("kind")
            expected_kind = _manifest_kind(manifest_file)
            if kind != expected_kind:
                raise ManifestError(
                    f"{artifact}: kind {kind!r} does not match manifest file type (expected {expected_kind!r})"
                )
            if kind == "pypi":
                actual_name, _ = pyproject_name_and_version(manifest_file)
            else:
                actual_name, _ = package_json_name_and_version(manifest_file)
            if actual_name != entry["package"]:
                raise ManifestError(
                    f"{artifact}: package name mismatch: manifest={entry['package']!r} actual={actual_name!r}"
                )

    return entries, ready_count, pending_count


def cmd_validate_manifest(args: argparse.Namespace) -> int:
    entries, ready_count, pending_count = validate(Path(args.manifest), Path(args.workspace_toml))
    print(
        f"bindings manifest validation passed "
        f"({ready_count} ready, {pending_count} pending, {len(entries)} total entries)"
    )
    return 0


def cmd_list_publish_plan(args: argparse.Namespace) -> int:
    entries, _, _ = validate(Path(args.manifest), Path(args.workspace_toml))
    entries_sorted = sorted(entries, key=lambda item: (item["publish_order"], item["artifact"]))

    if args.require_ready:
        not_ready = [entry for entry in entries_sorted if entry["status"] != "ready"]
        if not_ready:
            lines = [
                f"  - {entry['artifact']} (status={entry['status']}): "
                f"{entry.get('pending_reason', 'no reason recorded')}"
                for entry in not_ready
            ]
            raise ManifestError(
                "list-publish-plan --require-ready: refusing to publish, "
                f"{len(not_ready)} non-ready artifact(s) present:\n" + "\n".join(lines)
            )

    for entry in entries_sorted:
        kind = "crate" if entry["_table"] == "crates" else entry["kind"]
        wait_seconds = entry.get("wait_after_publish_seconds", 0)
        print(f"{kind}|{entry['package']}|{wait_seconds}|{entry['status']}")
    return 0


def cmd_verify_versions(args: argparse.Namespace) -> int:
    entries, ready_count, pending_count = validate(Path(args.manifest), Path(args.workspace_toml))
    version = workspace_version(Path(args.workspace_toml))

    checked = 0
    for entry in entries:
        if entry["status"] != "ready":
            continue
        artifact = entry["artifact"]
        if entry["_table"] == "crates":
            actual = cargo_package_version(Path(entry["cargo_toml"]), version)
        else:
            manifest_file = Path(entry["manifest_path"])
            if entry["kind"] == "pypi":
                _, actual = pyproject_name_and_version(manifest_file)
            else:
                _, actual = package_json_name_and_version(manifest_file)
        if actual != version:
            raise ManifestError(
                f"{artifact}: version mismatch: expected workspace version {version!r}, got {actual!r}"
            )
        checked += 1

    print(
        f"bindings version verification passed "
        f"(workspace version={version}, {checked} ready entries checked, {pending_count} pending skipped)"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("validate-manifest")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", default="Cargo.toml")
    p.set_defaults(func=cmd_validate_manifest)

    p = sub.add_parser("list-publish-plan")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", default="Cargo.toml")
    p.add_argument("--require-ready", action="store_true")
    p.set_defaults(func=cmd_list_publish_plan)

    p = sub.add_parser("verify-versions")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", required=True)
    p.set_defaults(func=cmd_verify_versions)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
