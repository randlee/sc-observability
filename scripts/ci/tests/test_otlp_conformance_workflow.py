#!/usr/bin/env python3
"""Regression coverage for the bounded OTLP conformance workflow."""

from __future__ import annotations

import unittest
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / ".github" / "workflows" / "otlp-conformance.yml"
RUNNER = ROOT / "scripts" / "integrate" / "suites" / "collector" / "run.py"


class OtlpConformanceWorkflowTests(unittest.TestCase):
    def test_collector_job_is_the_only_job_and_is_bounded(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        jobs = workflow["jobs"]

        self.assertEqual(list(jobs), ["collector-conformance"])
        self.assertEqual(jobs["collector-conformance"]["timeout-minutes"], 90)
        self.assertNotIn("strategy", jobs["collector-conformance"])

    def test_collector_job_runs_the_real_collector_suite_runner(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["collector-conformance"]["steps"]
        commands = [step["run"] for step in steps if "run" in step]

        self.assertEqual(len(commands), 1)
        self.assertEqual(
            commands[0],
            "python3 scripts/integrate/suites/collector/run.py "
            '--source-sha "$GITHUB_SHA" --output-dir "$RUNNER_TEMP/collector-evidence"',
        )
        self.assertTrue(RUNNER.is_file())
        self.assertNotIn("full_stack_integration", WORKFLOW.read_text(encoding="utf-8"))

    def test_dispatch_trigger_and_actions_are_immutable(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        triggers = workflow.get("on") or workflow[True]
        self.assertIn("workflow_dispatch", triggers)
        self.assertNotIn("pull_request", triggers)
        self.assertNotIn("push", triggers)

        jobs = workflow["jobs"]
        uses = [step["uses"] for job in jobs.values() for step in job["steps"]
                if "uses" in step]
        self.assertEqual(uses.count(
            "actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09"), 1)
        self.assertEqual(uses.count(
            "dtolnay/rust-toolchain@6bed0761d98439e5a578e2877258200ad565ba87"), 1)


if __name__ == "__main__":
    unittest.main()
