"""Recovery must be confined to owned resources and can never qualify a proof."""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import supervise_windows_proof as supervisor
from _python_distribution import DistributionError


class SupervisorTests(unittest.TestCase):
    def record(self, directory):
        path = directory / 'recovery.json'
        root = directory / 'owned-root'
        path.write_text(json.dumps({'schema_version': 1,
            'firewall': 'sc-observability-proof-' + 'a' * 32,
            'acls': [[str(root), str(directory / 'acl-0.txt')]]}))
        return path, root

    def test_missing_plan_does_not_restore_anything(self):
        with tempfile.TemporaryDirectory() as temporary:
            self.assertFalse(supervisor.recover(Path(temporary) / 'absent', set(), Path(temporary)))

    def test_recovery_restores_owned_rule_and_acl_and_removes_plan(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            path, root = self.record(directory)
            events = []
            with patch.object(supervisor.Sandbox, 'remove_firewall', side_effect=lambda: events.append('rule')):
                with patch.object(supervisor.Sandbox, 'restore_acls', side_effect=lambda: events.append('acl')):
                    self.assertTrue(supervisor.recover(path, {root}, directory))
            self.assertEqual(events, ['rule', 'acl'])
            self.assertFalse(path.exists())

    def test_foreign_roots_are_rejected_before_any_restoration(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            path, _ = self.record(directory)
            with patch.object(supervisor.Sandbox, 'remove_firewall') as remove:
                with self.assertRaisesRegex(DistributionError, 'outside owned roots'):
                    supervisor.recover(path, set(), directory)
            remove.assert_not_called()
            self.assertTrue(path.exists())

    def execute(self, process, recovered):
        with patch.object(sys, 'argv', ['supervisor', '--', 'proof-command']):
            with patch.dict(os.environ, {'GITHUB_ACTIONS': 'true'}):
                with patch.object(supervisor.platform, 'system', return_value='Windows'):
                    with patch.object(supervisor, 'registered_checkouts', return_value=[]):
                        with patch.object(supervisor.subprocess, 'Popen', return_value=process):
                            with patch.object(supervisor, 'recover', return_value=recovered):
                                with patch.object(supervisor, 'invalidate_evidence'):
                                    with self.assertRaises(SystemExit) as result:
                                        supervisor.main()
        return result.exception.code

    def test_normal_worker_success_remains_success(self):
        self.assertEqual(self.execute(Mock(wait=Mock(return_value=0)), False), 0)

    def test_recovery_invalidates_zero_worker_exit(self):
        process = Mock(wait=Mock(return_value=0))
        self.assertEqual(self.execute(process, True), 125)

    def test_supervisor_timeout_kills_owned_tree_and_fails_after_recovery(self):
        process = Mock(pid=12345, poll=Mock(return_value=None),
                       wait=Mock(side_effect=[subprocess.TimeoutExpired('proof', 1800), None]))
        with patch.object(supervisor.subprocess, 'run') as kill:
            self.assertEqual(self.execute(process, True), 124)
        self.assertEqual(kill.call_args.args[0], ['taskkill', '/PID', '12345', '/T', '/F'])
        process.kill.assert_called_once()

    def test_recovered_report_cannot_be_accepted_as_passed_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            path = directory / 'platform.json'
            path.write_text(json.dumps({'status': 'passed', 'source_commit': 'retained-source'}))
            supervisor.invalidate_evidence(directory, 124)
            report = json.loads(path.read_text())
            self.assertEqual(report['status'], 'failed')
            self.assertEqual(report['source_commit'], 'retained-source')
            self.assertEqual(report['windows_supervisor_exit'], 124)


if __name__ == '__main__':
    unittest.main()
