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


if __name__ == "__main__":
    unittest.main()
