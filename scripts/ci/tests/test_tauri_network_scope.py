"""The real desktop process must retain Windows network denial until exit."""
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from _python_sandbox import Sandbox
import _tauri_webview


class NetworkScopeTests(unittest.TestCase):
    def sandbox(self, system='Windows'):
        value = Sandbox.__new__(Sandbox)
        value.system = system
        value.firewall = 'qualification-test-owned-rule'
        value.commands = []
        value.powershell = value.commands.append
        return value

    def test_webview_is_inside_rule_scope(self):
        sandbox = self.sandbox()
        def process(*args):
            self.assertEqual(len(sandbox.commands), 1)
            self.assertIn('New-NetFirewallRule', sandbox.commands[0])
            return 'finished'
        with patch.object(_tauri_webview, '_execute', side_effect=process):
            self.assertEqual(_tauri_webview.execute(sandbox, None, None, None, None), 'finished')
        self.assertEqual(len(sandbox.commands), 2)
        self.assertIn("$policy.Rules.Remove('qualification-test-owned-rule')", sandbox.commands[1])
        self.assertIn('Rules.Remove', sandbox.commands[1])

    def test_webview_failure_still_removes_only_own_rule(self):
        sandbox = self.sandbox()
        with patch.object(_tauri_webview, '_execute', side_effect=RuntimeError('native failure')):
            with self.assertRaisesRegex(RuntimeError, 'native failure'):
                _tauri_webview.execute(sandbox, None, None, None, None)
        self.assertEqual(len(sandbox.commands), 2)
        self.assertIn('Rules.Remove', sandbox.commands[1])

    def test_non_windows_does_not_change_firewall(self):
        sandbox = self.sandbox('Darwin')
        with sandbox.network_denial():
            pass
        self.assertEqual(sandbox.commands, [])


if __name__ == '__main__':
    unittest.main()
