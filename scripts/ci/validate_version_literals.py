#!/usr/bin/env python3
"""Validate the active release train, including exact pins, without rewriting historical evidence."""
import json
import re
import tomllib
from pathlib import Path


def validate(root: Path) -> None:
    workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]
    version = workspace["package"]["version"]
    manifests = [(root / member / "Cargo.toml") for member in workspace["members"]]
    packages = {tomllib.loads(path.read_text(encoding="utf-8"))["package"]["name"] for path in manifests}
    release_input_manifests = [
        root / "bindings/tauri/Cargo.toml",
        root / "examples/tauri-logging/src-tauri/Cargo.toml",
    ]
    for path in [root / "Cargo.toml", *manifests, *release_input_manifests]:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        package = manifest.get("package")
        if package and package.get("version") not in (version, {"workspace": True}):
            raise ValueError(f"{path}: package version differs from workspace {version}")
        tables = [manifest, manifest.get("workspace", {})] + list(manifest.get("target", {}).values())
        for table in tables:
            for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
                for alias, spec in table.get(kind, {}).items():
                    spec = {"version": spec} if isinstance(spec, str) else spec
                    name = spec.get("package", alias)
                    if name not in packages or spec.get("workspace"):
                        continue
                    if package and package.get("publish") is False and "path" in spec and "version" not in spec:
                        continue  # CI-only workspace consumers have no distributable dependency pin.
                    if spec.get("version") not in (version, f"={version}"):
                        raise ValueError(f"{path}: {name} version must be {version} or ={version}")
                    if name == "sc-observability-log-macros" and spec.get("version") != f"={version}":
                        raise ValueError(f"{path}: macros must use exact ={version} pin")
    for path in (root / "release").glob("RELEASE-NOTES-*.md"):
        match = re.fullmatch(r"RELEASE-NOTES-(\d+\.\d+\.\d+)\.md", path.name)
        if match and match[1] == version and f"{version}" not in path.read_text(encoding="utf-8"):
            raise ValueError(f"{path}: release notes omit their release version")
    api_policy = json.loads((root / "release/public-api-policy.json").read_text(encoding="utf-8"))
    if api_policy.get("candidate_version") != version or not api_policy.get("crates"):
        raise ValueError("public API policy candidate must match the workspace release version")
    if any(item.get("baseline_version") != "1.4.1" for item in api_policy["crates"].values()):
        raise ValueError("every published API crate must use the actual 1.4.1 baseline")
    workspace_lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    locked_packages = {item["name"]: item["version"] for item in workspace_lock.get("package", [])}
    for crate in api_policy["crates"]:
        if locked_packages.get(crate) != version:
            raise ValueError(f"Cargo.lock: {crate} must resolve to candidate version {version}")
    breaks = tomllib.loads((root / "release/public-api-major-breaks.toml").read_text(encoding="utf-8"))
    if (breaks.get("candidate_version") != version or breaks.get("baseline_version") != "1.4.1"
            or breaks.get("breaks") != []):
        raise ValueError("compatible release requires a matching 1.4.1 baseline and no API break exceptions")
    python_policy = json.loads((root / "release/python-platform-policy.json").read_text(encoding="utf-8"))
    if python_policy.get("candidate_version") != version:
        raise ValueError("Python release policy candidate must match the workspace release version")

    def package_version(path: Path) -> str:
        if path.name == "pyproject.toml":
            return tomllib.loads(path.read_text(encoding="utf-8"))["project"]["version"]
        if path.suffix == ".toml":
            package = tomllib.loads(path.read_text(encoding="utf-8")).get("package", {})
            value = package.get("version")
            return workspace["package"]["version"] if value == {"workspace": True} else value
        return json.loads(path.read_text(encoding="utf-8"))["version"]

    publish_manifest = tomllib.loads((root / "release/publish-artifacts.toml").read_text(encoding="utf-8"))
    release_crates = publish_manifest.get("crates", [])
    if len(release_crates) != 10:
        raise ValueError(f"release artifact manifest must contain all ten Rust packages, found {len(release_crates)}")
    for item in release_crates:
        manifest_path = root / item["cargo_toml"]
        if package_version(manifest_path) != version:
            raise ValueError(f"{manifest_path}: published release package must use {version}")
    for item in publish_manifest.get("python_packages", []):
        manifest_path = root / item["manifest"]
        if package_version(manifest_path) != version:
            raise ValueError(f"{manifest_path}: Python release package must use {version}")
    binding_manifest = tomllib.loads((root / "release/bindings-artifacts.toml").read_text(encoding="utf-8"))
    for item in binding_manifest.get("crates", []):
        manifest_path = root / item["cargo_toml"]
        if package_version(manifest_path) != version:
            raise ValueError(f"{manifest_path}: binding crate must use {version}")
        if item["package"] == "sc-observability-tauri":
            binding_lock = tomllib.loads((root / "bindings/tauri/Cargo.lock").read_text(encoding="utf-8"))
            locked = {entry["name"]: entry["version"] for entry in binding_lock.get("package", [])}
            if locked.get(item["package"]) != version:
                raise ValueError("bindings/tauri/Cargo.lock must match the compatible release version")
    for item in binding_manifest.get("packages", []):
        manifest_path = root / item["manifest_path"]
        if package_version(manifest_path) != version:
            raise ValueError(f"{manifest_path}: binding package must use {version}")
    print(f"version literal validation passed (workspace.package.version={version}; exact macro pin verified)")


if __name__ == '__main__':
    validate(Path(__file__).resolve().parents[2])
