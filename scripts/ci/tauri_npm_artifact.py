"""Create the canonical Tauri npm archive and its producer manifest."""
from __future__ import annotations

import hashlib
import json
from collections.abc import Callable
from pathlib import Path


def produce(source_commit: str, output: Path, package: Path,
            run: Callable[[list[str], Path], object]) -> tuple[Path, Path]:
    """Pack one archive and write the sole producer-manifest schema owner."""
    output.mkdir(parents=True, exist_ok=True)
    run(["npm", "pack", "--pack-destination", str(output)], package)
    archives = list(output.glob("*.tgz"))
    if len(archives) != 1:
        raise RuntimeError("expected exactly one packaged TypeScript archive")
    archive = archives[0]
    manifest = output / "npm-producer.json"
    manifest.write_text(json.dumps({
        "source_commit": source_commit,
        "filename": archive.name,
        "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
    }, indent=2) + "\n", encoding="utf-8")
    return archive, manifest
