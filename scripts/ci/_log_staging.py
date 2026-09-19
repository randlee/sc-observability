"""B.2 immutable archive verification shared by staging and isolated consumers."""
from __future__ import annotations

import hashlib
import json
import re
import tarfile
import tomllib
from pathlib import Path, PurePosixPath

PACKAGES = (
    "sc-observability-types", "sc-observability", "sc-observe",
    "sc-observability-otlp", "sc-observability-log-macros", "sc-observability-log",
)
PRIVATE_PACKAGE = "sc-observability-log-consumer-check"
PLATFORMS = ("macos", "ubuntu", "windows")
ASSERTIONS = ("six_exact_archive_dependencies", "enabled_macro", "explicit_flush", "explicit_shutdown", "jsonl_content")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def safe_path(root: Path, relative: str) -> Path:
    parts = PurePosixPath(relative)
    if parts.is_absolute() or ".." in parts.parts or "\\" in relative or ":" in relative:
        raise ValueError(f"unsafe stage path: {relative}")
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"stage path escapes root: {relative}")
    return path


def inspect_archive(
    archive: Path,
    name: str,
    version: str,
    source_commit: str,
    *,
    package_names: tuple[str, ...] = PACKAGES,
    private_package: str | None = PRIVATE_PACKAGE,
    require_macro_pin: bool = True,
) -> dict:
    """Inspect every archive member; reject unsafe, ambient or mismatched content."""
    prefix = f"{name}-{version}/"
    with tarfile.open(archive, "r:gz") as contents:
        files: dict[str, bytes] = {}
        for member in contents.getmembers():
            if (not member.isfile() or not member.name.startswith(prefix)
                    or ".." in PurePosixPath(member.name).parts or "\\" in member.name
                    or member.name in files):
                raise ValueError(f"unsafe/duplicate archive member: {member.name}")
            stream = contents.extractfile(member)
            assert stream is not None
            files[member.name] = stream.read()
        manifest_bytes = files[prefix + "Cargo.toml"]
        manifest = tomllib.loads(manifest_bytes.decode())
        package = manifest["package"]
        if package["name"] != name or package["version"] != version or package.get("publish") is False:
            raise ValueError(f"archive identity/version/publish mismatch: {name}")
        if package.get("license") != "MIT" or not any(key.endswith('/LICENSE') for key in files):
            raise ValueError(f"missing package license: {name}")
        vcs = json.loads(files[prefix + ".cargo_vcs_info.json"])
        if vcs["git"]["sha1"] != source_commit or vcs["git"].get("dirty", False):
            raise ValueError(f"archive source commit/provenance mismatch or dirty source: {name}")
        if "workspace" in manifest or package.get("workspace"):
            raise ValueError(f"archive inherits workspace: {name}")
        tables = [manifest] + list(manifest.get("target", {}).values())
        for table in tables:
            for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
                for dependency, spec in table.get(kind, {}).items():
                    if isinstance(spec, str):
                        spec = {"version": spec}
                    actual = spec.get("package", dependency)
                    if any(key in spec for key in ("path", "git", "workspace")):
                        raise ValueError(f"ambient dependency in {name}: {dependency}")
                    if private_package and actual == private_package:
                        raise ValueError("CI-only package leaked into staged dependencies")
                    if actual in package_names and spec.get("version", "").lstrip("=") != version:
                        raise ValueError(f"first-party version mismatch in {name}: {dependency}")
        if require_macro_pin and name == "sc-observability-log" and manifest["dependencies"]["sc-observability-log-macros"]["version"] != f"={version}":
            raise ValueError("bridge must pin the macros crate exactly")
        return {
            "normalized_manifest": manifest_bytes.decode(),
            "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
            "files": {key: hashlib.sha256(value).hexdigest() for key, value in sorted(files.items())},
        }


def verify_stage(stage: Path, version: str, source_commit: str | None = None) -> dict:
    evidence = json.loads((stage / "stage-manifest.json").read_text())
    if (evidence.get("schema_version") != 1 or evidence.get("candidate_version") != version
            or evidence.get("publication") != "pending_B.7"):
        raise ValueError("stage schema/version/publication mismatch")
    actual_source = evidence.get("source_commit", "")
    if not re.fullmatch(r"[0-9a-f]{40}", actual_source) or (source_commit and actual_source != source_commit):
        raise ValueError("stage source commit mismatch")
    packages = evidence.get("packages", [])
    if tuple(item.get("name") for item in packages) != PACKAGES:
        raise ValueError("stage must contain exactly the six public packages in release order")
    for item in packages:
        if item.get("version") != version:
            raise ValueError("package version mismatch")
        archive = safe_path(stage, item["archive"])
        if sha256(archive) != item["archive_sha256"]:
            raise ValueError(f"archive checksum mismatch: {item['name']}")
        inspected = inspect_archive(archive, item["name"], version, actual_source)
        for key in ("files", "normalized_manifest", "manifest_sha256"):
            if inspected[key] != item[key]:
                raise ValueError(f"archive {key} mismatch: {item['name']}")
    return evidence


def extract_verified(stage: Path, evidence: dict, destination: Path) -> dict[str, Path]:
    paths = {}
    for item in evidence["packages"]:
        archive = safe_path(stage, item["archive"])
        # Verify again immediately before extraction; never consume cached extractions.
        if sha256(archive) != item["archive_sha256"]:
            raise ValueError("archive changed after verification")
        with tarfile.open(archive, "r:gz") as contents:
            contents.extractall(destination, filter="data")
        root = destination / f"{item['name']}-{item['version']}"
        for relative, expected in item["files"].items():
            if sha256(safe_path(destination, relative)) != expected:
                raise ValueError(f"extracted file checksum mismatch: {relative}")
        paths[item["name"]] = root.resolve()
    return paths
