"""Caller regressions; actual Windows enforcement is tested by hosted preflight."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from _python_sandbox import Sandbox, bounded_command
from _python_distribution import DistributionError
import _tauri_webview


class NetworkScopeTests(unittest.TestCase):
    def test_completed_command_does_not_wait_for_descendant_output_handles(self):
        child = None
        with tempfile.TemporaryDirectory() as temporary:
            try:
                started = time.monotonic()
                result = bounded_command([sys.executable, '-u', '-c',
                    'import subprocess,sys; '
                    'child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(30)"]); '
                    'print(child.pid,flush=True)'], Path(temporary), dict(os.environ), timeout=2)
                child = int(result.stdout.strip())
                self.assertEqual(result.returncode, 0)
                self.assertLess(time.monotonic() - started, 2)
            finally:
                if child is not None:
                    try: os.kill(child, signal.SIGTERM)
                    except ProcessLookupError: pass

    def sandbox(self, active=True):
        value = Sandbox.__new__(Sandbox)
        value.system = 'Windows'
        value.identity = Mock(active=active)
        value.env = {'PATH': 'reviewed-tools'}
        value.commands = []
        return value

    def test_webview_cannot_launch_without_identity_policy(self):
        sandbox = self.sandbox(active=False)
        with patch.object(_tauri_webview, '_execute') as execute:
            with self.assertRaisesRegex(DistributionError, 'policy is not active'):
                _tauri_webview.execute(sandbox, None, None, None, None)
        execute.assert_not_called()

    def test_windows_spawn_uses_identity_for_pipe_and_stderr(self):
        sandbox = self.sandbox()
        stderr = object()
        with patch('subprocess.Popen') as unprotected:
            actual = sandbox.spawn(['bare-program'], Path('/proof'), stdout=subprocess.PIPE, stderr=stderr)
        self.assertIs(actual, sandbox.identity.spawn.return_value)
        sandbox.identity.spawn.assert_called_once_with(['bare-program'], cwd=Path('/proof'),
            environment=sandbox.env, stdout=subprocess.PIPE, stderr=stderr)
        unprotected.assert_not_called()

    def test_windows_build_uses_same_identity_as_webview(self):
        sandbox = self.sandbox()
        sandbox.identity.run.return_value = subprocess.CompletedProcess([], 0, 'compiled', '')
        with patch('_python_sandbox.bounded_command') as unprotected:
            self.assertEqual(sandbox.run(['cargo', 'build'], Path('/proof')), 'compiled')
        sandbox.identity.run.assert_called_once_with(['cargo', 'build'], cwd=Path('/proof'),
                                                     environment=sandbox.env, timeout=900)
        unprotected.assert_not_called()

    def test_webview_failure_does_not_remove_policy_before_sandbox_cleanup(self):
        sandbox = self.sandbox()
        with patch.object(_tauri_webview, '_execute', side_effect=RuntimeError('native failure')):
            with self.assertRaisesRegex(RuntimeError, 'native failure'):
                _tauri_webview.execute(sandbox, None, None, None, None)
        sandbox.identity.close.assert_not_called()
        sandbox.cache_probe = Mock()
        sandbox.__exit__(None, None, None)
        sandbox.identity.close.assert_called_once()


if __name__ == '__main__': unittest.main()
