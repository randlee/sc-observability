"""Direct-entrypoint regressions for the shared API history helper."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from scripts.api import history


class HistoryScriptTests(unittest.TestCase):
    def test_row_differences_runs_from_a_foreign_directory_without_pythonpath(self):
        environment = os.environ.copy()
        environment.pop("PYTHONPATH", None)
        payload = {"reference": ["one", "two"], "candidate": ["two", "three"]}
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [sys.executable, str(Path(history.__file__)), "--row-differences"],
                cwd=directory,
                env=environment,
                input=json.dumps(payload),
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), ["- one", "+ three"])


if __name__ == "__main__":
    unittest.main()
