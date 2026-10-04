"""Ordering and failure preservation around the single ordinary Cargo command."""
import contextlib
import io
import json
import os
import re
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from scripts.api import history
with patch.dict(sys.modules, {'history': history}):
    from scripts.api import run_unit_tests as runner


class UnitRunnerTests(unittest.TestCase):
    def run_command(self, build_success, unit_status, api_fails=False):
        class Cargo:
            stdout = ['ordinary test output\n', '[1,2]\n', json.dumps({'reason': 'build-finished', 'success': build_success}) + '\n']
            waited = False
            def wait(self):
                self.waited = True
                return unit_status
        cargo = Cargo()
        def check(*args, **kwargs):
            self.assertTrue(cargo.waited, 'API checker must wait for complete Cargo output and exit')
            if api_fails:
                raise history.ApiError('deliberate mismatch')
            return {'builds_in_check': 0}
        with tempfile.TemporaryDirectory() as directory:
            args = ['runner', '--record', str(Path(directory) / 'record.json'), '--', 'cargo', 'test', '--workspace', '--no-fail-fast']
            with (patch.object(sys, 'argv', args),
                  patch.object(runner, 'source_fingerprint', return_value='current'),
                  patch.object(runner.subprocess, 'check_output', return_value='/compiler\n'),
                  patch.object(runner.subprocess, 'Popen', return_value=cargo) as launch,
                  patch.object(runner, 'collect_artifacts', return_value={}) as collect,
                  patch.object(runner, 'check_current', side_effect=check) as inspect,
                  contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO())):
                status = runner.main()
            launch.assert_called_once()
            self.assertEqual(launch.call_args.args[0], ['cargo', 'test', '--workspace', '--no-fail-fast', '--message-format=json-render-diagnostics'])
            return status, collect.call_count, inspect.call_count

    def test_runtime_failure_does_not_skip_independent_api_result(self):
        self.assertEqual(self.run_command(True, 101), (101, 1, 1))

    def test_incomplete_compilation_never_reads_partial_artifacts(self):
        self.assertEqual(self.run_command(False, 101), (101, 0, 0))

    def test_api_mismatch_fails_successful_normal_unit_command(self):
        self.assertEqual(self.run_command(True, 0, api_fails=True), (1, 1, 1))

    def test_success_runs_only_one_cargo_invocation(self):
        self.assertEqual(self.run_command(True, 0), (0, 1, 1))


class EnvironmentAndWorkflowTests(unittest.TestCase):
    def test_unix_cargo_path_is_unchanged(self):
        for platform in ('linux', 'darwin'):
            with (self.subTest(platform=platform), patch.object(sys, 'platform', platform),
                  patch.dict(os.environ, {'PATH': '/original/proxies', 'OTHER': 'retained'}, clear=True)):
                self.assertEqual(history.compiler_environment('/compiler'), dict(os.environ))

    def test_windows_compiler_dll_directories_are_available(self):
        with (patch.object(sys, 'platform', 'win32'),
              patch.dict(os.environ, {'PATH': 'original', 'OTHER': 'retained'}, clear=True)):
            env = history.compiler_environment('/compiler')
            self.assertEqual(env['PATH'], os.pathsep.join([
                str(Path('/compiler') / 'bin'), str(Path('/compiler') / 'lib'), 'original']))
            self.assertEqual(env['OTHER'], 'retained')

    def test_dispatch_has_one_explicit_windows_test_and_keeps_other_matrices(self):
        workflow = (history.ROOT / '.github/workflows/ci.yml').read_text()
        matrix = re.search(r'os: \$\{\{ fromJSON\((.+)\) \}\}', workflow).group(1)
        top = workflow.split('\n  windows-test:\n', 1)[1]
        top_guard = re.search(r'^    if: (.+)$', top, re.M).group(1)
        def evaluate(expression, event, base):
            expression = expression.replace('github.event_name', repr(event))
            expression = expression.replace('github.base_ref', repr(base))
            expression = expression.replace('github.head_ref', repr('ordinary-branch'))
            return eval(expression.replace('&&', 'and').replace('||', 'or'), {'__builtins__': {}})
        self.assertEqual(json.loads(evaluate(matrix, 'workflow_dispatch', '')), ['ubuntu-latest', 'macos-latest'])
        self.assertTrue(evaluate(top_guard, 'workflow_dispatch', ''))
        for event, base, expected in [
                ('pull_request', 'integrate/phase-e', ['ubuntu-latest']),
                ('pull_request', 'develop', ['ubuntu-latest', 'macos-latest', 'windows-latest']),
                ('pull_request', 'main', ['ubuntu-latest', 'macos-latest', 'windows-latest']),
                ('push', '', ['ubuntu-latest', 'macos-latest', 'windows-latest'])]:
            with self.subTest(event=event, base=base):
                self.assertEqual(json.loads(evaluate(matrix, event, base)), expected)


if __name__ == '__main__':
    unittest.main()
