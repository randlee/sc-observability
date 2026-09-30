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

SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
LOADER = importlib.machinery.SourceFileLoader("wave_monitor", str(SCRIPTS / "wave-monitor"))
SPEC = importlib.util.spec_from_loader("wave_monitor", LOADER)
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)

ROOT = "obs-phase-d"


def bead(id, parent, status="closed", labels=(), close_reason="", notes="", **metadata):
    return {"id": id, "parent": parent, "status": status, "labels": ["phase-d", *labels],
            "close_reason": close_reason, "notes": notes, "metadata": metadata}


def sprint(n, wave, *, dev=None, sanity=None, qa="closed", sanity_reason=None, notes=""):
    """A sprint container and its chain; `dev` is the dev step's integration metadata."""
    s = f"obs-d-{n}"
    head = (dev or {}).get("stack_head", "")
    return [
        bead(s, ROOT, "open", ["stage:sprint", f"wave:{wave}"], wave=wave, sprint=f"d-{n}"),
        bead(f"{s}.chain", s, "open"),
        bead(f"{s}.chain.dev", f"{s}.chain", "closed", ["stage:dev"], notes=notes, sprint_bead=s, wave=wave, **(dev or {})),
        bead(f"{s}.chain.sanity", f"{s}.chain", sanity or "closed", ["stage:dev-sanity"],
             sanity_reason or f"PASS at {head}", sprint_bead=s, dev_bead=f"{s}.chain.dev"),
        bead(f"{s}.chain.qa", f"{s}.chain", qa, ["stage:qa"], sprint_bead=s, checked_bead=f"{s}.chain.dev"),
    ]


def finding(id, parent, severity, status="open", close_reason="", **metadata):
    return bead(id, parent, status, ["stage:finding", f"severity:{severity}"], close_reason, severity=severity, **metadata)


def phase_beads():
    return [
        *sprint(30, 1, dev={"stack_head": "h30", "sanity_pass_commit": "h30"}),  # complete
        *sprint(31, 1, sanity="open"),  # dev closed, never integrated
        *sprint(32, 1, dev={"stack_head": "h32", "sanity_pass_commit": "h32old"}, sanity_reason="PASS at h32old",
                notes="restack: h32old -> h32"),
        *sprint(33, 1, dev={"stack_head": "h33", "sanity_pass_commit": "h33"}, qa="open"),
        *sprint(34, 1, dev={"stack_head": "h34", "sanity_pass_commit": "h34"}),
        finding("obs-d-34-qa-f1", "obs-d-34", "blocking", sprint_bead="obs-d-34"),
        *sprint(40, 2),  # another wave, not integrated
        # an important finding left open by its fixer, with sanity and QA at its head
        finding("obs-phase-d-f1", ROOT, "important", fixed_at_commit="hf1", exec_wave=1, stack_head="hf1",
                sanity_pass_commit="hf1", sprint_bead="obs-d-30"),
        bead("obs-phase-d-f1-sanity", "obs-phase-d-f1", "closed", ["stage:dev-sanity"], "PASS at hf1", dev_bead="obs-phase-d-f1"),
        bead("obs-phase-d-f1-qa", "obs-phase-d-f1", "closed", ["stage:qa"], "PASS", commit="hf1", checked_bead="obs-phase-d-f1"),
        finding("obs-phase-d-f2", ROOT, "minor"),
        finding("obs-phase-d-f3", ROOT, "important", status="closed", close_reason="fixed at abc"),
        # legacy model: sanity beads beside dev beads, no stack_head anywhere
        bead("obs-d-8", ROOT, "closed", ["stage:dev", "stage:sprint"], sanity_pass_commit="c8"),
        bead("obs-d-8-sanity", ROOT, "closed", ["stage:dev-sanity"], "PASS at c8 by lead ruling", dev_bead="obs-d-8"),
        bead("obs-d-9-sanity", ROOT, "closed", ["stage:dev-sanity"], "PASS at c9", dev_bead="obs-d-9"),
    ]


def row(task, sprint, iteration, verdict, findings=0, commit=None, pr=500):
    value = {"task": task, "sprint": sprint, "phase": "d", "pr_number": pr, "findings": findings, "verdict": verdict,
             "iteration": iteration, "completed_at": "2026-09-29T00:00:00Z", "completed_local": "17:00", "duration": "1m00s"}
    if commit:
        value["commit"] = commit
    return value


LEDGER = [
    row("obs-d-8-sanity", "d-8", 1, "FAIL", 3),  # legacy rows: no commit field
    row("obs-d-8-sanity", "d-8", 2, "FAIL", 2),
    row("obs-d-8-sanity", "d-8", 3, "FAIL", 1),
    row("obs-d-7-sanity", "d-7", 2, "PASS", 1),
    row("obs-d-30.chain.sanity", "d-30", 1, "PASS", commit="h30"),
    row("obs-d-32.chain.sanity", "d-32", 4, "PASS", commit="h32old", pr=None),
    row("obs-d-33.chain.sanity", "d-33", 1, "PASS", commit="h33"),
    row("obs-d-34.chain.sanity", "d-34", 1, "PASS", commit="h34"),
    row("obs-phase-d-f1-sanity", "d-30", 1, "PASS", commit="hf1"),
]


class FakeRunner:
    def __init__(self, common_dir, beads, fail_bd=False):
        self.common_dir, self.beads, self.fail_bd = common_dir, beads, fail_bd

    def __call__(self, args, **kwargs):
        if args[0] == "bd" and self.fail_bd:
            return subprocess.CompletedProcess(args, 1, "", "dolt server unreachable")
        if args[:2] == ["bd", "show"]:
            return subprocess.CompletedProcess(args, 0, json.dumps([bead(ROOT, None, "open", [], phase="d")]), "")
        if args[:2] == ["bd", "list"]:
            assert args[args.index("--label") + 1] == "phase-d"
            return subprocess.CompletedProcess(args, 0, json.dumps(self.beads), "")
        if args[:2] == ["git", "rev-parse"]:
            return subprocess.CompletedProcess(args, 0, str(self.common_dir) + "\n", "")
        return subprocess.run(args, **kwargs)  # sc-compose renders the real template


class WaveMonitorTests(unittest.TestCase):
    def monitor(self, *argv, ledger=LEDGER, beads=None, fail_bd=False):
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / ".sc" / "sanity-log" / "phase-d.jsonl"
            log.parent.mkdir(parents=True)
            log.write_text("".join(json.dumps(r) + "\n" for r in ledger))
            out = io.StringIO()
            with redirect_stdout(out):
                code = module.main(["--root", ROOT, *argv], FakeRunner(Path(tmp) / ".git", phase_beads() if beads is None else beads, fail_bd))
        return code, out.getvalue()

    def section(self, text, title):
        body = text.split(f"## {title}", 1)[1]
        return body.split("\n## ", 1)[0]

    def test_table_is_rendered_newest_first_with_timezone(self):
        code, text = self.monitor("--limit", "3")
        self.assertEqual(code, 0)
        table = [line for line in self.section(text, "Sanity runs").splitlines() if line.startswith("| ")]
        self.assertEqual(table[0], "| S | PR | Find | ✓ | Done | Iter |")
        self.assertEqual(table[1:], ["| d-30 | #500 | 0 | ✅ | 17:00 · 1m00s | 1 |",
                                     "| d-34 | #500 | 0 | ✅ | 17:00 · 1m00s | 1 |",
                                     "| d-33 | #500 | 0 | ✅ | 17:00 · 1m00s | 1 |"])
        self.assertRegex(text, r"Done is local time: \S+ \(UTC[+-]\d\d:\d\d\)")

    def test_iteration_signals_for_each_threshold(self):
        _, text = self.monitor()
        signals = self.section(text, "Iteration signals")
        self.assertIn("separate from SANITY.ROUND_CAP", signals)
        self.assertIn("- **PROBLEM**: obs-d-32.chain.sanity (d-32) iteration 4", signals)
        self.assertIn("- investigate: obs-d-8-sanity (d-8) iteration 3 FAIL", signals)
        self.assertIn("- suspect: obs-d-8-sanity (d-8) iteration 2 FAIL findings 2", signals)
        self.assertNotIn("obs-d-7-sanity", signals)  # iteration 2 with one finding
        self.assertNotIn("iteration 1", signals)  # iteration 1 FAIL is normal

    def test_ledger_missing_and_stale_rows(self):
        _, text = self.monitor()
        lines = self.section(text, "Ledger reconciliation").splitlines()
        self.assertIn("- missing: obs-d-9-sanity closed (PASS at c9) with no ledger row", lines)
        self.assertIn("- stale: obs-d-8-sanity ledger verdict FAIL but close reason 'PASS at c8 by lead ruling'", lines)
        self.assertIn("- stale: obs-d-32.chain.sanity PASS but obs-d-32.chain.dev sanity_pass_commit h32old is not stack_head h32", lines)
        self.assertFalse([l for l in lines if "obs-d-30.chain.sanity" in l or "obs-phase-d-f1-sanity" in l])

    def test_legacy_rows_without_commit_are_missing_commit_evidence(self):
        code, text = self.monitor("--limit", "0")
        self.assertEqual(code, 0)
        lines = self.section(text, "Ledger reconciliation").splitlines()
        self.assertIn("- missing commit evidence: 4 of 9 ledger rows have no commit", lines)
        self.assertIn("- missing commit evidence: obs-d-7-sanity (d-7) iteration 2 PASS findings 1", lines)
        code, text = self.monitor(ledger=[{"task": "obs-d-9-sanity", "verdict": "PASS", "iteration": 1}])
        self.assertEqual(code, 0)  # a sparse legacy row still renders
        self.assertIn("| ? | — | 0 | ✅ | ? · ? | 1 |", text)

    def test_each_handoff_stage(self):
        _, text = self.monitor("--wave", "1")
        handoffs = self.section(text, "Handoffs: wave 1").splitlines()
        self.assertIn("- obs-d-31.chain.dev: missing integrated", handoffs)
        self.assertIn("- obs-d-32.chain.dev: missing sanity", handoffs)
        self.assertIn("- stale: obs-d-32.chain.dev restacked h32old -> h32; sanity_pass_commit h32old", handoffs)
        self.assertIn("- obs-d-33.chain.dev: missing qa", handoffs)
        self.assertIn("- obs-d-34.chain.dev: missing findings", handoffs)
        self.assertIn("- 2 complete", handoffs)  # obs-d-30.chain.dev and the fixed important finding
        self.assertFalse([l for l in handoffs if "obs-d-40" in l or "obs-d-8" in l or "obs-phase-d-f3" in l])

    def test_whole_phase_includes_other_waves_and_closed_fixed_findings(self):
        _, text = self.monitor()
        handoffs = self.section(text, "Handoffs: whole phase").splitlines()
        self.assertIn("- obs-d-40.chain.dev: missing integrated", handoffs)
        self.assertIn("- obs-phase-d-f3: missing integrated", handoffs)

    def test_qa_bead_at_head_satisfies_a_chain_step_with_open_qa(self):
        beads = phase_beads() + [bead("obs-d-33-qa-r2", "obs-d-33.chain.dev", "closed", ["stage:qa"], commit="h33",
                                      checked_bead="obs-d-33.chain.dev")]
        _, text = self.monitor("--wave", "1", beads=beads)
        self.assertNotIn("obs-d-33.chain.dev: missing", text)

    def test_open_finding_counts_by_severity(self):
        _, text = self.monitor()
        self.assertIn("- important: 1\n- minor: 1", self.section(text, "Open findings under the root"))

    def test_bd_failure_exits_2(self):
        code, text = self.monitor(fail_bd=True)
        self.assertEqual((code, text), (2, ""))


if __name__ == "__main__":
    unittest.main()
