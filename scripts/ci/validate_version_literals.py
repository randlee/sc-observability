#!/usr/bin/env python3
"""Validate the active release train, including exact pins, without rewriting historical evidence."""
import json
import re
import tomllib
from pathlib import Path

HISTORICAL_LOCKS = {
    Path("crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-baseline/Cargo.lock"),
    Path("crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-consumer/Cargo.lock"),
    Path("docs/plans/phase-b/evidence/b3-final/Cargo.lock"),
}

APPROVED_DEFERRED_STANDALONE_PACKAGES = [
    {
        "package": "sc-observability-tauri",
        "baselineVersion": "1.4.1",
        "reason": (
            "The Tauri adapter is a separate workspace and remains pending its "
            "standalone API/publication qualification in "
            "release/bindings-artifacts.toml; it is not one of this candidate's "
            "nine workspace API packages."
        ),
    },
]


def validate_api_package_roster(inventory: dict, publish_artifacts: dict) -> None:
    """Require release inventory coverage for every published Rust crate."""
    candidate = inventory.get("qualificationCandidate")
    if not isinstance(candidate, dict):
        raise ValueError("release inventory must define qualificationCandidate")
    candidate_names = candidate.get("packages")
    deferred = candidate.get("deferredStandalonePackages")
    artifact_crates = publish_artifacts.get("crates")
    if not isinstance(candidate_names, list) or any(
        not isinstance(name, str) or not name for name in candidate_names
    ):
        raise ValueError("qualificationCandidate.packages must be a list of package names")
    if not isinstance(deferred, list) or any(not isinstance(item, dict) for item in deferred):
        raise ValueError(
            "qualificationCandidate.deferredStandalonePackages must be a list of package records"
        )
    deferred_names = [item.get("package") for item in deferred]
    if any(not isinstance(name, str) or not name for name in deferred_names):
        raise ValueError("deferred standalone API package records must name a package")
    if not isinstance(artifact_crates, list) or any(
        not isinstance(item, dict) for item in artifact_crates
    ):
        raise ValueError("publish-artifacts manifest must define a crates list")
    published_names = [item.get("package") for item in artifact_crates]
    if any(not isinstance(name, str) or not name for name in published_names):
        raise ValueError("every publish-artifacts crate must name a package")
    all_names = candidate_names + deferred_names
    if len(candidate_names) != len(set(candidate_names)):
        raise ValueError("qualificationCandidate.packages contains duplicate packages")
    if len(deferred_names) != len(set(deferred_names)):
        raise ValueError("qualificationCandidate.deferredStandalonePackages contains duplicate packages")
    if len(published_names) != len(set(published_names)):
        raise ValueError("publish-artifacts manifest contains duplicate crate packages")
    if len(all_names) != len(set(all_names)):
        raise ValueError("candidate and deferred API package sets overlap")
    if deferred != APPROVED_DEFERRED_STANDALONE_PACKAGES:
        raise ValueError("deferred standalone API package metadata differs from the exact approved exemption")
    if set(all_names) != set(published_names):
        missing = sorted(set(published_names) - set(all_names))
        unknown = sorted(set(all_names) - set(published_names))
        raise ValueError(
            "candidate and deferred API package sets must exactly match publish-artifacts crates "
            f"(omitted={missing}, unknown={unknown})"
        )


def is_candidate_package(name: str) -> bool:
    # This private code generator has its own independent 0.1.0 tool version.
    return name != "sc-observability-schema" and (
        name.startswith("sc-observability") or name == "sc-observe"
    )


def validate_cargo_lock(path: Path, version: str) -> None:
    lock = tomllib.loads(path.read_text(encoding="utf-8"))
    for package in lock.get("package", []):
        name = package.get("name", "")
        if is_candidate_package(name) and package.get("version") != version:
            raise ValueError(f"{path}: {name} must resolve to candidate version {version}")


def validate_package_lock(path: Path, version: str) -> None:
    lock = json.loads(path.read_text(encoding="utf-8"))
    candidates = []
    for key, package in lock.get("packages", {}).items():
        name = package.get("name")
        if not name and key.startswith("node_modules/"):
            name = key.removeprefix("node_modules/")
        if name and name.startswith("@synaptic-canvas/sc-observability"):
            candidates.append((name, package.get("version")))
    if not candidates:
        root_name = lock.get("name")
        if root_name and root_name.startswith("@synaptic-canvas/sc-observability"):
            candidates.append((root_name, lock.get("version")))
    for name, locked_version in candidates:
        if locked_version != version:
            raise ValueError(f"{path}: {name} must resolve to candidate version {version}")


def validate_inventory_candidate(path: Path, version: str) -> None:
    inventory = json.loads(path.read_text(encoding="utf-8"))
    candidate = inventory.get("qualificationCandidate", {}).get("version")
    if candidate != version:
        raise ValueError(f"{path}: qualificationCandidate.version must be {version}, found {candidate!r}")


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
    if api_policy.get("candidate_version") != version:
        raise ValueError("public API policy candidate must match the workspace release version")
    inventory = json.loads((root / "release/release-inventory.json").read_text(encoding="utf-8"))
    publish_manifest = tomllib.loads((root / "release/publish-artifacts.toml").read_text(encoding="utf-8"))
    validate_api_package_roster(inventory, publish_manifest)
    workspace_lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    locked_packages = {item["name"]: item["version"] for item in workspace_lock.get("package", [])}
    for crate in api_policy["crates"]:
        if locked_packages.get(crate) != version:
            raise ValueError(f"Cargo.lock: {crate} must resolve to candidate version {version}")
    python_policy = json.loads((root / "release/python-platform-policy.json").read_text(encoding="utf-8"))
    if python_policy.get("candidate_version") != version:
        raise ValueError("Python release policy candidate must match the workspace release version")

    for lock_path in root.rglob("Cargo.lock"):
        relative = lock_path.relative_to(root)
        if relative in HISTORICAL_LOCKS or any(
            part in {".git", ".beads", "target", "node_modules"} for part in relative.parts
        ):
            continue
        validate_cargo_lock(lock_path, version)
    for lock_path in root.rglob("package-lock.json"):
        relative = lock_path.relative_to(root)
        if any(part in {".git", ".beads", "target", "node_modules"} for part in relative.parts):
            continue
        validate_package_lock(lock_path, version)
    validate_inventory_candidate(root / "release/release-inventory.json", version)

    def package_version(path: Path) -> str:
        if path.name == "pyproject.toml":
            return tomllib.loads(path.read_text(encoding="utf-8"))["project"]["version"]
        if path.suffix == ".toml":
            package = tomllib.loads(path.read_text(encoding="utf-8")).get("package", {})
            value = package.get("version")
            return workspace["package"]["version"] if value == {"workspace": True} else value
        return json.loads(path.read_text(encoding="utf-8"))["version"]

    release_crates = publish_manifest.get("crates", [])
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
