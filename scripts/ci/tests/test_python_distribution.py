"""Boundary failures must be detected before executing artifact contents."""
import io
import json
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from _python_distribution import DistributionError, extract_sdist, inspect_wheel, digest, verify_source


class DistributionTests(unittest.TestCase):
    def test_rejects_escaping_or_linked_sdist_members(self):
        for name, kind in [('../outside', tarfile.REGTYPE), ('root/link', tarfile.SYMTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with tarfile.open(root / 'bad.tar.gz', 'w:gz') as archive:
                    entry = tarfile.TarInfo(name)
                    entry.type = kind
                    entry.linkname = '/outside'
                    entry.size = 0
                    archive.addfile(entry, io.BytesIO(b''))
                with self.assertRaises(DistributionError):
                    extract_sdist(root / 'bad.tar.gz', root / 'extract')

    def test_wheel_rejects_missing_stubs_and_wrong_platform(self):
        with tempfile.TemporaryDirectory() as temporary:
            wheel = Path(temporary) / 'sc_observability-1.4.0-cp310-abi3-win_amd64.whl'
            with zipfile.ZipFile(wheel, 'w') as archive:
                archive.writestr('sc_observability/__init__.py', '')
            with self.assertRaisesRegex(DistributionError, 'missing package data'):
                inspect_wheel(wheel, {'wheel_platform': 'win_amd64'}, '1.4.0')
            with self.assertRaisesRegex(DistributionError, 'wrong wheel ABI/platform'):
                inspect_wheel(wheel, {'wheel_platform': 'manylinux_2_28_x86_64'}, '1.4.0')

    def test_policy_preserves_all_twenty_five_cells(self):
        path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(path.read_text())
        self.assertEqual(policy['interpreters'], ['3.10', '3.11', '3.12', '3.13', '3.14'])
        self.assertEqual({p['id'] for p in policy['platforms']},
                         {'macos-arm64', 'macos-x86_64', 'linux-x86_64', 'linux-aarch64', 'windows-x86_64'})
        self.assertEqual(len(policy['interpreters']) * len(policy['platforms']), 25)

    def test_frozen_inventory_rejects_tampering_missing_lock_and_extra_files(self):
        required = ('Cargo.toml', 'Cargo.lock', '.cargo/config.toml', 'pyproject.toml',
                    'python/sc_observability/__init__.py', 'python/sc_observability/__init__.pyi',
                    'python/sc_observability/py.typed', 'rust-bundle/manifest.json')
        for mutation in ('tamper', 'missing', 'extra'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                for relative in required:
                    path = root / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text('fixture')
                record = {'schema_version': 1, 'publication': 'pending_B.7', 'source_commit': 'a' * 40,
                          'files': {path: digest(root / path) for path in required}}
                (root / 'distribution-manifest.json').write_text(json.dumps(record))
                verify_source(root)
                if mutation == 'tamper':
                    (root / 'Cargo.lock').write_text('stale lock')
                elif mutation == 'missing':
                    (root / 'Cargo.lock').unlink()
                else:
                    (root / 'unrecorded').write_text('extra')
                with self.assertRaises(DistributionError):
                    verify_source(root)
