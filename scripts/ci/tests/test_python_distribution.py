"""Boundary failures must be detected before executing artifact contents."""
import io
import json
import sys
import tarfile
import tempfile
import unittest
import zipfile
from argparse import Namespace
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

    def test_debug_contract_reaches_isolated_python_and_rejects_invalid_values(self):
        import subprocess
        from _python_distribution import runtime_options
        flags, environment = runtime_options({'asyncio_debug': True, 'warnings_as_errors': True})
        proof = subprocess.run([sys.executable, *flags, '-c',
            'import asyncio,warnings; loop=asyncio.new_event_loop(); '
            'assert loop.get_debug(); loop.close(); '
            'warnings.warn("qualification warning", RuntimeWarning)'], capture_output=True, text=True)
        self.assertNotEqual(proof.returncode, 0)
        self.assertIn('RuntimeWarning: qualification warning', proof.stderr)
        self.assertEqual(environment, {'PYTHONASYNCIODEBUG': '1', 'PYTHONWARNINGS': 'error'})
        with self.assertRaises(DistributionError):
            runtime_options({'warnings_as_errors': 'false'})

    def test_binary_architecture_cannot_be_overridden_by_filename(self):
        from _python_distribution import verify_native_architecture
        arm = b'\xcf\xfa\xed\xfe' + (0x100000c).to_bytes(4, 'little')
        verify_native_architecture(arm, 'macosx_11_0_arm64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(arm, 'macosx_10_13_x86_64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(b'MZ', 'win_amd64')

    def test_policy_preserves_all_twenty_five_cells(self):
        path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(path.read_text())
        self.assertEqual(policy['interpreters'], ['3.10', '3.11', '3.12', '3.13', '3.14'])
        self.assertEqual({p['id'] for p in policy['platforms']},
                         {'macos-arm64', 'macos-x86_64', 'linux-x86_64', 'linux-aarch64', 'windows-x86_64'})
        self.assertEqual(len(policy['interpreters']) * len(policy['platforms']), 25)

    def test_frozen_inventory_rejects_tampering_missing_lock_and_extra_files(self):
        required = ('Cargo.toml', 'Cargo.lock', '.cargo/config.toml', 'pyproject.toml',
                    'python/sc_observability/__init__.py', 'python/sc_observability/generated/__init__.pyi',
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

    def test_aggregate_rejects_missing_duplicate_and_mixed_source_cells(self):
        from validate_python_distribution import aggregate
        policy_path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(policy_path.read_text())
        for mutation in ('missing', 'duplicate', 'mixed-source'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                sdist = root / 'fixture.tar.gz'
                sdist.write_bytes(b'fixture')
                common = {'status': 'passed', 'source_commit': 'a' * 40,
                          'sdist_sha256': digest(sdist),
                          'isolation': {'checkout': True, 'cargo_cache': True, 'network': True}}
                for index, platform in enumerate(policy['platforms']):
                    directory = root / f'build-{index}'; directory.mkdir()
                    record = {**common, 'platform': platform['id'], 'wheel': {'sha256': 'fixture'}}
                    if mutation == 'mixed-source' and index == 0:
                        record['source_commit'] = 'b' * 40
                    (directory / 'build-result.json').write_text(json.dumps(record))
                    for count, version in enumerate(policy['interpreters']):
                        if mutation == 'missing' and index == 0 and count == 0:
                            continue
                        directory = root / f'cell-{index}-{count}'; directory.mkdir()
                        record = {**common, 'platform': platform['id'], 'python': version}
                        if mutation == 'duplicate' and index == 0 and count == 0:
                            record['python'] = '3.11'
                        (directory / 'cell-result.json').write_text(json.dumps(record))
                with self.assertRaises(DistributionError):
                    aggregate(Namespace(policy=policy_path, evidence=root, sdist=sdist, source_commit='a' * 40))
