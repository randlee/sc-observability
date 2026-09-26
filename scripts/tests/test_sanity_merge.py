"""sanity-merge accepts exactly one result per deliverable and folds in the lint result."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).parents[2]
SCRIPT = ROOT / ".claude/skills/atm-bd-orchestration/scripts/sanity-merge"
TEMPLATES = ROOT / ".claude/skills/atm-bd-orchestration/templates"
SHA = "4f1c2a9" + "0" * 33
TASK, DEV, SPRINT = "obs-d-4-sanity", "obs-d-4", "d-4"
FINDING = {"kind": "skipped", "file": "crates/x/src/lib.rs", "line": 3, "issue": "no 503 test"}


def result(number, findings=()):
    return {"success": True, "error": None, "data": {
        "sanity_bead": TASK, "dev_bead": DEV, "deliverable": number, "commit_checked": SHA,
        "findings": list(findings)}}


class SanityMerge(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.log = self.dir / "lint.log"
        self.exit = self.dir / "lint.exit"
        self.manifest = self.dir / "manifest.json"
        self.manifest.write_text(json.dumps({
            "sha": SHA, "base_sha": "0" * 40, "sanity_bead": TASK, "dev_bead": DEV, "branch": "sprint/d-4-x",
            "worktree_path": "/wt", "deliverables_total": 2, "changed_files": [], "files_outside_owned_paths": [],
            "lint": {"command": "just lint", "log": str(self.log), "exit_file": str(self.exit)},
            "assignments": []}))

    def lint(self, exit_code, log=""):
        self.log.write_text(log)
        self.exit.write_text(f"{exit_code}\n")

    def merge(self, results, *args):
        argv = args or (str(self.manifest), TASK, DEV, SPRINT)
        return subprocess.run([str(SCRIPT), *argv], input=json.dumps(results), capture_output=True, text=True)

    def test_pass(self):
        self.lint(0, "ok\n")
        out = self.merge([result(2), result(1)])
        self.assertEqual(out.returncode, 0, out.stderr)
        vars_ = json.loads(out.stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"], vars_["commit"], vars_["branch"], vars_["sprint"]),
                         ("PASS", 0, SHA, "sprint/d-4-x", SPRINT))
        self.assertEqual(vars_["findings_md"], "D1: done\nD2: done")
        self.assertEqual(vars_["lint_md"], "`just lint` exit 0")
        self.assertRegex(vars_["generated_at"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.render_complete(vars_, "# Dev Sanity Check PASS")

    def test_fail_from_skipped_finding(self):
        self.lint(0)
        out = self.merge([result(1), result(2, [FINDING])])
        self.assertEqual(out.returncode, 0, out.stderr)
        vars_ = json.loads(out.stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"]), ("FAIL", 1))
        self.assertEqual(vars_["findings_md"], "D1: done\nD2:\n- `crates/x/src/lib.rs:3` skipped: no 503 test")
        self.render_complete(vars_, "# Dev Sanity Check FAIL")

    def test_lint_failure_parsed_to_file_and_line(self):
        self.lint(101, "warning: unused variable\n  --> /wt/crates/x/src/lib.rs:9:5\n"
                       "Diff in /wt/crates/x/src/fmt.rs:12:\nerror: could not compile\n")
        out = self.merge([result(1), result(2)])
        self.assertEqual(out.returncode, 0, out.stderr)
        vars_ = json.loads(out.stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"]), ("FAIL", 2))
        self.assertIn("- `crates/x/src/lib.rs:9` lint:", vars_["lint_md"])
        self.assertIn("- `crates/x/src/fmt.rs:12` lint:", vars_["lint_md"])
        self.assertTrue(vars_["lint_md"].startswith("`just lint` exit 101"))

    def test_lint_failure_without_locations_still_fails(self):
        self.lint(2, "just: recipe failed\n")
        vars_ = json.loads(self.merge([result(1), result(2)]).stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"]), ("FAIL", 1))
        self.assertIn("without locatable diagnostics", vars_["lint_md"])

    def test_rejects(self):
        self.lint(0)
        wrong_sha = copy.deepcopy(result(2))
        wrong_sha["data"]["commit_checked"] = "1" * 40
        bad_finding = result(2, [dict(FINDING, line=None)])
        cases = {
            "missing deliverable": ([result(1)], "deliverable 2: no result"),
            "duplicate": ([result(1), result(1)], "deliverable 1: more than one result"),
            "wrong sha": ([result(1), wrong_sha], "deliverable 2: commit_checked"),
            "wrong task": ([result(1), {**result(2), "data": {**result(2)["data"], "sanity_bead": "other"}}],
                           "deliverable 2: sanity_bead"),
            "out of range": ([result(1), result(3)], "result 1: deliverable"),
            "finding without line": ([result(1), bad_finding], "deliverable 2: finding"),
            "not an array": ({"success": True}, "JSON array"),
            "not json": ("PASS", "not JSON"),
        }
        for name, (body, message) in cases.items():
            with self.subTest(name):
                out = subprocess.run([str(SCRIPT), str(self.manifest), TASK, DEV, SPRINT],
                                     input=body if isinstance(body, str) else json.dumps(body),
                                     capture_output=True, text=True)
                self.assertEqual(out.returncode, 1, out.stderr)
                self.assertIn(message, out.stderr)

    def test_failure_envelope_is_classified(self):
        self.lint(0)
        for recoverable, label in ((True, "recoverable"), (False, "fatal")):
            with self.subTest(label):
                envelope = {"success": False, "data": None, "error": {
                    "code": "SANITY.TARGET_UNREADABLE", "message": "no such path", "recoverable": recoverable,
                    "suggested_action": "re-pin", "deliverable": 2}}
                out = self.merge([result(1), envelope])
                self.assertEqual((out.returncode, out.stdout.strip()), (3, f"SANITY.TARGET_UNREADABLE {label} 2"))

    def test_malformed_failure_envelope_is_rejected(self):
        self.lint(0)
        out = self.merge([result(1), {"success": False, "data": None, "error": {"code": "SANITY.X"}}])
        self.assertEqual(out.returncode, 1)

    def test_lint_still_running(self):
        out = self.merge([result(1), result(2)])
        self.assertEqual((out.returncode, out.stderr.strip()), (4, "lint still running"))

    def test_split_only_manifest_is_rejected(self):
        manifest = json.loads(self.manifest.read_text())
        manifest["lint"] = None
        self.manifest.write_text(json.dumps(manifest))
        out = self.merge([result(1), result(2)])
        self.assertEqual(out.returncode, 1)
        self.assertIn("--split-only", out.stderr)

    def test_usage(self):
        self.assertEqual(self.merge([], str(self.manifest), TASK).returncode, 1)

    def render_complete(self, vars_, expected_heading):
        var_file = self.dir / "vars.json"
        var_file.write_text(json.dumps(vars_))
        out = subprocess.run(["sc-compose", "render", "--strict", "--root", str(TEMPLATES),
                              "--file", "dev-sanity-complete.md.j2", "--var-file", str(var_file)],
                             capture_output=True, text=True)
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertIn(expected_heading, out.stdout)
        self.assertIn(f'"verdict": "{vars_["verdict"]}"', out.stdout)
        self.assertIn(vars_["findings_md"].splitlines()[-1], out.stdout)


if __name__ == "__main__":
    unittest.main()
