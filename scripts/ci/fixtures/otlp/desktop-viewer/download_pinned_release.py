#!/usr/bin/env python3
"""Download and verify the release selected by config-agent for CI probes."""

from __future__ import annotations

import hashlib
import json
import platform
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path, PurePosixPath

HERE = Path(__file__).resolve().parent
LOCK = HERE / "release.json"


def main() -> int:
    lock = json.loads(LOCK.read_text())
    actual_platform = f"{platform.system().lower()}_{platform.machine().lower()}"
    if actual_platform != lock["platform"]:
        print(f"pinned viewer artifact is {lock['platform']}; this runner is {actual_platform}",
              file=sys.stderr)
        return 2
    output = Path(sys.argv[1] if len(sys.argv) > 1 else "build/otel-desktop-viewer").resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sc-obs-d9-viewer-") as temp:
        archive_path = Path(temp) / "release.tar.gz"
        urllib.request.urlretrieve(lock["artifact_url"], archive_path)
        digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
        if digest != lock["artifact_sha256"]:
            raise SystemExit(f"release SHA-256 mismatch: expected {lock['artifact_sha256']}, got {digest}")
        with tarfile.open(archive_path, "r:gz") as archive:
            candidates = [member for member in archive.getmembers()
                          if PurePosixPath(member.name).name == lock["binary_name"]]
            if len(candidates) != 1 or not candidates[0].isfile():
                raise SystemExit("pinned archive must contain exactly one viewer executable")
            member = candidates[0]
            source = archive.extractfile(member)
            if source is None:
                raise SystemExit("unable to read viewer executable from pinned archive")
            output.write_bytes(source.read())
            output.chmod(0o755)
        binary_digest = hashlib.sha256(output.read_bytes()).hexdigest()
        if binary_digest != lock["binary_sha256"]:
            raise SystemExit(f"binary SHA-256 mismatch: expected {lock['binary_sha256']}, got {binary_digest}")
    print(json.dumps({"binary": str(output), "version": lock["version"],
                      "platform": lock["platform"], "artifact_sha256": digest,
                      "binary_sha256": binary_digest,
                      "source_commit": lock["source_commit"]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
