"""Local selector, dispatch correlation, and common runner contract checks."""
from contextlib import redirect_stdout, redirect_stderr
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from scripts.integrate import dispatch, run_suite


class SelectionTests(unittest.TestCase):
    def test_defaults_select_eighteen_distinct_pairs(self):
        _, _, matrix, coverage = dispatch.selection(",".join(dispatch.SUITES), ",".join(dispatch.RUNNERS))
        cells = matrix["include"]
        self.assertEqual(18, len(cells))
        self.assertEqual(18, len({(cell["suite"], cell["os"]) for cell in cells}))
        self.assertIn("Full selection", coverage)

    def test_cli_defaults_match_full_selection(self):
        output = io.StringIO()
        with patch.dict("os.environ", {"GITHUB_OUTPUT": "", "GITHUB_STEP_SUMMARY": ""}), redirect_stdout(output):
            self.assertEqual(0, dispatch.main(["--matrix"]))
        cells = json.loads(output.getvalue())["include"]
        self.assertEqual(18, len(cells))
        self.assertEqual({"macos", "windows", "linux"}, {cell["os"] for cell in cells})

    def test_subset_is_exact_and_partial(self):
        _, _, matrix, coverage = dispatch.selection("wheels, collector", "windows,linux")
        self.assertEqual([("wheels", "windows"), ("wheels", "linux"),
                          ("collector", "windows"), ("collector", "linux")],
                         [(cell["suite"], cell["os"]) for cell in matrix["include"]])
        self.assertIn("not phase-end coverage", coverage)
        self.assertEqual("windows-2022", matrix["include"][0]["runner"])

    def test_invalid_and_duplicate_selectors(self):
        for value in ("", "all", "wheels,", ",wheels", "wheels,wheels", "wheels,unknown"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                dispatch.selection(value, "linux")
        for value in ("", "darwin", "linux,linux", "linux,,windows"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                dispatch.selection("wheels", value)

    def test_invalid_selection_never_dispatches(self):
        with patch.object(dispatch, "gh") as gh, redirect_stderr(io.StringIO()):
            self.assertEqual(1, dispatch.main(["branch", "--suite", "missing"]))
        gh.assert_not_called()

    def test_matrix_mode_needs_neither_gh_nor_suites(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            with patch.dict("os.environ", {"GITHUB_OUTPUT": str(output), "GITHUB_STEP_SUMMARY": str(summary)}), \
                 patch.object(dispatch, "gh") as gh, redirect_stdout(io.StringIO()):
                self.assertEqual(0, dispatch.main(["--matrix", "--suite", "tauri", "--os", "linux"]))
            gh.assert_not_called()
            matrix = json.loads(output.read_text().removeprefix("matrix="))
            self.assertEqual("tauri", matrix["include"][0]["suite"])
            self.assertIn("Partial selection", summary.read_text())

    def test_dispatch_ref_and_url_are_request_specific(self):
        runs = [{"displayTitle": "Integration other", "url": "wrong"},
                {"displayTitle": "Integration token", "url": "right"}]
        with patch.object(dispatch.uuid, "uuid4") as identifier, \
             patch.object(dispatch, "gh", side_effect=["", "[]", json.dumps(runs)]) as gh, \
             patch.object(dispatch.time, "sleep"):
            identifier.return_value.hex = "token"
            self.assertEqual("right", dispatch.dispatch("sprint/test", ["wheels"], ["linux"]))
        self.assertEqual(("workflow", "run", "integration.yml", "--ref", "sprint/test",
                          "-f", "suite=wheels", "-f", "os=linux", "-f", "dispatch_id=token"),
                         gh.call_args_list[0].args)

    def test_url_timeout_does_not_claim_execution(self):
        with patch.object(dispatch, "gh", return_value="[]"), patch.object(dispatch.time, "sleep"), \
             self.assertRaisesRegex(RuntimeError, "dispatch accepted.*run URL not yet available"):
            dispatch.dispatch("branch", ["wheels"], ["linux"])

    def test_dispatch_failure_is_nonzero(self):
        with patch.object(dispatch, "gh", side_effect=subprocess.CalledProcessError(1, "gh")), \
             redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            self.assertEqual(1, dispatch.main(["branch"]))


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "-m", "fixture"],
                       cwd=self.root, check=True)
        self.sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.root, text=True).strip()
        self.output = self.root / "output"
        self.runner = self.root / "scripts/integrate/suites/wheels/run.py"

    def test_missing_runner_fails(self):
        with self.assertRaisesRegex(ValueError, "missing suite runner"):
            run_suite.run("wheels", self.sha, self.output, self.root)

    def test_mismatched_head_fails_before_execution(self):
        with self.assertRaisesRegex(ValueError, "does not match"):
            run_suite.run("wheels", "0" * 40, self.output, self.root)

    def test_invalid_contract_inputs(self):
        for suite, sha, output in (("../wheels", self.sha, self.output),
                                   ("wheels", "HEAD", self.output),
                                   ("wheels", self.sha, Path("relative"))):
            with self.subTest(suite=suite, sha=sha, output=output), self.assertRaises(ValueError):
                run_suite.run(suite, sha, output, self.root)

    def test_cli_unknown_suite_uses_shared_runner_validation(self):
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.assertEqual(1, run_suite.main([
                "--suite", "missing", "--source-sha", self.sha, "--output-dir", str(self.output),
            ]))
        self.assertIn("unknown suite: missing", stderr.getvalue())

    def test_entrypoint_receives_arguments_cwd_and_failure_is_preserved(self):
        self.runner.parent.mkdir(parents=True)
        self.runner.write_text(
            "import json, os, pathlib, sys\n"
            "pathlib.Path(sys.argv[4], 'args.json').write_text(json.dumps([sys.argv[1:], os.getcwd()]))\n"
            "sys.exit(7)\n")
        self.assertEqual(7, run_suite.run("wheels", self.sha, self.output, self.root))
        args, cwd = json.loads((self.output / "args.json").read_text())
        self.assertEqual(["--source-sha", self.sha, "--output-dir", str(self.output)], args)
        self.assertEqual(str(self.root), cwd)

    def test_successful_entrypoint(self):
        self.runner.parent.mkdir(parents=True)
        self.runner.write_text("raise SystemExit(0)\n")
        self.assertEqual(0, run_suite.run("wheels", self.sha, self.output, self.root))


if __name__ == "__main__":
    unittest.main()
