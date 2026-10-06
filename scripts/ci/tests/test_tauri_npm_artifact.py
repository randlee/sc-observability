"""Tests for the shared Tauri npm producer artifact helper."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts.ci.tauri_npm_artifact import produce


class TauriNpmArtifactTests(unittest.TestCase):
    def test_produce_packs_once_and_writes_the_canonical_manifest(self) -> None:
        source_commit = "a" * 40
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            calls: list[tuple[list[str], Path]] = []

            def run(command: list[str], cwd: Path) -> None:
                calls.append((command, cwd))
                output.mkdir(parents=True, exist_ok=True)
                (output / "sc-observability-1.0.0.tgz").write_bytes(b"archive")

            archive, manifest = produce(source_commit, output, root / "package", run)
            self.assertEqual([["npm", "pack", "--pack-destination", str(output)]], [call[0] for call in calls])
            self.assertEqual({
                "source_commit": source_commit,
                "filename": archive.name,
                "sha256": hashlib.sha256(b"archive").hexdigest(),
            }, json.loads(manifest.read_text()))


if __name__ == "__main__":
    unittest.main()
