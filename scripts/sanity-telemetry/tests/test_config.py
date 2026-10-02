from __future__ import annotations

import subprocess
from pathlib import Path

import yaml


ROOT = Path(__file__).parents[3]


def test_committed_config_and_state_ignore_contract() -> None:
    value = yaml.safe_load((ROOT / ".sc/telemetry.yaml").read_text())
    assert value["store"]["path"] == ".sc/telemetry-state/store.sqlite"
    assert value["service"] == "sc-observability" and value["github"]["pr_url_template"].endswith("{pr_number}")
    assert subprocess.run(["git", "check-ignore", ".sc/telemetry-state/x"], cwd=ROOT).returncode == 0
