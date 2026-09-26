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
from _python_distribution import (DistributionError, extract_sdist, inspect_wheel, digest,
                                  source_python_contract, validate_requires_python, verify_source)


class DistributionTests(unittest.TestCase):
    def test_timeout_kills_descendants_that_hold_output_pipes(self):
        import os
        import time
        from _python_sandbox import bounded_command
        with tempfile.TemporaryDirectory() as temporary:
            started = time.monotonic()
            with self.assertRaisesRegex(DistributionError, 'exceeded.*child launched'):
                bounded_command([sys.executable, '-u', '-c',
                    'import subprocess,sys,time; '
                    'subprocess.Popen([sys.executable,"-c","import time; time.sleep(30)"]); '
                    'print("child launched",flush=True); time.sleep(30)'],
                    Path(temporary), dict(os.environ), timeout=0.5)
            self.assertLess(time.monotonic() - started, 10)

    def test_tracked_source_copy_excludes_generated_caches_without_removing_them(self):
        import subprocess
        from prepare_python_distributions import copy_tracked_tree
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'checkout'; source.mkdir()
            subprocess.run(['git', 'init', '-q', str(source)], check=True)
            for relative in ('python/package', 'embedding'):
                directory = source / relative; directory.mkdir(parents=True)
                (directory / 'source.py').write_text('VALUE = 1\n')
                subprocess.run(['git', 'add', relative + '/source.py'], cwd=source, check=True)
                (directory / '__pycache__').mkdir()
                cached = directory / '__pycache__/source.cpython-310.pyc'
                cached.write_bytes(b'generated cache')
                (directory / 'source.pyo').write_bytes(b'generated optimized cache')
                destination = root / ('copied-' + directory.name)
                copy_tracked_tree(source, Path(relative), destination)
                self.assertEqual([path.name for path in destination.iterdir()], ['source.py'])
                self.assertEqual(cached.read_bytes(), b'generated cache')

    def test_qualification_helper_staging_closure_supports_isolated_imports(self):
        import shutil
        import subprocess
        from prepare_python_distributions import QUALIFICATION_HELPERS
        source = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory() as temporary:
            qualification = Path(temporary) / 'qualification'
            qualification.mkdir()
            for filename in QUALIFICATION_HELPERS:
                shutil.copyfile(source / filename, qualification / filename)
            result = subprocess.run(
                [sys.executable, '-I', '-c',
                 'import sys; sys.path.insert(0, sys.argv[1]); '
                 'import _python_distribution, build_binding_source_bundle; '
                 'print("QUALIFICATION_HELPER_IMPORTS_PASSED")', str(qualification)],
                capture_output=True, text=True, check=True,
            )
            self.assertIn('QUALIFICATION_HELPER_IMPORTS_PASSED', result.stdout)

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
        for key in ('warnings_as_errors', 'asyncio_debug', 'embedding_in_each_cell'):
            with self.subTest(key=key), self.assertRaises(DistributionError):
                runtime_options({key: 'false'})

    def test_binary_architecture_cannot_be_overridden_by_filename(self):
        from _python_distribution import verify_native_architecture
        from test_python_arm64 import pe
        arm = b'\xcf\xfa\xed\xfe' + (0x100000c).to_bytes(4, 'little')
        verify_native_architecture(arm, 'macosx_11_0_arm64')
        verify_native_architecture(pe(0xAA64), 'win_arm64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(arm, 'macosx_10_13_x86_64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(b'MZ', 'win_amd64')

    def test_requires_python_is_open_ended_and_has_the_abi3_floor(self):
        self.assertEqual(validate_requires_python('>=3.10'), '>=3.10')
        for value in ('>=3.10,<3.13', '>=3.10,!=3.12', '>=3.11', '==3.10'):
            with self.subTest(value=value), self.assertRaises(DistributionError):
                validate_requires_python(value)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'pyproject.toml').write_text(
                '[project]\nrequires-python = ">=3.10"\n'
                '[tool.maturin]\nfeatures = ["pyo3/abi3-py310"]\n')
            (root / 'Cargo.toml').write_text(
                '[dependencies]\npyo3 = { version = "0.29.2", features = [] }\n')
            with self.assertRaisesRegex(DistributionError, 'abi3-py310'):
                source_python_contract(root)

    def test_wheel_metadata_is_parsed_and_compared_to_source(self):
        from test_python_arm64 import pe
        with tempfile.TemporaryDirectory() as temporary:
            wheel = Path(temporary) / 'sc_observability-1.4.0-cp310-abi3-win_arm64.whl'
            members = {
                'sc_observability/__init__.py': '',
                'sc_observability/py.typed': '',
                'sc_observability/generated/__init__.py': '',
                'sc_observability/generated/__init__.pyi': '',
                'sc_observability/_native.pyd': pe(0xAA64),
                'sc_observability-1.4.0.dist-info/WHEEL':
                    'Wheel-Version: 1.0\nTag: cp310-abi3-win_arm64\n',
                'sc_observability-1.4.0.dist-info/METADATA':
                    'Metadata-Version: 2.1\nName: sc-observability\nVersion: 1.4.0\nRequires-Python: >=3.10\n',
            }
            with zipfile.ZipFile(wheel, 'w') as archive:
                for name, content in members.items():
                    archive.writestr(name, content)
            policy = {'wheel_platform': 'win_arm64', 'expected_requires_python': '>=3.10'}
            self.assertEqual(inspect_wheel(wheel, policy, '1.4.0')['expected_requires_python'], '>=3.10')
            with zipfile.ZipFile(wheel, 'w') as archive:
                for name, content in members.items():
                    if isinstance(content, str):
                        content = content.replace('Requires-Python: >=3.10',
                                                  'Requires-Python: >=3.10,<3.13')
                    archive.writestr(name, content)
            with self.assertRaisesRegex(DistributionError, 'Requires-Python'):
                inspect_wheel(wheel, policy, '1.4.0')

    def test_instrumented_wheel_cannot_enter_publication_inventory(self):
        from _python_distribution import release_wheel, fault_paths
        production = {'role': 'production', 'publication': 'pending_B.7', 'maturin_features': ['pyo3/abi3-py310']}
        self.assertEqual(release_wheel(production), production)
        for changed in ({'role': 'instrumented', 'publication': 'never'}, {'maturin_features': ['test-hooks']}):
            with self.subTest(changed=changed), self.assertRaisesRegex(DistributionError, 'production release'):
                release_wheel({**production, **changed})
        with self.assertRaises(DistributionError):
            fault_paths({'fault_pytest_paths': ['tests/']})

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
                    path.write_text({
                        'Cargo.toml': '[dependencies]\npyo3 = { version = "0.29.2", features = ["abi3-py310"] }\n',
                        'pyproject.toml': '[project]\nrequires-python = ">=3.10"\n[tool.maturin]\nfeatures = ["pyo3/abi3-py310"]\n',
                    }.get(relative, 'fixture'))
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

    def test_aggregate_requires_opted_in_host_execution_on_the_cell_interpreter(self):
        from validate_python_distribution import aggregate
        policy_path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(policy_path.read_text())
        required = ('Cargo.toml', 'Cargo.lock', '.cargo/config.toml', 'pyproject.toml',
                    'python/sc_observability/__init__.py', 'python/sc_observability/generated/__init__.pyi',
                    'python/sc_observability/py.typed', 'rust-bundle/manifest.json')
        for mutation in ('missing-host', 'wrong-interpreter', 'missing-command'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                source = root / 'source'; source.mkdir()
                for relative in required:
                    path = source / relative; path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text({
                        'Cargo.toml': '[dependencies]\npyo3 = { version = "0.29.2", features = ["abi3-py310"] }\n',
                        'pyproject.toml': '[project]\nrequires-python = ">=3.10"\n[tool.maturin]\nfeatures = ["pyo3/abi3-py310"]\n',
                    }.get(relative, 'fixture'))
                contract = {'schema_version': 1, 'runtime_complete': True, 'embedding_in_each_cell': True}
                manifest = {'schema_version': 1, 'publication': 'pending_B.7', 'source_commit': 'a' * 40,
                            'runtime_suite': contract, 'files': {path: digest(source / path) for path in required}}
                (source / 'distribution-manifest.json').write_text(json.dumps(manifest))
                sdist = root / 'fixture.tar.gz'
                with tarfile.open(sdist, 'w:gz') as archive:
                    archive.add(source, arcname='source')
                common = {'status': 'passed', 'source_commit': 'a' * 40, 'sdist_sha256': digest(sdist),
                          'isolation': {'checkout': True, 'cargo_cache': True, 'network': True},
                          'wheel': {'sha256': 'fixture'}}
                for index, platform in enumerate(policy['platforms']):
                    directory = root / f'build-{index}'; directory.mkdir()
                    (directory / 'build-result.json').write_text(json.dumps({**common, 'platform': platform['id']}))
                    for count, version in enumerate(policy['interpreters']):
                        directory = root / f'cell-{index}-{count}'; directory.mkdir()
                        record = {**common, 'platform': platform['id'], 'python': version, 'python_full': version,
                                  'runtime_suite': contract, 'typecheck': 'passed', 'commands': []}
                        if mutation != 'missing-host':
                            record['embedding'] = {'status': 'passed', 'python': version,
                                'python_full': 'wrong' if mutation == 'wrong-interpreter' else version}
                        (directory / 'cell-result.json').write_text(json.dumps(record))
                with self.assertRaisesRegex(DistributionError, 'six builds|interpreter-matched embedded-host'):
                    aggregate(Namespace(policy=policy_path, evidence=root, sdist=sdist, source_commit='a' * 40))

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
