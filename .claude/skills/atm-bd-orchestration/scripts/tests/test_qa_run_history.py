from __future__ import annotations

from contextlib import redirect_stdout
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
LOADER = importlib.machinery.SourceFileLoader("qa_run_history", str(SCRIPTS / "qa-run-history"))
SPEC = importlib.util.spec_from_loader("qa_run_history", LOADER)
history = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(history)

ROUND_KEYS = {"completed_at", "completed_local", "duration", "phase", "sprint", "task", "pr_number",
              "iteration", "verdict", "tested", "fnd", "blk", "imp", "min"}
STATS_KEYS = {"snapshot_at", "snapshot_local", "phase", "trigger_task", "tot", "open", "blk", "imp", "min"}
TASK = "obs-d-24-qa-pr519"


def bead(id, status="open", labels=(), **metadata):
    return {"id": id, "status": status, "labels": list(labels), "metadata": {"phase": "d", **metadata}}


def finding(severity, screen="keep"):
    """A rendered finding-bead.json.j2 row: screen and severity live in metadata."""
    return {"id": "x", "metadata": {"severity": severity, "screen": screen}}


class FakeBd:
    def __init__(self, beads):
        self.beads = beads

    def __call__(self, argv, **_):
        if argv[:2] == ["bd", "show"]:
            out = [{"id": argv[2], "created_at": "2026-09-30T06:45:45Z"}]
        elif argv[:2] == ["bd", "list"]:
            out = self.beads
        else:
            raise AssertionError(f"unexpected bd call {argv}")
        return subprocess.CompletedProcess(argv, 0, json.dumps(out), "")


class QaRunHistoryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.findings = self.root / "findings.jsonl"
        patch = mock.patch.object(history, "primary_root", lambda: self.root)
        patch.start()
        self.addCleanup(patch.stop)
        self.addCleanup(self.tmp.cleanup)

    def append(self, beads=(), *extra, pr="519"):
        history.main(["append", "--task", TASK, "--phase", "d", "--sprint", "d-24", "--pr-number", pr,
                      "--iteration", "2", "--verdict", "FAIL", "--tested", "obs-d-24-qa-f1,obs-d-24-qa-f2",
                      "--findings", str(self.findings), *extra], runner=FakeBd(list(beads)))
        log = self.root / ".sc" / "qa-log"
        return (json.loads((log / "phase-d.jsonl").read_text().splitlines()[-1]),
                json.loads((log / "phase-d-stats.jsonl").read_text().splitlines()[-1]))

    def stats(self, *beads):
        return self.append(beads)[1]

    def test_rows_keep_todays_keys_and_types(self):
        round_row, stats_row = self.append()
        self.assertEqual(set(round_row), ROUND_KEYS)
        self.assertEqual(set(stats_row), STATS_KEYS)
        self.assertEqual((round_row["pr_number"], round_row["iteration"], round_row["tested"]),
                         (519, 2, "obs-d-24-qa-f1,obs-d-24-qa-f2"))
        self.assertRegex(round_row["completed_at"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.assertRegex(round_row["completed_local"], r"^\d\d:\d\d$")
        self.assertRegex(round_row["duration"], r"^\d+m\d\ds$")
        self.assertEqual(stats_row["snapshot_at"], round_row["completed_at"])
        self.assertEqual(stats_row["trigger_task"], TASK)
        self.assertIsNone(self.append(pr="")[0]["pr_number"])

    def test_a_pr_style_finding_id_is_counted(self):
        stats = self.stats(bead("obs-d-24-qa-pr519-f1", severity="blocking"))
        self.assertEqual((stats["tot"], stats["open"], stats["blk"]), (1, 1, 1))

    def test_legacy_pr_and_round_finding_ids_are_all_counted(self):
        stats = self.stats(bead("obs-d-19-qa-f17", severity="minor"),
                           bead("obs-d-24-qa-pr522-f3", severity="blocking"),
                           bead("obs-d-24-qa-pr522-r2-f5", severity="important"),
                           bead("obs-d-24-qa-pr524-r3-f1.2", severity="minor"))
        self.assertEqual({k: stats[k] for k in ("tot", "open", "blk", "imp", "min")},
                         {"tot": 4, "open": 4, "blk": 1, "imp": 1, "min": 2})

    def test_a_labelled_finding_without_an_f_id_is_counted(self):
        stats = self.stats(bead("obs-d-ad-hoc-leak", labels=["stage:finding"], severity="important"))
        self.assertEqual((stats["tot"], stats["imp"]), (1, 1))

    def test_an_unlabelled_qa_f_id_is_counted_and_severity_falls_back_to_its_label(self):
        stats = self.stats(bead("obs-d-18-qa-f3", labels=["severity:minor"]))
        self.assertEqual((stats["tot"], stats["min"]), (1, 1))

    def test_in_progress_is_open_and_other_beads_are_not_findings(self):
        stats = self.stats(bead("obs-d-1-qa-f1", status="in_progress", severity="minor"),
                           bead("obs-d-1-qa-f2", status="closed", severity="blocking"),
                           bead("obs-d-1-qa-f1-wf-PLAN_INVALID", labels=["workflow-issue"]),
                           bead("obs-d-24-qa-f3-qa-pr520", labels=["stage:qa"]),
                           {**bead("obs-e-1-qa-f1", severity="minor"), "metadata": {"phase": "e"}})
        self.assertEqual({k: stats[k] for k in ("tot", "open", "blk", "imp", "min")},
                         {"tot": 2, "open": 1, "blk": 0, "imp": 0, "min": 1})

    def test_ceremony_findings_are_excluded_from_round_counts(self):
        rows = [finding("blocking"), finding("important", "ceremony"), finding("minor"), finding("minor"),
                {"severity": "important", "screen": "not_applicable"}]
        self.findings.write_text("".join(json.dumps(row) + "\n" for row in rows))
        round_row = self.append()[0]
        self.assertEqual({k: round_row[k] for k in ("fnd", "blk", "imp", "min")},
                         {"fnd": 4, "blk": 1, "imp": 1, "min": 2})

    def test_a_missing_findings_file_gives_zeros(self):
        round_row = self.append()[0]
        self.assertEqual({k: round_row[k] for k in ("fnd", "blk", "imp", "min")},
                         {"fnd": 0, "blk": 0, "imp": 0, "min": 0})

    def test_dry_run_prints_both_rows_and_appends_nothing(self):
        out = io.StringIO()
        with redirect_stdout(out):
            history.main(["append", "--task", TASK, "--phase", "d", "--sprint", "d-24", "--pr-number", "",
                          "--iteration", "1", "--verdict", "PASS", "--tested", "obs-d-24",
                          "--findings", str(self.findings), "--dry-run"],
                         runner=FakeBd([bead("obs-d-24-qa-pr519-f1", severity="minor")]))
        round_row, stats_row = (json.loads(line) for line in out.getvalue().splitlines())
        self.assertEqual((set(round_row), set(stats_row)), (ROUND_KEYS, STATS_KEYS))
        self.assertEqual(stats_row["min"], 1)
        self.assertFalse((self.root / ".sc").exists())


if __name__ == "__main__":
    unittest.main()
