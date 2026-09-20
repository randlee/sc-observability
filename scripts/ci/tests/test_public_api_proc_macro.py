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
    def check(self, stdout, exit_code=0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'release').mkdir()
            (root / 'release/public-api-policy.json').write_text(json.dumps({
                'candidate_version': '1.4.0', 'crates': {'example-macros': {
                    'baseline_version': '1.4.0', 'kind': 'proc-macro'}}}))
            commands = []
            def run(command):
                commands.append(command)
                if command[:2] == ['cargo', 'metadata']:
                    output = json.dumps({'packages': [{'name': 'example-macros',
                        'version': '1.4.0', 'publish': None, 'manifest_path': 'Cargo.toml',
                        'targets': [{'kind': ['proc-macro']}]}]})
                    return subprocess.CompletedProcess(command, 0, output, '')
                if command[0] == 'git':
                    return subprocess.CompletedProcess(command, 0, 'head', '')
                return subprocess.CompletedProcess(command, exit_code, stdout, '')
            with patch.object(gate, 'ROOT', root), patch.object(gate, 'CACHE', root / 'cache'), patch.object(gate, 'run', run), patch.object(sys, 'argv', ['gate', 'semver']), contextlib.redirect_stdout(io.StringIO()):
                result = gate.main()
            self.assertIn(['cargo', 'public-api', '--manifest-path', 'Cargo.toml', '-sss', 'diff', '1.4.0'], commands)
            self.assertFalse(any('semver-checks' in c for c in commands))
            return result

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
