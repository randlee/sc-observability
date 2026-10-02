from __future__ import annotations

import subprocess
from pathlib import Path

import yaml


ROOT = Path(__file__).parents[3]


def test_committed_config_and_state_ignore_contract() -> None:
    value = yaml.safe_load((ROOT / ".sc/telemetry.yaml").read_text())
    assert value == {
        "service": "sc-observability",
        "team": "sc-obs",
        "github": {"pr_url_template": "https://github.com/randlee/sc-observability/pull/{pr_number}"},
        "otlp": {"endpoint": "http://localhost:4318"},
        "store": {"path": ".sc/telemetry-state/store.sqlite"},
        "sources": [
            {"path": ".sc/qa-log/phase-d.jsonl", "kind": "qa", "phase": "phase-d"},
            {"path": ".sc/sanity-log/sanity-llm.jsonl", "kind": "sanity", "phase": "phase-d", "reviewer": "sanity-llm"},
            {"path": ".sc/sanity-log/phase-d.jsonl", "kind": "sanity", "phase": "phase-d"},
            {"path": ".sc/qa-log/phase-d-stats.jsonl", "kind": "finding-counts", "phase": "phase-d"},
        ],
    }
    assert subprocess.run(["git", "check-ignore", ".sc/telemetry-state/x"], cwd=ROOT).returncode == 0
