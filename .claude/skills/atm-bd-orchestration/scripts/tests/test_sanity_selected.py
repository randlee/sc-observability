from __future__ import annotations

import argparse
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPTS = Path(__file__).parents[1]


def load(name: str):
    loader = importlib.machinery.SourceFileLoader(name, str(SCRIPTS / name.replace("_", "-")))
    spec = importlib.util.spec_from_loader(name, loader)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


merge = load("sanity_merge")
history = load("sanity_run_history")


def reply(number: int, findings: list[dict] | None = None) -> dict:
    return {
        "success": True,
        "data": {
            "deliverable": number,
            "sanity_bead": "sanity",
            "dev_bead": "dev",
            "commit_checked": "a" * 40,
            "findings": findings or [],
        },
        "error": None,
    }


class SelectedMergeTests(unittest.TestCase):
    manifest = {"deliverables_total": 1, "sha": "a" * 40}

    def selected(self, llm, jev, selection):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "llm.json").write_text(json.dumps([llm]))
            (root / "jev.json").write_text(json.dumps([jev]))
            (root / "selection.json").write_text(json.dumps(selection))
            args = argparse.Namespace(
                llm_results=root / "llm.json", jev_results=root / "jev.json", selection=root / "selection.json"
            )
            return merge.selected_results(args, self.manifest, "sanity", "dev")

    def test_selected_uses_whole_jev_envelope_and_keeps_source(self):
        selected, selection, sources = self.selected(
            reply(1), reply(1, [{"kind": "skipped", "file": "a.rs", "line": 1, "issue": "missing"}]),
            [{"deliverable": 1, "llm": "done", "jev": "undone", "selected": "jev", "reason": "JEV has pinned evidence", "rerun": None, "checker_defect": False}],
        )
        self.assertEqual(selected[0]["data"]["findings"][0]["issue"], "missing")
        self.assertEqual(selection[0]["selected"], "jev")
        self.assertEqual(sources, {1: "sanity-jev"})

    def test_checker_defect_requires_reason(self):
        undone = reply(1, [{"kind": "skipped", "file": "a.rs", "line": 1, "issue": "missing"}])
        with self.assertRaisesRegex(merge.Reject, "requires a reason"):
            self.selected(undone, undone, [{"deliverable": 1, "llm": "undone", "jev": "undone", "selected": "llm", "reason": "", "rerun": None, "checker_defect": True}])

    def test_pick_counts_agreements_and_non_agreement_choices(self):
        rows = history.display_runs([{
            "run_id": "one", "reviewer": "sanity-selected", "sprint": "d", "pr_number": 1,
            "completed_at": "2026-01-01T00:00:00Z", "duration": "1s", "iteration": 1,
            "verdict": "PASS", "findings": 0,
            "selection": [
                {"llm": "done", "jev": "done", "selected": "llm", "checker_defect": False},
                {"llm": "done", "jev": "undone", "selected": "llm", "checker_defect": False},
                {"llm": "done", "jev": "undone", "selected": "rerun", "checker_defect": True},
            ],
        }], 10)
        self.assertEqual(rows[0]["pick"], "=1 L1 R1 D1")

    def test_selected_ledger_record_accepts_selection_list(self):
        record = {
            "run_id": "one", "reviewer": "sanity-selected", "commit": "a" * 40,
            "task": "sanity", "sprint": "d", "phase": "d",
            "started_at": "2026-01-01T00:00:00Z", "completed_at": "2026-01-01T00:00:01Z",
            "duration": "1s", "duration_seconds": 1, "pr_number": 1, "iteration": 1,
            "verdict": "PASS", "findings": 0, "error": None, "selection": [],
        }
        self.assertEqual(history.validate_record(record), record)


if __name__ == "__main__":
    unittest.main()
