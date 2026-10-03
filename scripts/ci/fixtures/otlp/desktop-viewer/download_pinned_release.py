#!/usr/bin/env python3
"""Download and verify the viewer release pinned by the checked-in manifest."""

from __future__ import annotations

import hashlib
import hmac
import json
import os
import platform
import re
import sys
import tarfile
import tempfile
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path, PurePosixPath

HERE = Path(__file__).resolve().parent
LOCK = HERE / "release.json"
DOWNLOAD_TIMEOUT = 60
CHUNK_SIZE = 1024 * 1024


ARCHIVE_KINDS = ("tar.gz", "zip")
MACHINES = {"x86_64": "amd64", "amd64": "amd64", "aarch64": "arm64", "arm64": "arm64"}


def _host_platform() -> str:
    machine = platform.machine().lower()
    return f"{platform.system().lower()}_{MACHINES.get(machine, machine)}"


def _validate_entry(name: str, entry: dict[str, str]) -> None:
    parsed = urllib.parse.urlsplit(entry.get("artifact_url", ""))
    if parsed.scheme != "https" or not parsed.netloc:
        raise ValueError(f"{name}: release artifact URL must use HTTPS")
    for field in ("artifact_sha256", "binary_sha256"):
        if not re.fullmatch(r"[0-9a-fA-F]{64}", entry.get(field, "")):
            raise ValueError(f"{name}: {field} must be a 64-character hexadecimal SHA-256")
    if entry.get("archive") not in ARCHIVE_KINDS:
        raise ValueError(f"{name}: archive must be one of {ARCHIVE_KINDS}")
    binary_name = entry.get("binary_name", "")
    if not binary_name or PurePosixPath(binary_name).name != binary_name:
        raise ValueError(f"{name}: binary_name must be a bare file name")


def _validate_manifest(lock: dict) -> None:
    platforms = lock.get("platforms")
    if not isinstance(platforms, dict) or not platforms:
        raise ValueError("manifest must define platforms")
    for name, entry in platforms.items():
        _validate_entry(name, entry)


def _download_archive(url: str, path: Path, expected_digest: str) -> str:
    digest = hashlib.sha256()
    request = urllib.request.Request(url, headers={"User-Agent": "sc-observability-viewer-setup"})
    with urllib.request.urlopen(request, timeout=DOWNLOAD_TIMEOUT) as response, path.open("wb") as target:
        if urllib.parse.urlsplit(response.geturl()).scheme != "https":
            raise ValueError("release download redirected away from HTTPS")
        while True:
            block = response.read(CHUNK_SIZE)
            if not block:
                break
            digest.update(block)
            target.write(block)
    actual = digest.hexdigest()
    if not hmac.compare_digest(actual, expected_digest.lower()):
        raise SystemExit(f"release SHA-256 mismatch: expected {expected_digest}, got {actual}")
    return actual


def _copy_verified(source: object, output: Path, expected: str) -> str:
    digest = hashlib.sha256()
    with source, output.open("wb") as target:  # type: ignore[attr-defined]
        while True:
            block = source.read(CHUNK_SIZE)  # type: ignore[attr-defined]
            if not block:
                break
            digest.update(block)
            target.write(block)
    actual = digest.hexdigest()
    if not hmac.compare_digest(actual, expected.lower()):
        raise SystemExit(f"binary SHA-256 mismatch: expected {expected}, got {actual}")
    return actual


def _extract_binary(archive_path: Path, output: Path, entry: dict[str, str]) -> str:
    if entry["archive"] == "zip":
        with zipfile.ZipFile(archive_path) as archive:
            candidates = [info for info in archive.infolist() if not info.is_dir()
                          and PurePosixPath(info.filename).name == entry["binary_name"]]
            if len(candidates) != 1:
                raise SystemExit("pinned archive must contain exactly one viewer executable")
            actual = _copy_verified(archive.open(candidates[0]), output, entry["binary_sha256"])
    else:
        with tarfile.open(archive_path, "r:gz") as archive:
            candidates = [member for member in archive.getmembers()
                          if PurePosixPath(member.name).name == entry["binary_name"]]
            if len(candidates) != 1 or not candidates[0].isfile():
                raise SystemExit("pinned archive must contain exactly one viewer executable")
            source = archive.extractfile(candidates[0])
            if source is None:
                raise SystemExit("unable to read viewer executable from pinned archive")
            actual = _copy_verified(source, output, entry["binary_sha256"])
    output.chmod(0o755)
    return actual


def _is_windows() -> bool:
    return platform.system().lower() == "windows"


def _output_path(value: str) -> Path:
    requested = Path(value).expanduser()
    if _is_windows() and requested.suffix.lower() != ".exe":
        requested = requested.with_name(requested.name + ".exe")
    if requested.is_symlink():
        raise SystemExit(f"refusing symlink output path: {requested}")
    requested.parent.mkdir(parents=True, exist_ok=True)
    output = requested.parent.resolve() / requested.name
    if output.is_symlink():
        raise SystemExit(f"refusing symlink output path: {output}")
    return output


def main() -> int:
    lock = json.loads(LOCK.read_text())
    _validate_manifest(lock)
    host = _host_platform()
    entry = lock["platforms"].get(host)
    if entry is None:
        print(f"pinned viewer has no artifact for this runner ({host}); "
              f"available: {', '.join(sorted(lock['platforms']))}", file=sys.stderr)
        return 2
    output = _output_path(sys.argv[1] if len(sys.argv) > 1 else "build/otel-desktop-viewer")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sc-obs-d9-viewer-", dir=output.parent) as temp:
        archive_path = Path(temp) / f"release.{entry['archive']}"
        digest = _download_archive(entry["artifact_url"], archive_path, entry["artifact_sha256"])
        staged = Path(temp) / entry["binary_name"]
        binary_digest = _extract_binary(archive_path, staged, entry)
        if output.is_symlink():
            raise SystemExit(f"refusing symlink output path: {output}")
        # Publish with an atomic rename so a failed verification never leaves a
        # partial binary at the requested path.
        os.replace(staged, output)
    print(json.dumps({"binary": str(output), "version": lock["version"],
                      "platform": host, "artifact_sha256": digest,
                      "binary_sha256": binary_digest,
                      "source_commit": lock["source_commit"]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
