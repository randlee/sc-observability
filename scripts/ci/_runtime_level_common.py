"""Shared B.P2 staged-consumer qualification metadata and validation helpers."""

from __future__ import annotations

import argparse
import hashlib
import re
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
QUALIFICATION = ROOT / "release" / "runtime-level-qualification.toml"
PUBLISH_ARTIFACTS = ROOT / "release" / "publish-artifacts.toml"
# B.1 companion crates copied alongside the staged/published core: none of them
# is a publish-artifacts.toml roster member, so they are workspace members
# that the staged-order check below must tolerate without being treated as
# staged/publishable packages themselves.
UNPUBLISHED_COMPANION_PACKAGES = (
    "sc-observability-log",
    "sc-observability-log-macros",
    "sc-observability-log-consumer-check",
)
# The consumer-check crate is CI-only and must never be staged/published.
PRIVATE_ONLY_COMPANION_PACKAGES = ("sc-observability-log-consumer-check",)
PLATFORMS = ("macos", "ubuntu", "windows")
PLATFORM_ASSERTIONS = (
    "baseline_exact_resolution",
    "candidate_extracted_archive_resolution",
    "threshold_filtering",
    "log_and_query",
    "level_state_and_reset",
    "stale_owner_after_shutdown",
)
VERSION_RE = re.compile(r"\d+\.\d+\.\d+")


def qualification() -> dict[str, str]:
    values = tomllib.loads(QUALIFICATION.read_text())
    required = ("candidate_version", "baseline_version", "baseline_source_commit")
    if set(values) != set(required) or any(not isinstance(values[key], str) for key in required):
        raise SystemExit(f"invalid qualification metadata: {QUALIFICATION}")
    for key in ("candidate_version", "baseline_version"):
        validate_version(values[key])
    return {key: values[key] for key in required}


def release_packages() -> tuple[str, ...]:
    values = tomllib.loads(PUBLISH_ARTIFACTS.read_text())
    crates = values.get("crates")
    if not isinstance(crates, list):
        raise SystemExit(f"missing crates in {PUBLISH_ARTIFACTS}")
    ordered = sorted(crates, key=lambda item: item.get("publish_order", -1))
    packages = tuple(item.get("package") for item in ordered)
    if (not packages or any(not isinstance(package, str) for package in packages)
            or len(set(packages)) != len(packages)
            or [item.get("publish_order") for item in ordered] != list(range(1, len(packages) + 1))):
        raise SystemExit(f"invalid publish artifact roster: {PUBLISH_ARTIFACTS}")
    return packages


def validate_version(value: str) -> None:
    if not VERSION_RE.fullmatch(value):
        raise SystemExit("version must be exact X.Y.Z; placeholders are rejected")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    selector = parser.add_mutually_exclusive_group(required=True)
    selector.add_argument("--candidate-version", action="store_true")
    selector.add_argument("--baseline-version", action="store_true")
    args = parser.parse_args()
    values = qualification()
    print(values["candidate_version" if args.candidate_version else "baseline_version"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
