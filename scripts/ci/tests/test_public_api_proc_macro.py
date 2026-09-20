"""Published proc-macro APIs must be checked, never silently exempted."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import validate_public_api as gate


class PublishedProcMacroTests(unittest.TestCase):
    def check(self, stdout, exit_code=0, kind="proc-macro", baseline="1.4.0"):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'release').mkdir()
            (root / 'release/public-api-policy.json').write_text(json.dumps({
                'candidate_version': '1.4.0', 'crates': {'example-macros': {
                    'baseline_version': baseline, 'kind': kind}}}))
            commands = []
            def run(command):
                commands.append(command)
                if command[:2] == ['cargo', 'metadata']:
                    output = json.dumps({'packages': [{'name': 'example-macros',
                        'version': '1.4.0', 'publish': None, 'manifest_path': 'Cargo.toml',
                        'targets': [{'kind': [kind]}]}]})
                    return subprocess.CompletedProcess(command, 0, output, '')
                if command[0] == 'git':
                    return subprocess.CompletedProcess(command, 0, 'head', '')
                return subprocess.CompletedProcess(command, exit_code, stdout, '')
            with patch.object(gate, 'ROOT', root), patch.object(gate, 'CACHE', root / 'cache'), patch.object(gate, 'run', run), patch.object(sys, 'argv', ['gate', 'semver']), contextlib.redirect_stdout(io.StringIO()):
                result = gate.main()
            if kind == 'proc-macro':
                self.assertIn(['cargo', 'public-api', '--manifest-path', 'Cargo.toml', '-sss', 'diff', baseline], commands)
                self.assertFalse(any('semver-checks' in c for c in commands))
            else:
                self.assertIn(['cargo', 'semver-checks', '--manifest-path', 'Cargo.toml', '--baseline-version', baseline, '--release-type', 'patch' if baseline == '1.4.0' else 'minor', '--default-features'], commands)
            return result

    def test_same_version_library_uses_stricter_patch_checks(self):
        self.assertEqual(self.check('checked', kind='lib'), 0)

    def test_historical_library_retains_minor_checks(self):
        self.assertEqual(self.check('checked', kind='lib', baseline='1.2.0'), 0)

    def output(self, removal='(none)'):
        return f'Removed items from the public API\n{removal}\nChanged items in the public API\n(none)\nAdded items to the public API\n(none)\n'

    def test_identical_published_surface_passes(self):
        self.assertEqual(self.check(self.output()), 0)

    def test_removed_macro_fails(self):
        self.assertEqual(self.check(self.output('-pub macro removed!')), 2)

    def test_failed_tool_cannot_pass(self):
        self.assertEqual(self.check(self.output(), 1), 2)

    def test_empty_success_cannot_pass(self):
        self.assertEqual(self.check(''), 2)


if __name__ == '__main__':
    unittest.main()
