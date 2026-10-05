"""Retained Windows headless runner workflow regression."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]

class RunnerWorkflowTests(unittest.TestCase):
    def test_windows_runner_discovery_is_complete_and_uses_fail_fast_bash(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        windows = workflow.split('\n  windows-test:\n', 1)[1]
        block = windows.split('      - name: Run Phase E headless integration runner unit tests\n', 1)[1]
        block = block.split('\n      - name:', 1)[0]
        self.assertIn('        shell: bash\n', block)
        self.assertEqual([
            'python3 -m unittest discover -s scripts/integrate/suites/collector -p test_run.py',
            'python3 -m unittest discover -s scripts/integrate/suites/rust-consumers -p test_run.py',
            'python3 -m unittest discover -s scripts/integrate/suites/rust-viewer -p test_run.py',
            'python3 -m unittest discover -s scripts/integrate/suites/tauri/tests -p test_run.py',
            'python3 -m unittest discover -s scripts/integrate/suites/wheel-cli-viewer/tests -p test_run.py',
            'python3 -m unittest discover -s scripts/integrate/suites/wheels/tests -p test_run.py',
            "python3 -m unittest discover -s scripts/ci/fixtures/otlp/desktop-viewer -p 'test_*.py'",
        ], [line.strip() for line in block.splitlines() if line.strip().startswith('python3 ')])

