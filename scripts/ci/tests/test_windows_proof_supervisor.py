"""Recovery cannot qualify a failed or incompletely cleaned proof."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import supervise_windows_proof as supervisor


class SupervisorTests(unittest.TestCase):
    def execute(self, process, leftover=False, failure=None):
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / 'evidence'
            def launch(command, env):
                if leftover:
                    control = Path(env['SC_WINDOWS_IDENTITY_CONTROL_ROOT']) / 'windows-identity-control-owned'
                    control.mkdir()
                    (control / 'identity-recovery.json').write_text('retained journal')
                return process
            with patch.object(sys, 'argv', ['supervisor', '--evidence', str(evidence), '--', 'proof-command']), \
                 patch.dict(os.environ, {'GITHUB_ACTIONS': 'true'}), \
                 patch.object(supervisor.platform, 'system', return_value='Windows'), \
                 patch.object(supervisor.subprocess, 'Popen', side_effect=launch), \
                 patch.object(supervisor, 'recover', side_effect=failure, return_value=True):
                with self.assertRaises(SystemExit) as result:
                    supervisor.main()
            report = json.loads((evidence / 'windows-supervisor.json').read_text())
            self.assertEqual(report['exit'], result.exception.code)
            if failure:
                self.assertEqual((evidence / 'failed-recovery-1/identity-recovery.json').read_text(), 'retained journal')
                self.assertEqual(report['recovery_errors'], [str(failure)])
            return result.exception.code

    def test_normal_worker_success_remains_success(self):
        self.assertEqual(self.execute(Mock(wait=Mock(return_value=0))), 0)

    def test_leftover_recovery_invalidates_zero_worker_exit(self):
        self.assertEqual(self.execute(Mock(wait=Mock(return_value=0)), leftover=True), 125)

    def test_recovery_error_invalidates_success_and_retains_journal(self):
        self.assertEqual(self.execute(Mock(wait=Mock(return_value=0)), True, OSError('restore failed')), 125)

    def test_supervisor_timeout_kills_worker_tree_and_fails(self):
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


if __name__ == '__main__': unittest.main()
