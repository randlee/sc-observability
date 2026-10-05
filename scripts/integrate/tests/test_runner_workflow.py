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

    def test_windows_public_api_setup_and_check_share_a_bash_runner_temp_path(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        windows = workflow.split('\n  windows-test:\n', 1)[1]
        setup = windows.split('      - name: Generate native public API JSON and stock text snapshots\n', 1)[1]
        setup = setup.split('\n      - name:', 1)[0]
        check = windows.split('      - name: Compare native public API text without a build\n', 1)[1]
        check = check.split('\n      - name:', 1)[0]
        target = '"$RUNNER_TEMP/e-api-public-api"'
        self.assertIn('        shell: bash\n', setup)
        self.assertIn('        shell: bash\n', check)
        self.assertIn(f'setup --target-dir {target}', setup)
        self.assertIn(f'check --target-dir {target}', check)
