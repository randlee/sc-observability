"""No-GUI tests for the retired Rust viewer suite entrypoint."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


RUNNER = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("rust_viewer_runner", RUNNER)
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class RustViewerRunnerTests(unittest.TestCase):
    def test_retired_suite_writes_explicit_skip_receipt_without_viewer_side_effects(self) -> None:
        source_sha = "a" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            result = runner.run(source_sha, output)

            self.assertEqual("skipped", result["status"])
            self.assertEqual(source_sha, result["source_commit"])
            self.assertIn("full_stack_integration", result["reason"])
            self.assertIn("local collector", result["reason"])
            self.assertEqual(result, json.loads((output / "result.json").read_text(encoding="utf-8")))
            self.assertEqual(["result.json"], [path.name for path in output.iterdir()])

    def test_skip_receipt_rejects_non_commit_source_sha(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, "full 40-hex commit"):
                runner.run("not-a-commit", Path(temporary))


if __name__ == "__main__":
    unittest.main()
