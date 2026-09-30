from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).parents[1] / "sanity-split"

CONTAINER = {"id": "obs-d-30", "title": "d-30: container", "labels": ["phase-d", "stage:sprint", "wave:3"],
             "description": "Sprint doc.\n\n## Deliverables\n1. First container item.\n2. Second container item.\n",
             "metadata": {"owned_paths": ["crates/a/**"]}}
CHAIN_STEP = {"id": "obs-d-30.chain.dev", "title": "d-30: dev", "labels": ["phase-d", "stage:dev", "wave:3"],
              "description": "", "metadata": {"sprint_bead": "obs-d-30", "sprint": "d-30"}}


class SanitySplitSourceTests(unittest.TestCase):
    def split(self, bead, sprint_bead=None):
        with tempfile.TemporaryDirectory() as tmp:
            bead_json = Path(tmp) / "bead.json"
            bead_json.write_text(json.dumps([bead]))
            argv = [sys.executable, str(SCRIPT), "--task", "obs-d-30.chain.sanity", "--bead", bead["id"],
                    "--worktree", tmp, "--branch", "b", "--commit", "c", "--base", "base",
                    "--lint-command", "true", "--scratch", tmp, "--bead-json", str(bead_json), "--split-only"]
            if sprint_bead is not None:
                container_json = Path(tmp) / "sprint.json"
                container_json.write_text(json.dumps([sprint_bead]))
                argv += ["--sprint-bead-json", str(container_json)]
            run = subprocess.run(argv, capture_output=True, text=True)
        return run

    def texts(self, manifest):
        return [item["assignment"] for item in manifest["assignments"]]

    def test_chain_dev_step_reads_deliverables_and_owned_paths_from_its_sprint_container(self):
        run = self.split(CHAIN_STEP, CONTAINER)
        self.assertEqual(run.returncode, 0, run.stderr)
        manifest = json.loads(run.stdout)
        self.assertEqual(manifest["dev_bead"], "obs-d-30.chain.dev")
        self.assertEqual(manifest["deliverables_total"], 2)
        rendered = json.dumps(self.texts(manifest))
        self.assertIn("First container item.", rendered)
        self.assertIn("crates/a/**", rendered)

    def test_chain_dev_step_without_its_container_cannot_split(self):
        run = self.split(CHAIN_STEP, dict(CONTAINER, id="obs-d-99"))
        self.assertEqual(run.returncode, 3, run.stderr)

    def test_legacy_dev_bead_and_finding_read_their_own_description(self):
        legacy = {"id": "obs-d-12", "title": "d-12", "labels": ["stage:dev", "stage:sprint"],
                  "description": "## Deliverables\n1. Legacy item.\n", "metadata": {"owned_paths": ["x/**"]}}
        finding = {"id": "obs-d-12-f1", "title": "f", "labels": ["stage:finding"],
                   "description": "## Deliverables\n1. Fix item.\n", "metadata": {"sprint_bead": "obs-d-12"}}
        for bead, text in ((legacy, "Legacy item."), (finding, "Fix item.")):
            with self.subTest(bead=bead["id"]):
                run = self.split(bead)  # no container: a bd lookup here would fail the split
                self.assertEqual(run.returncode, 0, run.stderr)
                manifest = json.loads(run.stdout)
                self.assertEqual(manifest["deliverables_total"], 1)
                self.assertIn(text, json.dumps(self.texts(manifest)))


if __name__ == "__main__":
    unittest.main()
