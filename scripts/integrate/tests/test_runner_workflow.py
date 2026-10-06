"""Retained Windows headless runner workflow regression."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]

class RunnerWorkflowTests(unittest.TestCase):
    def test_rust_consumers_linux_provisions_the_required_isolation_helper(self):
        workflow = (ROOT / '.github/workflows/integration.yml').read_text()
        suite = workflow.split('\n  suite:\n', 1)[1]
        setup = suite.split('      - name: Install Linux isolation helper for Rust consumers\n', 1)[1]
        setup = setup.split('\n      - name:', 1)[0]
        self.assertIn("matrix.suite == 'rust-consumers' && runner.os == 'Linux'", setup)
        self.assertIn('sudo apt-get update', setup)
        self.assertIn('sudo apt-get install -y bubblewrap', setup)
        self.assertIn('kernel.apparmor_restrict_unprivileged_userns=0', setup)

    def test_integration_provides_a_fetched_trusted_history_base_to_suite_runners(self):
        workflow = (ROOT / '.github/workflows/integration.yml').read_text()
        suite = workflow.split('\n  suite:\n', 1)[1]
        self.assertIn('SC_API_ACCEPTED_BASE: origin/develop', suite)
        self.assertIn('Fetch trusted accepted API history base', suite)
        self.assertIn('git fetch origin develop:refs/remotes/origin/develop --depth=1', suite)

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

    def test_native_public_api_job_is_parallel_three_os_qualification(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        native = workflow.split('\n  native-public-api:\n', 1)[1]
        self.assertNotIn('needs:', native.split('\n    steps:\n', 1)[0])
        self.assertIn('os: [ubuntu-latest, macos-latest, windows-latest]', native)
        self.assertNotIn('SC_API_ACCEPTED_BASE', native)
        self.assertIn('fetch-depth: 0', native)

    def test_native_public_api_setup_and_check_share_a_bash_runner_temp_path(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        native = workflow.split('\n  native-public-api:\n', 1)[1]
        setup = native.split('      - name: Generate native public API JSON and stock text snapshots\n', 1)[1]
        setup = setup.split('\n      - name:', 1)[0]
        check = native.split('      - name: Compare native public API text without a build\n', 1)[1]
        check = check.split('\n      - name:', 1)[0]
        target = '"$RUNNER_TEMP/e-api-public-api"'
        self.assertIn('        shell: bash\n', setup)
        self.assertIn('        shell: bash\n', check)
        self.assertIn(f'setup --target-dir {target}', setup)
        self.assertIn(f'check --target-dir {target}', check)

    def test_api_only_dispatch_skips_workspace_jobs_and_keeps_native_api_job(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('      api_only:\n        type: boolean', workflow)
        for job in ('fmt', 'clippy', 'docs-consistency', 'version-literals', 'manifest-validation', 'test', 'windows-clippy', 'windows-test'):
            section = workflow.split(f'\n  {job}:\n', 1)[1]
            header = section.split('\n    steps:\n', 1)[0]
            self.assertIn("inputs.api_only != true", header, job)
        native = workflow.split('\n  native-public-api:\n', 1)[1]
        self.assertNotIn('if:', native.split('\n    steps:\n', 1)[0])

    def test_workspace_test_jobs_no_longer_own_native_public_api_steps(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        sections = {
            'test': workflow.split('\n  test:\n', 1)[1].split('\n  windows-clippy:\n', 1)[0],
            'windows-test': workflow.split('\n  windows-test:\n', 1)[1].split('\n  native-public-api:\n', 1)[0],
        }
        for job, section in sections.items():
            self.assertNotIn('Generate native public API JSON and stock text snapshots', section)
            self.assertNotIn('Compare native public API text without a build', section)
