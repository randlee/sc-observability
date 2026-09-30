from __future__ import annotations

from contextlib import redirect_stdout
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import unittest

SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
LOADER = importlib.machinery.SourceFileLoader("sprint_closable", str(SCRIPTS / "sprint-closable"))
SPEC = importlib.util.spec_from_loader("sprint_closable", LOADER)
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)

SPRINT = "obs-d-30"


def bead(id, parent, status="closed", labels=(), **metadata):
    return {"id": id, "parent": parent, "status": status, "labels": list(labels), "metadata": metadata}


def chain(dev="closed", sanity="closed", qa="closed"):
    return [
        bead(f"{SPRINT}.chain", SPRINT, status="open"),  # the chain bead itself need not be closed
        bead(f"{SPRINT}.chain.dev", f"{SPRINT}.chain", status=dev, labels=["stage:dev"]),
        bead(f"{SPRINT}.chain.sanity", f"{SPRINT}.chain", status=sanity, labels=["stage:dev-sanity"]),
        bead(f"{SPRINT}.chain.qa", f"{SPRINT}.chain", status=qa, labels=["stage:qa"]),
    ]


def finding(id, parent, severity, status="open"):
    return bead(id, parent, status=status, labels=["stage:finding", f"severity:{severity}"], severity=severity)


class FakeBd:
    """Answers `bd show` and `bd list --parent` from a flat bead list, as bd returns direct children."""

    def __init__(self, rows, fail=False):
        self.rows, self.fail = rows, fail

    def __call__(self, args, **kwargs):
        if self.fail:
            return subprocess.CompletedProcess(args, 1, "", "dolt server unreachable")
        if args[:2] == ["bd", "show"]:
            return subprocess.CompletedProcess(args, 0, json.dumps([bead(SPRINT, "obs-phase-d", "open", ["stage:sprint"])]), "")
        parent = args[args.index("--parent") + 1]
        return subprocess.CompletedProcess(args, 0, json.dumps([r for r in self.rows if r["parent"] == parent]), "")


def closable(rows, fail=False):
    out = io.StringIO()
    with redirect_stdout(out):
        code = module.main([SPRINT], FakeBd(rows, fail))
    return code, out.getvalue().splitlines()


class SprintClosableTests(unittest.TestCase):
    def test_closed_chain_without_blocking_findings_is_closable(self):
        rows = chain() + [finding(f"{SPRINT}-qa-f1", SPRINT, "blocking", status="closed")]
        self.assertEqual(closable(rows), (0, ["closable"]))

    def test_open_step_is_reported(self):
        code, lines = closable(chain(qa="open"))
        self.assertEqual(code, 1)
        self.assertEqual(lines, [f"qa step {SPRINT}.chain.qa is open"])

    def test_open_blocking_finding_under_a_step_is_found_at_depth(self):
        rows = chain() + [finding(f"{SPRINT}.chain.dev.1", f"{SPRINT}.chain.dev", "blocking", status="in_progress")]
        code, lines = closable(rows)
        self.assertEqual(code, 1)
        self.assertEqual(lines, [f"blocking finding {SPRINT}.chain.dev.1 is in_progress (parent {SPRINT}.chain.dev)"])

    def test_open_important_and_minor_findings_are_ignored(self):
        rows = chain() + [finding(f"{SPRINT}-qa-f2", SPRINT, "important"), finding(f"{SPRINT}-qa-f3", SPRINT, "minor")]
        self.assertEqual(closable(rows), (0, ["closable"]))

    def test_sprint_without_a_chain(self):
        self.assertEqual(closable([finding(f"{SPRINT}-qa-f1", SPRINT, "blocking")]), (1, ["no chain"]))

    def test_bd_failure_exits_2(self):
        self.assertEqual(closable(chain(), fail=True), (2, []))


if __name__ == "__main__":
    unittest.main()
