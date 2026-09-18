#!/usr/bin/env python3
"""Validate the active release train, including exact pins, without rewriting historical evidence."""
import re
import tomllib
from pathlib import Path


def validate(root: Path) -> None:
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    version = workspace["package"]["version"]
    manifests = [(root / member / "Cargo.toml") for member in workspace["members"]]
    packages = {tomllib.loads(path.read_text())["package"]["name"] for path in manifests}
    for path in [root / "Cargo.toml", *manifests]:
        manifest = tomllib.loads(path.read_text())
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
        if match and match[1] == version and f"{version}" not in path.read_text():
            raise ValueError(f"{path}: release notes omit their release version")
    print(f"version literal validation passed (workspace.package.version={version}; exact macro pin verified)")


if __name__ == '__main__':
    validate(Path(__file__).resolve().parents[2])
