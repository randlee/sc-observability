"""Build the B.4a workflow matrix and policy overlay without inline Python."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from python_arm64 import apply_windows_arm64_overlay


ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = ROOT / "release/python-platform-policy.json"


def read_policy(policy_path: Path = POLICY_PATH) -> dict:
    """Read, validate, and apply the temporary Windows ARM64 policy overlay."""
    policy = json.loads(policy_path.read_text(encoding="utf-8"))
    ids = {platform["id"] for platform in policy["platforms"]}
    assert len(ids) == len(policy["platforms"])
    apply_windows_arm64_overlay(policy)
    return policy


def matrix(output_path: Path, policy_path: Path = POLICY_PATH) -> None:
    """Write the workflow matrix using the legacy JSON representation exactly."""
    policy = read_policy(policy_path)
    cells = [
        {**platform, "python": version}
        for platform in policy["platforms"]
        for version in platform.get("interpreters", policy["interpreters"])
    ]
    assert len(policy["platforms"]) == 6
    assert len(cells) == 29
    with output_path.open("a", encoding="utf-8") as output:
        output.write("platforms=" + json.dumps(policy["platforms"]) + "\n")
        output.write("cells=" + json.dumps(cells) + "\n")


def write_policy(output_path: Path, policy_path: Path = POLICY_PATH) -> None:
    """Write the aggregate policy overlay used by the B.4a qualification step."""
    output_path.write_text(json.dumps(read_policy(policy_path)), encoding="utf-8")


def main() -> None:
    """Run one workflow policy operation."""
    parser = argparse.ArgumentParser()
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("matrix")
    write_policy_parser = subcommands.add_parser("write-policy")
    write_policy_parser.add_argument("out", type=Path)
    args = parser.parse_args()
    if args.command == "matrix":
        matrix(Path(os.environ["GITHUB_OUTPUT"]))
    else:
        write_policy(args.out)


if __name__ == "__main__":
    main()
