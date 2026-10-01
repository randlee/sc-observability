#!/usr/bin/env python3
"""Regression coverage for bounded OTLP conformance workflow jobs."""

from __future__ import annotations

import unittest
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / ".github" / "workflows" / "otlp-conformance.yml"


class OtlpConformanceWorkflowTests(unittest.TestCase):
    def test_hermetic_conformance_jobs_have_bounded_timeouts(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        jobs = workflow["jobs"]

        self.assertEqual(jobs["collector-conformance"]["timeout-minutes"], 30)
        self.assertEqual(jobs["canonical-ingress-conformance"]["timeout-minutes"], 30)
        self.assertEqual(jobs["desktop-viewer-factory-conformance"]["timeout-minutes"], 20)

    def test_desktop_viewer_cleanup_has_early_state_and_missing_state_guard(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["desktop-viewer-factory-conformance"]["steps"]
        names = [step.get("name") for step in steps]
        state_step = names.index("Define owned desktop viewer state")
        manifest_step = names.index("Read pinned viewer manifest")
        start_step = names.index("Start an owned isolated desktop viewer")
        cleanup_step = names.index("Remove the owned desktop viewer state")

        self.assertLess(state_step, manifest_step)
        self.assertLess(state_step, start_step)
        self.assertIn('VIEWER_STATE_DIR=$RUNNER_TEMP/sc-observability-d9-viewer',
                      steps[state_step]["run"])
        self.assertIn('[ -d "$VIEWER_STATE_DIR" ]', steps[cleanup_step]["run"])
        self.assertIn('viewer_harness.py stop', steps[cleanup_step]["run"])


if __name__ == "__main__":
    unittest.main()
