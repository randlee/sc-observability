#!/usr/bin/env python3
"""Download and verify the official Collector pinned by ``collector-release.json``.

Reuses the pinned-viewer downloader's verified download and extraction: the
archive and the extracted binary are both checked against the manifest SHA-256
values before the binary is published at the requested path.  Prints one JSON
receipt line whose ``binary`` is the value for ``TELEMETRY_E2E_COLLECTOR_BINARY``.
"""
from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
MANIFEST = HERE / "collector-release.json"
VIEWER_DOWNLOADER = HERE.parents[1] / "scripts/ci/fixtures/otlp/desktop-viewer/download_pinned_release.py"


def _viewer_downloader():
    spec = importlib.util.spec_from_file_location("pinned_release_downloader", VIEWER_DOWNLOADER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        print("usage: download_collector.py OUTPUT_PATH", file=sys.stderr)
        return 2
    shared = _viewer_downloader()
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    shared._validate_manifest(manifest)
    host = shared._host_platform()
    entry = manifest["platforms"].get(host)
    if entry is None:
        print(f"pinned Collector has no artifact for this runner ({host}); "
              f"available: {', '.join(sorted(manifest['platforms']))}", file=sys.stderr)
        return 2
    output = shared._output_path(args[0])
    with tempfile.TemporaryDirectory(prefix="sc-obs-collector-", dir=output.parent) as temp:
        archive = Path(temp) / f"release.{entry['archive']}"
        archive_digest = shared._download_archive(entry["artifact_url"], archive, entry["artifact_sha256"])
        staged = Path(temp) / entry["binary_name"]
        binary_digest = shared._extract_binary(archive, staged, entry)
        if output.is_symlink():
            raise SystemExit(f"refusing symlink output path: {output}")
        os.replace(staged, output)
    print(json.dumps({"binary": str(output), "version": manifest["version"], "platform": host,
                      "artifact_sha256": archive_digest, "binary_sha256": binary_digest}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
