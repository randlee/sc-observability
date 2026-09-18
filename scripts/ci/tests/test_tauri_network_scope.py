"""The real desktop process must retain Windows network denial until exit."""
import sys
import inspect
import subprocess
import os
import signal
import tempfile
import time
import unittest
from pathlib import Path
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
                    try:
                        os.kill(child, signal.SIGTERM)
                    except ProcessLookupError:
                        pass

    def sandbox(self, system='Windows'):
        value = Sandbox.__new__(Sandbox)
        value.system = system
        value.firewall = 'qualification-test-owned-rule'
        value.commands = []
        value.powershell = value.commands.append
        return value

    def test_webview_is_inside_rule_scope(self):
        sandbox = self.sandbox()
        with patch('_python_sandbox.subprocess.check_output', return_value=''):
          def process(*args):
            self.assertTrue(any('New-NetFirewallRule' in item for item in sandbox.commands))
            return 'finished'
          with patch.object(_tauri_webview, '_execute', side_effect=process):
            self.assertEqual(_tauri_webview.execute(sandbox, None, None, None, None), 'finished')
        self.assertTrue(any('Rules.Remove' in item for item in sandbox.commands))

    def test_command_rule_resolves_bare_executable_and_tracks_process_tree(self):
        sandbox = self.sandbox()
        with patch('_python_sandbox.subprocess.check_output', return_value=''):
          with patch('_python_sandbox.shutil.which', return_value='C:/tool/python.exe'):
            with sandbox.network_denial(Path('python')):
                pass
        self.assertTrue(any('-Direction Outbound -Action Block' in item for item in sandbox.commands))
        self.assertIn('-OverrideBlockRules', inspect.getsource(Sandbox.network_denial))

    def test_webview_failure_still_removes_only_own_rule(self):
        sandbox = self.sandbox()
        with patch('_python_sandbox.subprocess.check_output', return_value=''):
            with patch.object(_tauri_webview, '_execute', side_effect=RuntimeError('native failure')):
                with self.assertRaisesRegex(RuntimeError, 'native failure'):
                    _tauri_webview.execute(sandbox, None, None, None, None)
        self.assertTrue(any('Rules.Remove' in item for item in sandbox.commands))

    def test_non_windows_does_not_change_firewall(self):
        sandbox = self.sandbox('Darwin')
        with sandbox.network_denial():
            pass
        self.assertEqual(sandbox.commands, [])

    def test_watchdog_restoration_cannot_turn_timeout_into_success(self):
        sandbox = self.sandbox()
        events = []
        def abort(code):
            events.append(('exit', code))
            raise SystemExit(code)
        with patch.object(sandbox, '__exit__', side_effect=lambda *args: events.append('restore')):
            with patch('_python_sandbox.os._exit', side_effect=abort):
                with self.assertRaises(SystemExit) as error:
                    sandbox.abort_windows_proof()
        self.assertEqual(error.exception.code, 124)
        self.assertEqual(events, ['restore', ('exit', 124)])

    def test_watchdog_still_fails_when_restoration_fails(self):
        sandbox = self.sandbox()
        with patch.object(sandbox, '__exit__', side_effect=RuntimeError('restore failed')):
            with patch('_python_sandbox.os._exit', side_effect=SystemExit(124)):
                with self.assertRaises(SystemExit) as error:
                    sandbox.abort_windows_proof()
        self.assertEqual(error.exception.code, 124)

    def test_acl_failure_does_not_skip_other_saved_roots(self):
        sandbox = self.sandbox()
        sandbox.acls = [(Path('/first/root'), Path('/first.saved')),
                        (Path('/second/root'), Path('/second.saved'))]
        failure = subprocess.TimeoutExpired('icacls', 60)
        with patch('_python_sandbox.subprocess.run', side_effect=[failure, None]) as restore:
            with self.assertRaisesRegex(DistributionError, 'ACL restoration failed'):
                sandbox.restore_acls()
        self.assertEqual(restore.call_count, 2)
        self.assertIn(str(sandbox.acls[0][1]), restore.call_args_list[1].args[0])
        self.assertEqual(restore.call_args_list[1].kwargs['timeout'], 60)

    def test_expired_watchdog_rejects_normal_context_return(self):
        sandbox = self.sandbox()
        def timer(seconds, callback):
            return Mock(start=Mock(side_effect=callback), is_alive=Mock(return_value=False))
        with patch('_python_sandbox.threading.Timer', side_effect=timer):
            with patch('_python_sandbox.subprocess.check_output', return_value=''):
                with patch.object(sandbox, 'abort_windows_proof'):
                    with self.assertRaisesRegex(DistributionError, 'watchdog exceeded'):
                        with sandbox.network_denial():
                            pass


if __name__ == '__main__':
    unittest.main()
