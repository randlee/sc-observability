"""Workspace discovery must include new product workspaces without workflow edits."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from scripts.ci.cargo_workspaces import discover, main


class WorkspaceDiscoveryTests(unittest.TestCase):
    def test_discovers_standalone_and_excluded_but_not_members_or_ignored_builds(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / "Cargo.toml").write_text('[workspace]\nmembers=["member"]\nexclude=["examples/*"]\n')
            (root / ".gitignore").write_text("target/\n")
            for folder, content in {
                "member": '[package]\nname="member"\n',
                "new workspace": '[workspace]\n',
                "new workspace/member": '[package]\nname="nested"\n',
                "examples/new-consumer": '[package]\nname="consumer"\n',
                "crates/tests/fixtures/real-consumer": '[workspace]\n',
                "docs/plans/sample": '[workspace]\n',
                "scripts/ci/fixtures/invalid": 'intentionally invalid TOML',
                "target/generated": '[workspace]\n',
            }.items():
                path = root / folder / "Cargo.toml"
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            self.assertEqual([p.as_posix() for p in discover(root)], [
                "Cargo.toml", "crates/tests/fixtures/real-consumer/Cargo.toml",
                "examples/new-consumer/Cargo.toml", "new workspace/Cargo.toml",
            ])

    def test_failure_does_not_prevent_other_workspace_tests(self):
        manifests = [Path("Cargo.toml"), Path("another/Cargo.toml")]
        with patch("sys.argv", ["cargo_workspaces.py", "unit"]), \
             patch("scripts.ci.cargo_workspaces.discover", return_value=manifests), \
             patch("scripts.ci.cargo_workspaces.subprocess.run") as run:
            run.side_effect = [subprocess.CompletedProcess([], 1), subprocess.CompletedProcess([], 0)]
            self.assertEqual(main(), 1)
            self.assertEqual(run.call_count, 2)
            for call in run.call_args_list:
                self.assertIn("--no-fail-fast", call.args[0])


if __name__ == "__main__":
    unittest.main()
