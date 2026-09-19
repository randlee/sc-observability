"""Exact Phase C manifest proof, composed after the immutable Phase B proof.

Git snapshots are evidence, not authorization: their parsed delta must also
match the narrow metadata/dependency-inheritance contract below. Historical
release validation uses the verified BEFORE bytes, never rewritten B.2 hashes.
"""
from __future__ import annotations

import copy
import hashlib
import json
import posixpath
import re
import subprocess
import tomllib
from pathlib import Path

RECORD = Path("docs/plans/phase-c/manifest-metadata-adaptations.json")
CONSUMER = "crates/sc-observability-log-consumer-check/Cargo.toml"
BRIDGE = "crates/sc-observability-log/Cargo.toml"
METADATA = frozenset({"authors", "homepage", "repository"})
# This is the reviewed metadata scope, not arbitrary paths supplied by evidence.
MANIFESTS = frozenset({
    "Cargo.toml", CONSUMER, BRIDGE,
    "crates/sc-observability-log-macros/Cargo.toml",
    "crates/sc-observability-types/Cargo.toml",
    "crates/sc-observability/Cargo.toml",
    "crates/sc-observe/Cargo.toml",
    "crates/sc-observability-otlp/Cargo.toml",
    "crates/sc-observability-dto/Cargo.toml",
    "crates/sc-observability-binding-runtime/Cargo.toml",
    "bindings/python/sc-observability-py/Cargo.toml",
    "examples/rust-python-logging/Cargo.toml",
})


def blob(content: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(content)).encode() + b"\0" + content).hexdigest()


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(["git", "-C", str(repo), *args], check=True,
                          capture_output=True, timeout=30).stdout


def validate_delta(before: dict[str, bytes], after: dict[str, bytes]) -> None:
    """Reject every semantic change except the declared inherited metadata."""
    old = {p: tomllib.loads(data.decode()) for p, data in before.items()}
    new = {p: tomllib.loads(data.decode()) for p, data in after.items()}
    root = new["Cargo.toml"]["workspace"]
    restored = copy.deepcopy(new)
    old_package = old["Cargo.toml"]["workspace"]["package"]
    root_package = restored["Cargo.toml"]["workspace"]["package"]
    if "authors" in old_package or root_package.pop("authors", None) != ["Rand Lee"]:
        raise ValueError("metadata root change must add only the declared authors")
    restored_bytes = dict(after)
    restored_bytes["Cargo.toml"] = after["Cargo.toml"].replace(b'authors = ["Rand Lee"]\n', b'', 1)
    for path in MANIFESTS - {"Cargo.toml"}:
        previous = old[path]["package"]
        package = restored[path]["package"]
        for field in METADATA:
            if field not in previous and field in package:
                if package.pop(field) != {"workspace": True} or field not in root["package"]:
                    raise ValueError(f"metadata must inherit a declared workspace value: {path}:{field}")
                restored_bytes[path] = restored_bytes[path].replace(
                    f"{field}.workspace = true\n".encode(), b'', 1)
        # Existing metadata values must remain identical, including repository URLs.
    dependency = "sc-observability-log"
    old_dep = old[CONSUMER]["dependencies"][dependency]
    new_dep = restored[CONSUMER]["dependencies"][dependency]
    workspace_dep = root["dependencies"][dependency]
    # This conversion adds the already-existing workspace version to a path-only
    # entry. No feature, package alias, optional/default-feature, registry or git
    # changes are permitted, even if the evidence records self-consistent hashes.
    if (new_dep != {"workspace": True} or set(old_dep) != {"path"}
            or set(workspace_dep) != {"path", "version"}):
        raise ValueError("consumer dependency inheritance changes dependency options/features")
    old_path = (Path(CONSUMER).parent / old_dep["path"]).as_posix()
    if (posixpath.normpath(old_path) != "crates/sc-observability-log"
            or workspace_dep["path"] != "crates/sc-observability-log"):
        raise ValueError("consumer dependency inheritance changes resolved path")
    version = old_package["version"]
    if (workspace_dep["version"] != version or root["package"]["version"] != version
            or old[BRIDGE]["package"]["version"] != {"workspace": True}
            or new[BRIDGE]["package"]["version"] != {"workspace": True}):
        raise ValueError("consumer dependency must use the existing workspace package version")
    restored[CONSUMER]["dependencies"][dependency] = old_dep
    if restored != old:
        raise ValueError("change exceeds Phase C metadata/dependency-inheritance contract")
    restored_bytes[CONSUMER] = restored_bytes[CONSUMER].replace(
        b'sc-observability-log.workspace = true\n',
        b'sc-observability-log = { path = "../sc-observability-log" }\n', 1)
    if restored_bytes != before:
        raise ValueError("change exceeds exact Phase C metadata-only lines")


class MetadataAdaptations:
    def __init__(self, destination: Path, record_path: Path):
        destination = destination.resolve()
        if record_path.is_symlink():
            raise ValueError("metadata evidence may not be a symlink")
        record = json.loads(record_path.read_text())
        if (record.get("schema_version") != 1 or record.get("kind") != "phase_c_workspace_metadata"
                or record.get("historical_provenance") != "docs/plans/phase-b/import-provenance.json"
                or record.get("release_adaptations") != "docs/plans/phase-b/release-adaptations-b-2.json"
                or not str(record.get("reason", "")).strip()
                or set(record.get("manifests", {})) != MANIFESTS):
            raise ValueError("unsupported Phase C metadata evidence scope")
        for key in ("before_commit", "after_commit"):
            if not re.fullmatch(r"[0-9a-f]{40}", record.get(key, "")):
                raise ValueError("metadata evidence requires immutable commits")
        before_commit, after_commit = record["before_commit"], record["after_commit"]
        if git(destination, "rev-parse", after_commit + "^").decode().strip() != before_commit:
            raise ValueError("metadata evidence commits must be adjacent")
        changed = set(git(destination, "diff-tree", "--no-commit-id", "--name-only", "-r",
                          after_commit).decode().splitlines())
        if changed != MANIFESTS:
            raise ValueError("metadata evidence commit changes files outside declared manifests")
        self.before: dict[str, bytes] = {}
        self.after: dict[str, bytes] = {}
        for path, item in record["manifests"].items():
            live = destination / path
            ancestors = [live, *live.parents[:len(Path(path).parts) - 1]]
            if any(p.is_symlink() for p in ancestors) or not live.is_file():
                raise ValueError(f"metadata destination is not a regular confined file: {path}")
            for commit, side, inventory in ((before_commit, "before", self.before),
                                             (after_commit, "after", self.after)):
                content = git(destination, "show", f"{commit}:{path}")
                if blob(content) != item.get(side + "_blob"):
                    raise ValueError(f"metadata {side} blob differs from immutable snapshot: {path}")
                inventory[path] = content
            if live.read_bytes() != self.after[path]:
                raise ValueError(f"metadata destination differs from exact after snapshot: {path}")
        validate_delta(self.before, self.after)

    def apply(self, expected: dict[str, str]) -> dict[str, str]:
        """Advance only imported manifests whose prior proof matches exactly."""
        result = dict(expected)
        for path in (CONSUMER, BRIDGE, "crates/sc-observability-log-macros/Cargo.toml"):
            if expected.get(path) != blob(self.before[path]):
                raise ValueError(f"metadata before blob does not match historical/release proof: {path}")
            result[path] = blob(self.after[path])
        return result
