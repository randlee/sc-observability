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
from pathlib import Path, PurePosixPath

HERE = Path(__file__).resolve().parent
LOCK = HERE / "release.json"
DOWNLOAD_TIMEOUT = 60
CHUNK_SIZE = 1024 * 1024


def _validate_manifest(lock: dict[str, str]) -> None:
    parsed = urllib.parse.urlsplit(lock.get("artifact_url", ""))
    if parsed.scheme != "https" or not parsed.netloc:
        raise ValueError("release artifact URL must use HTTPS")
    for name in ("artifact_sha256", "binary_sha256"):
        digest = lock.get(name, "")
        if not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
            raise ValueError(f"{name} must be a 64-character hexadecimal SHA-256")


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


def _extract_binary(archive_path: Path, output: Path, lock: dict[str, str]) -> str:
    with tarfile.open(archive_path, "r:gz") as archive:
        candidates = [member for member in archive.getmembers()
                      if PurePosixPath(member.name).name == lock["binary_name"]]
        if len(candidates) != 1 or not candidates[0].isfile():
            raise SystemExit("pinned archive must contain exactly one viewer executable")
        source = archive.extractfile(candidates[0])
        if source is None:
            raise SystemExit("unable to read viewer executable from pinned archive")
        digest = hashlib.sha256()
        with source, output.open("wb") as target:
            while True:
                block = source.read(CHUNK_SIZE)
                if not block:
                    break
                digest.update(block)
                target.write(block)
    actual = digest.hexdigest()
    if not hmac.compare_digest(actual, lock["binary_sha256"].lower()):
        raise SystemExit(f"binary SHA-256 mismatch: expected {lock['binary_sha256']}, got {actual}")
    output.chmod(0o755)
    return actual


def _output_path(value: str) -> Path:
    requested = Path(value).expanduser()
    if platform.system().lower() == "windows" and requested.suffix.lower() != ".exe":
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
    actual_platform = f"{platform.system().lower()}_{platform.machine().lower()}"
    if actual_platform != lock["platform"]:
        print(f"pinned viewer artifact is {lock['platform']}; this runner is {actual_platform}",
              file=sys.stderr)
        return 2
    output = _output_path(sys.argv[1] if len(sys.argv) > 1 else "build/otel-desktop-viewer")
    with tempfile.TemporaryDirectory(prefix="sc-obs-d9-viewer-") as temp:
        archive_path = Path(temp) / "release.tar.gz"
        digest = _download_archive(lock["artifact_url"], archive_path, lock["artifact_sha256"])
        staged = Path(temp) / lock["binary_name"]
        binary_digest = _extract_binary(archive_path, staged, lock)
        # _output_path owns normal validation and parent creation. Recheck at
        # publication to reject a symlink swapped in during the download.
        if output.is_symlink():
            raise SystemExit(f"refusing symlink output path: {output}")
        # Publish with an atomic rename so a failed verification never leaves a
        # partial binary at the requested path.
        os.replace(staged, output)
    print(json.dumps({"binary": str(output), "version": lock["version"],
                      "platform": lock["platform"], "artifact_sha256": digest,
                      "binary_sha256": binary_digest,
                      "source_commit": lock["source_commit"]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
