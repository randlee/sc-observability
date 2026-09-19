"""Reject altered source-install receipts before accepting qualification evidence."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('receipt_verifier', Path(__file__).parents[1] / 'verify_sc_lint_source_receipt.py')
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)


class ReceiptTests(unittest.TestCase):
    def fixture(self, root, windows):
        suffix = '.exe' if windows else ''
        binary = root / 'bin'
        binary.mkdir()
        hashes = {}
        for name in ('sc-lint', 'sc-lint-boundary', 'sc-lint-runtime', 'sc-lint-portability'):
            p = binary / (name + suffix)
            p.write_bytes(name.encode())
            hashes[p.name] = hashlib.sha256(p.read_bytes()).hexdigest()
        wheel = root / 'built.whl'
        wheel.write_bytes(b'wheel')
        python = root / 'venv' / ('Scripts' if windows else 'bin') / ('python' + suffix)
        python.parent.mkdir(parents=True)
        python.write_bytes(b'python')
        receipt = dict(source_revision='a' * 40, repository='randlee/sc-lint', version='0.6.0',
                       binaries=hashes, binary_directory=str(binary), wheel=str(wheel),
                       wheel_sha256=hashlib.sha256(wheel.read_bytes()).hexdigest(), python=str(python))
        return receipt, binary / ('sc-lint' + suffix)

    def run_case(self, mutation=None, expected=None, windows=False):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            receipt, cli = self.fixture(root, windows)
            python_version = '0.6.0'
            if mutation:
                receipt, cli, python_version = mutation(receipt, cli, root)
            def probe(args, **kwargs):
                if args[1] == 'version':
                    value = {'ok': True, 'data': {'version': '0.6.0', 'status': 'pass'}}
                else:
                    value = {'version': python_version, 'path': str(root / 'venv/lib/sc_lint/__init__.py')}
                return subprocess.CompletedProcess(args, 0, json.dumps(value), '')
            with patch.object(VERIFIER.subprocess, 'run', side_effect=probe):
                if expected:
                    with self.assertRaisesRegex(SystemExit, expected):
                        VERIFIER.verify_receipt(receipt, 'a' * 40, cli, windows=windows)
                else:
                    VERIFIER.verify_receipt(receipt, 'a' * 40, cli, windows=windows)

    def test_actual_unix_and_windows_receipt_shapes(self):
        for windows in (False, True):
            with self.subTest(windows=windows):
                self.run_case(windows=windows)

    def test_tampered_binary_bytes(self):
        def alter(r, cli, root):
            cli.write_bytes(b'tampered')
            return r, cli, '0.6.0'
        self.run_case(alter, 'binary digest does not match')

    def test_tampered_wheel_bytes(self):
        def alter(r, cli, root):
            Path(r['wheel']).write_bytes(b'tampered')
            return r, cli, '0.6.0'
        self.run_case(alter, 'wheel digest does not match')

    def test_other_cli_cannot_borrow_receipt(self):
        def alter(r, cli, root):
            other = root / 'other-cli'
            other.write_bytes(cli.read_bytes())
            return r, other, '0.6.0'
        self.run_case(alter, 'does not match the verified receipt CLI')

    def test_python_version_mismatch(self):
        self.run_case(lambda r,c,p: (r,c,'0.4.0'), 'Python package version/path')

    def test_arbitrary_extension_is_not_windows_binary(self):
        def alter(r, cli, root):
            r['binaries'] = {name.replace('.exe', '.junk'): value for name, value in r['binaries'].items()}
            return r, cli, '0.6.0'
        self.run_case(alter, 'sibling binary set', windows=True)

    def test_source_revision_mismatch(self):
        def alter(r, cli, root):
            r['source_revision'] = 'b' * 40
            return r, cli, '0.6.0'
        self.run_case(alter, 'revision mismatch')


if __name__ == '__main__':
    unittest.main()
