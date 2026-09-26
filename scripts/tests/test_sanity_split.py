"""sanity-split renders one assignment per numbered deliverable of a pinned, pushed commit."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

SCRIPT = Path(__file__).parents[2] / ".claude/skills/atm-bd-orchestration/scripts/sanity-split"
FAKE_LINT = "sh -c 'echo lint-ran; exit 1'"

DESCRIPTION = """## Goal

Retry helper.

## Deliverables

1. Add `pub mod retry` with `pub fn is_retryable(status: u16) -> bool`
   in the types crate.
2. Unit tests for 429, 503 and 404.

## Non-closure

Nothing else.
"""


def bead(description=DESCRIPTION, owned=("crates/types/src/**",)):
    return [{"id": "obs-x-1", "title": "x-1: retry", "description": description, "design": "## Design\n\nplain",
             "acceptance_criteria": "- true for 429\n- tests", "metadata": {"owned_paths": list(owned)}}]


def git(cwd, *argv):
    return subprocess.run(["git", "-C", str(cwd), *argv], check=True, capture_output=True, text=True).stdout.strip()


class Repo:
    """A clone with a bare origin: develop pushed, sprint/x pushed with one change."""

    def __init__(self, root):
        self.origin = root / "origin.git"
        self.wt = root / "wt"
        subprocess.run(["git", "init", "-q", "--bare", "-b", "develop", str(self.origin)], check=True)
        subprocess.run(["git", "clone", "-q", str(self.origin), str(self.wt)], check=True, capture_output=True)
        git(self.wt, "config", "user.email", "t@example.com")
        git(self.wt, "config", "user.name", "t")
        git(self.wt, "checkout", "-q", "-b", "develop")
        self.commit("crates/types/src/lib.rs", "mod query;\n", "base")
        git(self.wt, "push", "-q", "-u", "origin", "develop")
        git(self.wt, "checkout", "-q", "-b", "sprint/x")
        self.commit("crates/types/src/retry.rs", "pub fn is_retryable(s: u16) -> bool { s == 429 }\n", "retry")
        git(self.wt, "push", "-q", "-u", "origin", "sprint/x")
        self.sha = git(self.wt, "rev-parse", "HEAD")
        self.base_sha = git(self.wt, "rev-parse", "origin/develop")

    def commit(self, path, text, message):
        target = self.wt / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
        git(self.wt, "add", "-A")
        git(self.wt, "commit", "-q", "-m", message)


class SanitySplit(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root, ignore_errors=True)
        self.repo = Repo(self.root)
        self.scratch = self.root / "scratch"

    def run_split(self, bead_json=None, worktree=None, commit=None, branch="sprint/x", base="develop", *extra):
        bead_file = self.root / "bead.json"
        bead_file.write_text(json.dumps(bead_json if bead_json is not None else bead()))
        argv = [str(SCRIPT), "--task", "obs-x-1-sanity", "--bead", "obs-x-1", "--worktree", str(worktree or self.repo.wt),
                "--branch", branch, "--commit", commit or self.repo.sha, "--base", base, "--lint-command", FAKE_LINT,
                "--scratch", str(self.scratch), "--bead-json", str(bead_file), *extra]
        return subprocess.run(argv, capture_output=True, text=True)

    def wait_lint(self, exit_file):
        for _ in range(100):
            if Path(exit_file).exists():
                return Path(exit_file).read_text().strip()
            time.sleep(0.05)
        self.fail("lint never wrote its exit file")

    def test_happy_path_renders_one_assignment_per_deliverable(self):
        out = self.run_split(commit=self.repo.sha[:8])
        self.assertEqual(out.returncode, 0, out.stderr)
        manifest = json.loads(out.stdout)
        self.assertEqual((manifest["sha"], manifest["base_sha"], manifest["deliverables_total"]),
                         (self.repo.sha, self.repo.base_sha, 2))
        self.assertEqual(manifest["changed_files"], ["crates/types/src/retry.rs"])
        self.assertEqual(manifest["files_outside_owned_paths"], [])
        self.assertEqual([a["number"] for a in manifest["assignments"]], [1, 2])
        first = manifest["assignments"][0]["assignment"]
        self.assertEqual(first["deliverable"]["number"], 1)
        self.assertIn("pub mod retry", first["deliverable"]["text"])
        self.assertIn("in the types crate.", first["deliverable"]["text"])
        self.assertEqual(first["deliverables_total"], 2)
        self.assertEqual(first["commit"], self.repo.sha)
        self.assertEqual(first["base_sha"], self.repo.base_sha)
        self.assertEqual(first["owned_paths"], ["crates/types/src/**"])
        self.assertEqual(first["sanity_bead"], "obs-x-1-sanity")
        self.assertEqual(manifest["assignments"][1]["assignment"]["deliverable"]["text"],
                         "Unit tests for 429, 503 and 404.")
        self.assertEqual(self.wait_lint(manifest["lint"]["exit_file"]), "1")
        self.assertIn("lint-ran", Path(manifest["lint"]["log"]).read_text())
        self.assertEqual(sorted(os.listdir(self.scratch)), ["obs-x-1-sanity-lint.exit", "obs-x-1-sanity-lint.log"])

    def test_plan_invalid(self):
        cases = {
            "no section": DESCRIPTION.replace("## Deliverables", "## Things"),
            "bulleted not numbered": DESCRIPTION.replace("1. Add", "- Add").replace("2. Unit", "- Unit"),
            "empty": "## Deliverables\n\n## Non-closure\n\nx\n",
            "numbering gap": DESCRIPTION.replace("2. Unit", "3. Unit"),
        }
        for name, description in cases.items():
            with self.subTest(name):
                out = self.run_split(bead(description))
                self.assertEqual(out.returncode, 2, out.stderr)
                self.assertIn("SANITY.PLAN_INVALID", out.stderr)
                self.assertFalse(self.scratch.exists(), "lint must not start for an invalid plan")

    def test_commit_mismatch(self):
        with self.subTest("dirty tree"):
            (self.repo.wt / "scratch.txt").write_text("x")
            out = self.run_split()
            self.assertEqual((out.returncode, "SANITY.COMMIT_MISMATCH" in out.stderr), (4, True), out.stderr)
            (self.repo.wt / "scratch.txt").unlink()
        with self.subTest("HEAD elsewhere"):
            git(self.repo.wt, "checkout", "-q", "develop")
            out = self.run_split()
            self.assertEqual(out.returncode, 4, out.stderr)
            git(self.repo.wt, "checkout", "-q", "sprint/x")
        with self.subTest("not pushed"):
            self.repo.commit("crates/types/src/retry.rs", "pub fn is_retryable(s: u16) -> bool { s >= 500 }\n", "more")
            out = self.run_split(commit=git(self.repo.wt, "rev-parse", "HEAD"))
            self.assertEqual(out.returncode, 4, out.stderr)
            self.assertIn("origin/sprint/x", out.stderr)

    def test_target_unreadable(self):
        with self.subTest("worktree missing"):
            out = self.run_split(worktree=self.root / "nowhere")
            self.assertEqual((out.returncode, "SANITY.TARGET_UNREADABLE" in out.stderr), (3, True), out.stderr)
        with self.subTest("unknown commit"):
            out = self.run_split(commit="deadbeef")
            self.assertEqual(out.returncode, 3, out.stderr)
        with self.subTest("unknown base"):
            out = self.run_split(base="integrate/nope")
            self.assertEqual(out.returncode, 3, out.stderr)

    def test_files_outside_owned_paths(self):
        self.repo.commit("docs/other.md", "stray\n", "stray")
        git(self.repo.wt, "push", "-q", "origin", "sprint/x")
        out = self.run_split(commit=git(self.repo.wt, "rev-parse", "HEAD"))
        self.assertEqual(out.returncode, 0, out.stderr)
        manifest = json.loads(out.stdout)
        self.assertEqual(manifest["changed_files"], ["crates/types/src/retry.rs", "docs/other.md"])
        self.assertEqual(manifest["files_outside_owned_paths"], ["docs/other.md"])
        self.assertEqual(manifest["assignments"][0]["assignment"]["files_outside_owned_paths"], ["docs/other.md"])
        self.wait_lint(manifest["lint"]["exit_file"])

    def test_split_only_skips_git_and_lint(self):
        (self.repo.wt / "dirty.txt").write_text("would fail pinning")
        out = self.run_split(None, None, "abc1234", "sprint/x", "develop", "--split-only")
        self.assertEqual(out.returncode, 0, out.stderr)
        manifest = json.loads(out.stdout)
        self.assertEqual((manifest["sha"], manifest["deliverables_total"], manifest["lint"]), ("abc1234", 2, None))
        self.assertEqual(manifest["changed_files"], [])
        self.assertEqual(len(manifest["assignments"]), 2)
        self.assertFalse(self.scratch.exists())

    def test_usage(self):
        out = subprocess.run([str(SCRIPT), "--task", "t"], capture_output=True, text=True)
        self.assertEqual(out.returncode, 1)


if __name__ == "__main__":
    unittest.main()
