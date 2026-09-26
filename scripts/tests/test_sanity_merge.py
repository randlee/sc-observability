"""sanity-merge accepts exactly one result per deliverable, folds in the lint result and re-checks the tree."""
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
TASK, DEV, SPRINT, BRANCH = "obs-d-4-sanity", "obs-d-4", "d-4", "sprint/d-4-x"
FINDING = {"kind": "skipped", "file": "crates/x/src/lib.rs", "line": 3, "issue": "no 503 test"}


def git(cwd, *argv):
    return subprocess.run(["git", "-C", str(cwd), *argv], check=True, capture_output=True, text=True).stdout.strip()


class SanityMerge(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.wt = self.dir / "wt"
        self.wt.mkdir()
        git(self.wt, "init", "-q", "-b", BRANCH)
        git(self.wt, "config", "user.email", "t@example.com")
        git(self.wt, "config", "user.name", "t")
        (self.wt / "lib.rs").write_text("fn main() {}\n")
        git(self.wt, "add", "-A")
        git(self.wt, "commit", "-q", "-m", "pinned")
        self.sha = git(self.wt, "rev-parse", "HEAD")
        self.log = self.dir / "lint.log"
        self.exit = self.dir / "lint.exit"
        self.manifest = self.dir / "manifest.json"
        self.write_manifest()

    def write_manifest(self, **overrides):
        manifest = {
            "sha": self.sha, "base_sha": "0" * 40, "sanity_bead": TASK, "dev_bead": DEV, "branch": BRANCH,
            "worktree_path": str(self.wt), "deliverables_total": 2, "changed_files": [],
            "files_outside_owned_paths": [],
            "lint": {"command": "just lint", "log": str(self.log), "exit_file": str(self.exit), "pid": 1,
                     "timeout_seconds": 1800},
            "assignments": []}
        manifest.update(overrides)
        self.manifest.write_text(json.dumps(manifest))

    def result(self, number, findings=()):
        return {"success": True, "error": None, "data": {
            "sanity_bead": TASK, "dev_bead": DEV, "deliverable": number, "commit_checked": self.sha,
            "findings": list(findings)}}

    def failure(self, **error):
        base = {"code": "SANITY.TARGET_UNREADABLE", "message": "no such path", "recoverable": False,
                "suggested_action": "re-pin", "deliverable": 2}
        base.update(error)
        return {"success": False, "data": None, "error": {k: v for k, v in base.items() if v is not ...}}

    def lint(self, state, log=""):
        self.log.write_text(log)
        self.exit.write_text(f"{state}\n")

    def merge(self, results, *args):
        argv = args or (str(self.manifest), TASK, DEV, SPRINT)
        body = results if isinstance(results, str) else json.dumps(results)
        return subprocess.run([str(SCRIPT), *argv], input=body, capture_output=True, text=True)

    def test_pass(self):
        self.lint(0, "ok\n")
        out = self.merge([self.result(2), self.result(1)])
        self.assertEqual(out.returncode, 0, out.stderr)
        vars_ = json.loads(out.stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"], vars_["commit"], vars_["branch"], vars_["sprint"]),
                         ("PASS", 0, self.sha, BRANCH, SPRINT))
        self.assertEqual(vars_["findings_md"], "D1: done\nD2: done")
        self.assertEqual(vars_["lint_md"], "`just lint` exit 0")
        self.assertRegex(vars_["generated_at"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.render_complete(vars_, "# Dev Sanity Check PASS")

    def test_fail_from_skipped_finding(self):
        self.lint(0)
        out = self.merge([self.result(1), self.result(2, [FINDING])])
        self.assertEqual(out.returncode, 0, out.stderr)
        vars_ = json.loads(out.stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"]), ("FAIL", 1))
        self.assertEqual(vars_["findings_md"], "D1: done\nD2:\n- `crates/x/src/lib.rs:3` skipped: no 503 test")
        self.render_complete(vars_, "# Dev Sanity Check FAIL")

    def test_lint_failure_parsed_to_file_and_line(self):
        self.lint(101, f"warning: unused variable\n  --> {self.wt}/crates/x/src/lib.rs:9:5\n"
                       f"Diff in {self.wt}/crates/x/src/fmt.rs:12:\nerror: could not compile\n")
        out = self.merge([self.result(1), self.result(2)])
        self.assertEqual(out.returncode, 0, out.stderr)
        vars_ = json.loads(out.stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"]), ("FAIL", 2))
        self.assertIn("- `crates/x/src/lib.rs:9` lint:", vars_["lint_md"])
        self.assertIn("- `crates/x/src/fmt.rs:12` lint:", vars_["lint_md"])
        self.assertTrue(vars_["lint_md"].startswith("`just lint` exit 101"))

    def test_lint_failure_without_locations_still_fails(self):
        self.lint(2, "just: recipe failed\n")
        vars_ = json.loads(self.merge([self.result(1), self.result(2)]).stdout)
        self.assertEqual((vars_["verdict"], vars_["findings_count"]), ("FAIL", 1))
        self.assertIn("without locatable diagnostics", vars_["lint_md"])

    def test_rejects(self):
        self.lint(0)
        wrong_sha = copy.deepcopy(self.result(2))
        wrong_sha["data"]["commit_checked"] = "1" * 40
        contradictory = {**self.result(2), "error": {"code": "SANITY.TARGET_UNREADABLE", "recoverable": False}}
        cases = {
            "missing deliverable": ([self.result(1)], "deliverable 2: no result"),
            "duplicate": ([self.result(1), self.result(1)], "deliverable 1: more than one result"),
            "wrong sha": ([self.result(1), wrong_sha], "deliverable 2: commit_checked"),
            "wrong task": ([self.result(1), {**self.result(2), "data": {**self.result(2)["data"], "sanity_bead": "o"}}],
                           "deliverable 2: sanity_bead"),
            "out of range": ([self.result(1), self.result(3)], "result 1: deliverable"),
            "finding without line": ([self.result(1), self.result(2, [dict(FINDING, line=None)])],
                                     "deliverable 2: finding"),
            "success with an error object": ([self.result(1), contradictory], "deliverable 2: a success envelope must have error null"),
            "failure without deliverable": ([self.result(1), self.failure(deliverable=...)], "result 1: failure envelope must name"),
            "failure with out-of-range deliverable": ([self.result(1), self.failure(deliverable=7)], "result 1: failure envelope must name"),
            "failure without message": ([self.result(1), self.failure(message=...)], "result 1: failure envelope must have code"),
            "failure with data": ([self.result(1), {**self.failure(), "data": {}}], "result 1: failure envelope must have data null"),
            "not a result": (["not a result"], "result 0: not a success or failure envelope"),
            "not an array": ({"success": True}, "JSON array"),
            "not json": ("PASS", "not JSON"),
        }
        for name, (body, message) in cases.items():
            with self.subTest(name):
                out = self.merge(body)
                self.assertEqual(out.returncode, 1, out.stderr)
                self.assertIn(message, out.stderr)

    def test_failure_envelope_is_classified(self):
        self.lint(0)
        for recoverable, label in ((True, "recoverable"), (False, "fatal")):
            with self.subTest(label):
                out = self.merge([self.result(1), self.failure(recoverable=recoverable)])
                self.assertEqual((out.returncode, out.stdout.strip()), (3, f"SANITY.TARGET_UNREADABLE {label} 2"))

    def test_lint_still_running(self):
        out = self.merge([self.result(1), self.result(2)])
        self.assertEqual((out.returncode, out.stderr.strip()), (4, "lint still running"))

    def test_lint_unavailable(self):
        for state in ("timeout", "cancelled", "error"):
            with self.subTest(state):
                self.lint(state)
                out = self.merge([self.result(1), self.result(2)])
                self.assertEqual((out.returncode, out.stdout.strip()), (3, "SANITY.LINT_UNAVAILABLE fatal 0"), out.stderr)

    def test_worktree_must_still_be_pinned(self):
        self.lint(0)
        with self.subTest("unreadable"):
            self.write_manifest(worktree_path=str(self.dir / "nowhere"))
            out = self.merge([self.result(1), self.result(2)])
            self.assertEqual((out.returncode, out.stdout.strip()), (3, "SANITY.TARGET_UNREADABLE fatal 0"))
        self.write_manifest()
        with self.subTest("dirty"):
            (self.wt / "new.rs").write_text("x")
            out = self.merge([self.result(1), self.result(2)])
            self.assertEqual((out.returncode, out.stdout.strip()), (3, "SANITY.COMMIT_MISMATCH fatal 0"))
            (self.wt / "new.rs").unlink()
        with self.subTest("other branch"):
            git(self.wt, "checkout", "-q", "-b", "other")
            out = self.merge([self.result(1), self.result(2)])
            self.assertEqual((out.returncode, out.stdout.strip()), (3, "SANITY.COMMIT_MISMATCH fatal 0"))
            git(self.wt, "checkout", "-q", BRANCH)
        with self.subTest("HEAD moved"):
            (self.wt / "lib.rs").write_text("fn main() { moved() }\n")
            git(self.wt, "commit", "-q", "-am", "moved")
            out = self.merge([self.result(1), self.result(2)])
            self.assertEqual((out.returncode, out.stdout.strip()), (3, "SANITY.COMMIT_MISMATCH fatal 0"))

    def test_split_only_manifest_is_rejected(self):
        self.write_manifest(lint=None)
        out = self.merge([self.result(1), self.result(2)])
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
