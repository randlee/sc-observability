#!/usr/bin/env python3
"""Keep B.P2 staged qualification consumers tied to their single authorities."""

from __future__ import annotations

import tomllib
from pathlib import Path

from _runtime_level_common import (
    PRIVATE_ONLY_COMPANION_PACKAGES,
    UNPUBLISHED_COMPANION_MEMBERS,
    PUBLISH_ARTIFACTS,
    QUALIFICATION,
    ROOT,
    qualification,
    release_packages,
)


def validate_workspace_member_roster(members: tuple[str, ...], packages: tuple[str, ...]) -> None:
    """Staged packages must keep their publish order; companions ride along unordered."""
    expected_members = tuple(f"crates/{package}" for package in packages)
    allowed_companions = {
        package if "/" in package else f"crates/{package}"
        for package in UNPUBLISHED_COMPANION_MEMBERS
    }
    overlap = allowed_companions & set(expected_members)
    if overlap:
        raise SystemExit(f"unpublished companion package roster overlaps staged publish roster: {sorted(overlap)}")

    staged_subsequence = tuple(member for member in members if member in set(expected_members))
    if staged_subsequence != expected_members:
        raise SystemExit("workspace member order does not match release/publish-artifacts.toml")

    unrecognized = [
        member for member in members
        if member not in expected_members and member not in allowed_companions
    ]
    if unrecognized:
        raise SystemExit(f"workspace has unrecognized members outside the staged and companion rosters: {unrecognized}")

    private_only = {f"crates/{package}" for package in PRIVATE_ONLY_COMPANION_PACKAGES}
    leaked = private_only & set(expected_members)
    if leaked:
        raise SystemExit(f"private-only companion package must not appear in the staged publish roster: {sorted(leaked)}")


def validate_version_declarations(values: dict[str, str], handoff: Path, baseline_fixture: Path) -> None:
    """Reject drift in the current handoff or frozen released-baseline fixture."""
    candidate, baseline = values["candidate_version"], values["baseline_version"]
    declaration = f"Candidate version: `{candidate}` (the next minor after the current `{baseline}` release)."
    if declaration not in handoff.read_text():
        raise SystemExit("B.P2 handoff current candidate/baseline declaration disagrees with qualification metadata")
    fixture = tomllib.loads(baseline_fixture.read_text())
    dependencies = fixture.get("dependencies", {})
    expected = f"={baseline}"
    if (fixture.get("package", {}).get("version") != baseline
            or dependencies.get("sc-observability") != expected
            or dependencies.get("sc-observability-types") != expected):
        raise SystemExit("frozen released-baseline fixture disagrees with qualification baseline metadata")


def main() -> int:
    values = qualification()
    packages = release_packages()
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
    members = tuple(workspace["workspace"]["members"])
    validate_workspace_member_roster(members, packages)

    artifacts = tomllib.loads(PUBLISH_ARTIFACTS.read_text())["crates"]
    if tuple(item["cargo_toml"] for item in artifacts) != tuple(f"crates/{package}/Cargo.toml" for package in packages):
        raise SystemExit("publish artifact Cargo.toml paths do not match its package roster")

    workflow = (ROOT / ".github" / "workflows" / "bp2-staged-consumer.yml").read_text()
    required = "scripts/ci/_runtime_level_common.py --candidate-version"
    if workflow.count(required) != 2:
        raise SystemExit("B.P2 workflow must derive both staging and consumer versions from qualification metadata")
    for forbidden in (values["candidate_version"], values["baseline_version"]):
        if f"--version {forbidden}" in workflow:
            raise SystemExit("B.P2 workflow hard-codes a release version")

    validate_version_declarations(
        values,
        ROOT / "docs" / "plans" / "phase-b" / "handoff-b-p2.md",
        ROOT / "crates" / "sc-observability" / "tests" / "fixtures" / "bp1-published-v1.2.0-baseline" / "Cargo.toml",
    )

    fixtures = ROOT / "scripts" / "ci" / "fixtures" / "runtime-level-consumer"
    for fixture in (fixtures / "baseline.rs", fixtures / "candidate.rs"):
        if not fixture.is_file() or not fixture.read_text().strip():
            raise SystemExit(f"missing checked-in consumer fixture: {fixture}")
    if "candidate_version" not in QUALIFICATION.read_text() or "baseline_version" not in QUALIFICATION.read_text():
        raise SystemExit("qualification metadata is incomplete")
    print("B.P2 qualification metadata and roster are coherent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
