from __future__ import annotations

import importlib.machinery
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from unittest import mock

SCRIPTS = Path(__file__).parents[1]
LOADER = importlib.machinery.SourceFileLoader("sanity_run_history", str(SCRIPTS / "sanity-run-history"))
SPEC = importlib.util.spec_from_loader("sanity_run_history", LOADER)
history = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(history)
TABLE = SCRIPTS.parent / "templates" / "sanity-run-table.md.j2"

OLD_ROW = {"task": "obs-d-12-sanity", "sprint": "d-12", "phase": "d", "pr_number": 300, "findings": 1,
           "verdict": "FAIL", "iteration": 1, "completed_at": "2026-09-28T10:00:00Z",
           "completed_local": "03:00", "duration": "5m00s"}


class SanityRunHistoryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.log = self.root / ".sc" / "sanity-log" / "phase-d.jsonl"
        self.log.parent.mkdir(parents=True)
        self.log.write_text(json.dumps(OLD_ROW) + "\n")  # a row written before `commit` was recorded
        self.vars = self.root / "vars.json"
        self.vars.write_text(json.dumps({"sprint": "d-30", "verdict": "PASS", "findings_count": 0}))
        patches = [mock.patch.object(history, "primary_root", lambda: self.root),
                   mock.patch.object(history, "phase_from_bead", lambda bead, sprint: "d")]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)
        self.addCleanup(self.tmp.cleanup)

    def append(self, *extra):
        output = self.root / "table.json"
        history.main(["--vars", str(self.vars), "--task", "obs-d-30.chain.sanity", "--bead", "obs-d-30.chain.dev",
                      "--pr-number", "412", "--iteration", "1", "--started-at", str(time.time()),
                      "--output", str(output), *extra])
        return json.loads(output.read_text())

    def test_append_records_the_checked_commit_and_loads_old_rows(self):
        runs = self.append("--commit", "abc123")["runs"]
        self.assertEqual([run["task"] for run in runs], ["obs-d-30.chain.sanity", "obs-d-12-sanity"])
        self.assertEqual(runs[0]["commit"], "abc123")
        self.assertNotIn("commit", runs[1])
        self.assertEqual(json.loads(self.log.read_text().splitlines()[-1])["commit"], "abc123")

    def test_append_requires_commit(self):
        with self.assertRaises(SystemExit) as raised:
            self.append()
        self.assertIn("--commit", str(raised.exception))
        self.assertEqual(len(self.log.read_text().splitlines()), 1)

    def test_view_is_read_only_newest_first_and_needs_no_append_args(self):
        self.append("--commit", "abc123")
        before = self.log.read_text()
        output = self.root / "view.json"
        history.main(["--view", "--phase", "d", "--limit", "1", "--output", str(output)])
        self.assertEqual(self.log.read_text(), before)
        runs = json.loads(output.read_text())["runs"]
        self.assertEqual([run["commit"] for run in runs], ["abc123"])
        history.main(["--view", "--phase", "phase-d", "--limit", "0", "--output", str(output)])
        self.assertEqual([run["task"] for run in json.loads(output.read_text())["runs"]],
                         ["obs-d-30.chain.sanity", "obs-d-12-sanity"])
        render = subprocess.run(["sc-compose", "render", "--strict", "--file", str(TABLE), "--var-file", str(output)],
                                capture_output=True, text=True)
        self.assertEqual(render.returncode, 0, render.stderr)
        self.assertIn("| d-30 | #412 | 0 | ✅ |", render.stdout)

    def test_view_of_an_empty_phase_writes_no_runs(self):
        output = self.root / "view.json"
        history.main(["--view", "--phase", "e", "--output", str(output)])
        self.assertEqual(json.loads(output.read_text()), {"runs": []})
        self.assertFalse((self.log.parent / "phase-e.jsonl").exists())

    def test_view_requires_a_phase(self):
        with self.assertRaises(SystemExit):
            history.main(["--view", "--output", str(self.root / "view.json")])


if __name__ == "__main__":
    unittest.main()
