"""check-sanity-result must accept only a complete result for exactly one task and lint command."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).parents[2] / ".claude/skills/atm-bd-orchestration/scripts/check-sanity-result"
SHA = "4f1c2a9" + "0" * 33
TASK, DEV, LINT_CMD = "obs-d-4-sanity", "obs-d-4", "just lint"


def result(verdict="PASS", findings=(), exit_code=0):
    return {"success": True, "error": None, "data": {
        "sanity_bead": TASK, "dev_bead": DEV, "commit_checked": SHA, "verdict": verdict,
        "findings": list(findings),
        "lint": {"command": LINT_CMD, "exit_code": exit_code, "summary": "."},
    }}


FINDING = {"kind": "error", "file": "src/lib.rs", "line": 3, "issue": "inverted condition"}
LINT = {"kind": "lint", "file": "src/lib.rs", "line": 9, "issue": "unused variable"}


def run(body, *args):
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        f.write(body if isinstance(body, str) else json.dumps(body))
    argv = args or (TASK, DEV, SHA, LINT_CMD)
    return subprocess.run([str(SCRIPT), f.name, *argv], capture_output=True, text=True)


def edit(**data):
    r = copy.deepcopy(result())
    r["data"].update(data)
    return r


class CheckSanityResult(unittest.TestCase):
    def test_accepts(self):
        for r in (result(), result("FAIL", [FINDING]), result("FAIL", [LINT], exit_code=1)):
            self.assertEqual(run(r).returncode, 0, r)

    def test_rejects(self):
        cases = {
            "wrong dev bead": edit(dev_bead="obs-d-5"),
            "wrong task": edit(sanity_bead="obs-d-5-sanity"),
            "short sha": edit(commit_checked="4f1c2a9"),
            "PASS with findings": result("PASS", [FINDING]),
            "PASS with lint failure": result("PASS", [], exit_code=1),
            "FAIL without findings": result("FAIL"),
            "lint failure without lint finding": result("FAIL", [FINDING], exit_code=1),
            "finding without line": result("FAIL", [dict(FINDING, line=None)]),
            "unknown verdict": edit(verdict="MAYBE"),
            "missing lint": edit(lint=None),
            "other lint command": edit(lint={"command": "true", "exit_code": 0, "summary": "."}),
            "malformed failure envelope": {"success": False, "data": None, "error": {"code": "SANITY.TIMEOUT"}},
            "not json": "PASS",
        }
        for name, body in cases.items():
            with self.subTest(name):
                self.assertEqual(run(body).returncode, 1)

    def test_failure_envelope_is_classified(self):
        for recoverable, label in ((True, "recoverable"), (False, "fatal")):
            with self.subTest(label):
                out = run({"success": False, "data": None, "error": {
                    "code": "SANITY.COMMIT_MISMATCH", "message": "HEAD moved",
                    "recoverable": recoverable, "suggested_action": "re-pin"}})
                self.assertEqual((out.returncode, out.stdout.strip()), (3, f"SANITY.COMMIT_MISMATCH {label}"))

    def test_usage(self):
        self.assertEqual(run(result(), TASK, DEV, SHA).returncode, 2)


if __name__ == "__main__":
    unittest.main()
