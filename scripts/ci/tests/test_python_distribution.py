"""Boundary failures must be detected before executing artifact contents."""
import io
import json
import sys
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
import zipfile
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from _python_distribution import (DistributionError, actual_cell, extract_sdist, inspect_wheel, digest,
                                  source_python_contract, validate_requires_python, verify_source)
from python_test_fixtures import pe


class DistributionTests(unittest.TestCase):
    def test_source_python_contract_reads_utf8_independent_of_locale(self):
        import io

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'pyproject.toml').write_text(
                '# café ✓ č\n'
                '[project]\nrequires-python = ">=3.10"\n'
                '[tool.maturin]\nfeatures = ["pyo3/abi3-py310"]\n',
                encoding='utf-8',
            )
            (root / 'Cargo.toml').write_text(
                '# café ✓ č\n'
                '[dependencies]\n'
                'pyo3 = { version = "0.29.2", features = ["abi3-py310"] }\n',
                encoding='utf-8',
            )

            default_open = io.open

            def cp1252_default(*args, **kwargs):
                if len(args) > 3:
                    if args[3] in (None, 'locale'):
                        args = (*args[:3], 'cp1252', *args[4:])
                elif kwargs.get('encoding') in (None, 'locale'):
                    kwargs['encoding'] = 'cp1252'
                return default_open(*args, **kwargs)

            with patch('pathlib.io.open', side_effect=cp1252_default):
                self.assertEqual(source_python_contract(root), '>=3.10')


    def test_actual_cell_rejects_interpreter_outside_matched_platform_policy(self):
        policy = {
            'interpreters': ['3.10', '3.11', '3.12', '3.13', '3.14'],
            'platforms': [{
                'id': 'windows-arm64',
                'interpreters': ['3.11', '3.12', '3.13', '3.14'],
            }],
        }
        with patch('_python_distribution.platform.system', return_value='Windows'), \
                patch('_python_distribution.platform.machine', return_value='ARM64'), \
                patch('_python_distribution.platform.python_implementation', return_value='CPython'), \
                patch('_python_distribution.platform.platform', return_value='Windows-ARM64'), \
                patch('_python_distribution.sys.version_info', SimpleNamespace(major=3, minor=10)), \
                patch('_python_distribution.sysconfig.get_config_var', return_value=None), \
                self.assertRaisesRegex(DistributionError, 'unsupported interpreter'):
            actual_cell(policy)

    def test_actual_cell_falls_back_to_global_policy_without_platform_override(self):
        policy = {
            'interpreters': ['3.10'],
            'platforms': [{'id': 'windows-arm64'}],
        }
        with patch('_python_distribution.platform.system', return_value='Windows'), \
                patch('_python_distribution.platform.machine', return_value='ARM64'), \
                patch('_python_distribution.platform.python_implementation', return_value='CPython'), \
                patch('_python_distribution.platform.platform', return_value='Windows-ARM64'), \
                patch('_python_distribution.sys.version_info', SimpleNamespace(major=3, minor=10)), \
                patch('_python_distribution.sysconfig.get_config_var', return_value=None):
            actual = actual_cell(policy)
        self.assertEqual(actual['platform'], 'windows-arm64')
        self.assertEqual(actual['python'], '3.10')

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

    def test_relocated_conformance_corpus_is_an_exact_source_input(self):
        from stage_python_conformance import stage_conformance
        source = Path(__file__).resolve().parents[3]
        corpus = source / 'bindings/conformance/v1/conversion-cases.json'
        with tempfile.TemporaryDirectory() as temporary:
            tests = Path(temporary) / 'tests'
            staged = stage_conformance(source, tests)
            self.assertEqual(staged.relative_to(tests).as_posix(),
                             'conformance/v1/conversion-cases.json')
            self.assertEqual(staged.read_bytes(), corpus.read_bytes())
            cases = json.loads(staged.read_text(encoding='utf-8'))
            self.assertTrue(any(case.get('operation') == 'canonical_envelope' for case in cases))
            with self.assertRaises(FileNotFoundError):
                stage_conformance(Path(temporary) / 'missing-source', tests)

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
                 'import _python_distribution, build_binding_source_bundle, python_binding_validator, validate_python_distribution; '
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
                inspect_wheel(wheel, {'wheel_platform': 'win_amd64'}, '1.4.0', '>=3.10')
            with self.assertRaisesRegex(DistributionError, 'wrong wheel ABI/platform'):
                inspect_wheel(wheel, {'wheel_platform': 'manylinux_2_28_x86_64'}, '1.4.0', '>=3.10')
            cp311 = Path(temporary) / 'sc_observability-1.4.0-cp311-abi3-win_amd64.whl'
            cp311.write_bytes(wheel.read_bytes())
            with self.assertRaisesRegex(DistributionError, 'wrong wheel ABI/platform'):
                inspect_wheel(cp311, {'wheel_platform': 'win_amd64'}, '1.4.0', '>=3.10')
            non_abi3 = Path(temporary) / 'sc_observability-1.4.0-cp310-cp310-win_amd64.whl'
            non_abi3.write_bytes(wheel.read_bytes())
            with self.assertRaisesRegex(DistributionError, 'wrong wheel ABI/platform'):
                inspect_wheel(non_abi3, {'wheel_platform': 'win_amd64'}, '1.4.0', '>=3.10')

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
        arm = b'\xcf\xfa\xed\xfe' + (0x100000c).to_bytes(4, 'little')
        verify_native_architecture(arm, 'macosx_11_0_arm64')
        verify_native_architecture(pe(0x8664), 'win_amd64')
        verify_native_architecture(pe(0xAA64), 'win_arm64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(arm, 'macosx_10_13_x86_64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(b'MZ', 'win_amd64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(pe(0x8664), 'win_arm64')
        with self.assertRaisesRegex(DistributionError, 'architecture'):
            verify_native_architecture(pe(0xAA64), 'win_amd64')

    def test_verify_native_architecture_uses_pe_helper_once(self):
        from unittest.mock import patch
        from _python_distribution import verify_native_architecture
        with patch('_python_distribution.pe_machine', return_value=0xAA64) as helper:
            verify_native_architecture(b'fixture', 'win_arm64')
        helper.assert_called_once_with(b'fixture')

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
            (root / 'pyproject.toml').write_text(
                '[project]\nrequires-python = ">=3.10,<3.13"\n'
                '[tool.maturin]\nfeatures = ["pyo3/abi3-py310"]\n')
            (root / 'Cargo.toml').write_text(
                '[dependencies]\npyo3 = { version = "0.29.2", features = ["abi3-py310"] }\n')
            with self.assertRaisesRegex(DistributionError, 'open-ended'):
                source_python_contract(root)
            (root / 'pyproject.toml').write_text(
                '[project]\nrequires-python = ">=3.10"\n[tool.maturin]\nfeatures = []\n')
            with self.assertRaisesRegex(DistributionError, 'abi3-py310'):
                source_python_contract(root)

    def test_wheel_metadata_is_parsed_and_compared_to_source(self):
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
            self.assertEqual(inspect_wheel(wheel, policy, '1.4.0', '>=3.10')['wheel_requires_python'], '>=3.10')
            with zipfile.ZipFile(wheel, 'w') as archive:
                for name, content in members.items():
                    if isinstance(content, str):
                        content = content.replace('Requires-Python: >=3.10',
                                                  'Requires-Python: >=3.10,<3.13')
                    archive.writestr(name, content)
            with self.assertRaisesRegex(DistributionError, 'Requires-Python'):
                inspect_wheel(wheel, policy, '1.4.0', '>=3.10')

    def test_instrumented_wheel_cannot_enter_publication_inventory(self):
        from _python_distribution import release_wheel, fault_paths
        production = {'role': 'production', 'publication': 'pending_B.7', 'maturin_features': ['pyo3/abi3-py310']}
        self.assertEqual(release_wheel(production), production)
        for changed in ({'role': 'instrumented', 'publication': 'never'}, {'maturin_features': ['test-hooks']}):
            with self.subTest(changed=changed), self.assertRaisesRegex(DistributionError, 'production release'):
                release_wheel({**production, **changed})
        with self.assertRaises(DistributionError):
            fault_paths({'fault_pytest_paths': ['tests/']})

    def test_policy_has_six_platforms_and_29_native_cells(self):
        path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(path.read_text(encoding='utf-8'))
        self.assertEqual(policy['interpreters'], ['3.10', '3.11', '3.12', '3.13', '3.14'])
        self.assertEqual({p['id'] for p in policy['platforms']},
                         {'macos-arm64', 'macos-x86_64', 'linux-x86_64', 'linux-aarch64',
                          'windows-x86_64', 'windows-arm64'})
        self.assertEqual(len(policy['platforms']), 6)
        arm64 = next(p for p in policy['platforms'] if p['id'] == 'windows-arm64')
        self.assertEqual(arm64['interpreters'], ['3.11', '3.12', '3.13', '3.14'])
        self.assertEqual(arm64['build_python'], '3.11')
        self.assertEqual(arm64['native_cells'], 4)
        self.assertEqual(policy['native_installed_suite_cells'], 29)
        for platform in policy['platforms']:
            if platform['id'] != 'windows-arm64':
                self.assertEqual(platform.get('interpreters', policy['interpreters']),
                                 policy['interpreters'])
        self.assertEqual(sum(len(p.get('interpreters', policy['interpreters'])) for p in policy['platforms']), 29)

    def _six_platform_aggregate_fixture(self, root: Path):
        policy = json.loads((Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json').read_text())
        policy_path = root / 'policy.json'
        policy_path.write_text(json.dumps(policy))
        sdist = root / 'fixture.tar.gz'
        sdist.write_bytes(b'fixture')
        source = root / 'source'
        source.mkdir()
        (source / 'pyproject.toml').write_text(
            '[tool.maturin]\nfeatures = ["pyo3/abi3-py310"]\n')
        common = {'status': 'passed', 'source_commit': 'a' * 40,
                  'sdist_sha256': digest(sdist), 'expected_requires_python': '>=3.10',
                  'isolation': {'checkout': True, 'cargo_cache': True, 'network': True}}
        for index, platform in enumerate(policy['platforms']):
            build_dir = root / f'build-{index}'
            build_dir.mkdir()
            wheel_path = build_dir / f'wheel-{index}.whl'
            wheel_path.write_bytes(f'wheel-{index}'.encode())
            wheel = {'sha256': digest(wheel_path), 'wheel': wheel_path.name,
                     'role': 'production', 'publication': 'pending_B.7',
                     'maturin_features': ['pyo3/abi3-py310']}
            build = {**common, 'platform': platform['id'], 'wheel': wheel,
                     'embedding': 'passed', 'native_runtime_tests': 'passed',
                     'negative_results': {str(number): 'passed' for number in range(9)}}
            (build_dir / 'build-result.json').write_text(json.dumps(build))
            for count, version in enumerate(platform.get('interpreters', policy['interpreters'])):
                cell_dir = root / f'cell-{index}-{count}'
                cell_dir.mkdir()
                cell = {**common, 'platform': platform['id'], 'python': version,
                        'python_full': version,
                        'runtime_suite': {'runtime_complete': True, 'embedding_in_each_cell': False},
                        'wheel': {'sha256': wheel['sha256']}, 'typecheck': 'passed',
                        'production_hooks_absent': True, 'test_count': 1,
                        'test_cases': ['fixture::passes'],
                        'commands': [{'command': ['python', '-m', 'pytest'], 'exit_code': 0},
                                     {'command': ['python', '-m', 'mypy'], 'exit_code': 0}]}
                (cell_dir / 'cell-result.json').write_text(json.dumps(cell))
                (cell_dir / 'runtime.xml').write_text('<testsuite><testcase classname="fixture" name="passes" /></testsuite>')
        return policy_path, sdist, source

    def _aggregate_source_patches(self, source: Path):
        return (patch('validate_python_distribution.extract_sdist', return_value=source),
                patch('validate_python_distribution.verify_source', return_value={
                    'runtime_suite': {'runtime_complete': True, 'embedding_in_each_cell': False},
                    'source_commit': 'a' * 40, 'expected_requires_python': '>=3.10'}),
                patch('validate_python_distribution.runtime_options'),
                patch('validate_python_distribution.inspect_wheel',
                      side_effect=lambda wheel, *_: {'sha256': digest(wheel)}))

    def test_aggregate_has_a_valid_six_platform_positive_control(self):
        from validate_python_distribution import aggregate
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            policy, sdist, source = self._six_platform_aggregate_fixture(root)
            self.assertEqual(len(json.loads(policy.read_text())['platforms']), 6)
            self.assertEqual(len(list(root.rglob('build-result.json'))), 6)
            self.assertEqual(len(list(root.rglob('cell-result.json'))), 29)
            patches = self._aggregate_source_patches(source)
            with patches[0], patches[1], patches[2], patches[3]:
                aggregate(Namespace(policy=policy, evidence=root, sdist=sdist, source_commit='a' * 40))

    def test_aggregate_rejects_build_or_cell_requires_python_disagreement(self):
        from validate_python_distribution import aggregate
        for record_name in ('build-result.json', 'cell-result.json'):
            with self.subTest(record_name=record_name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                policy, sdist, source = self._six_platform_aggregate_fixture(root)
                path = next(root.rglob(record_name))
                record = json.loads(path.read_text())
                record['expected_requires_python'] = '>=3.11'
                path.write_text(json.dumps(record))
                patches = self._aggregate_source_patches(source)
                with patches[0], patches[1], patches[2], patches[3]:
                    with self.assertRaisesRegex(DistributionError, 'Requires-Python metadata disagree'):
                        aggregate(Namespace(policy=policy, evidence=root, sdist=sdist, source_commit='a' * 40))

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

    def test_aggregate_rejects_five_platform_policy(self):
        from validate_python_distribution import aggregate
        policy_path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(policy_path.read_text(encoding='utf-8'))
        policy['platforms'] = [p for p in policy['platforms'] if p['id'] != 'windows-arm64']
        self.assertEqual(len(policy['platforms']), 5)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            policy_path = root / 'five-platform-policy.json'
            policy_path.write_text(json.dumps(policy))
            sdist = root / 'fixture.tar.gz'
            sdist.write_bytes(b'fixture')
            with self.assertRaisesRegex(DistributionError, 'exactly six platforms, six builds and 29'):
                aggregate(Namespace(policy=policy_path, evidence=root, sdist=sdist, source_commit='a' * 40))

    def test_aggregate_requires_opted_in_host_execution_on_the_cell_interpreter(self):
        from validate_python_distribution import aggregate
        policy_path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(policy_path.read_text(encoding='utf-8'))
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
                          'expected_requires_python': '>=3.10',
                          'isolation': {'checkout': True, 'cargo_cache': True, 'network': True},
                          'wheel': {'sha256': 'fixture'}}
                for index, platform in enumerate(policy['platforms']):
                    directory = root / f'build-{index}'; directory.mkdir()
                    (directory / 'build-result.json').write_text(json.dumps({**common, 'platform': platform['id']}))
                    for count, version in enumerate(platform.get('interpreters', policy['interpreters'])):
                        directory = root / f'cell-{index}-{count}'; directory.mkdir()
                        record = {**common, 'platform': platform['id'], 'python': version, 'python_full': version,
                                  'runtime_suite': contract, 'typecheck': 'passed', 'commands': []}
                        if mutation != 'missing-host':
                            record['embedding'] = {'status': 'passed', 'python': version,
                                'python_full': 'wrong' if mutation == 'wrong-interpreter' else version}
                        (directory / 'cell-result.json').write_text(json.dumps(record))
                with self.assertRaisesRegex(
                        DistributionError,
                        'missing interpreter-matched embedded-host execution'):
                    aggregate(Namespace(policy=policy_path, evidence=root, sdist=sdist, source_commit='a' * 40))

    def test_aggregate_rejects_missing_duplicate_and_mixed_source_cells(self):
        from validate_python_distribution import aggregate
        policy_path = Path(__file__).resolve().parents[3] / 'release/python-platform-policy.json'
        policy = json.loads(policy_path.read_text(encoding='utf-8'))
        expected_messages = {
            'missing': 'exactly six builds and 29 installed-suite cells are required',
            'duplicate': 'matrix contains missing, duplicate or unsupported cells',
            'wrong-target': 'build records contain missing, duplicate or unsupported platforms',
            'mixed-source': 'mixed source/artifacts or incomplete isolation evidence',
        }
        for mutation in ('missing', 'duplicate', 'wrong-target', 'mixed-source'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                policy_fixture = root / 'policy.json'
                policy_fixture.write_text(json.dumps(policy))
                sdist = root / 'fixture.tar.gz'
                sdist.write_bytes(b'fixture')
                common = {'status': 'passed', 'source_commit': 'a' * 40,
                          'sdist_sha256': digest(sdist),
                          'expected_requires_python': '>=3.10',
                          'isolation': {'checkout': True, 'cargo_cache': True, 'network': True}}
                for index, platform in enumerate(policy['platforms']):
                    directory = root / f'build-{index}'; directory.mkdir()
                    record = {**common, 'platform': platform['id'], 'wheel': {'sha256': 'fixture'}}
                    if mutation == 'mixed-source' and index == 0:
                        record['source_commit'] = 'b' * 40
                    if mutation == 'wrong-target' and index == 0:
                        record['platform'] = 'unsupported-platform'
                    (directory / 'build-result.json').write_text(json.dumps(record))
                    for count, version in enumerate(platform.get('interpreters', policy['interpreters'])):
                        if mutation == 'missing' and index == 0 and count == 0:
                            continue
                        directory = root / f'cell-{index}-{count}'; directory.mkdir()
                        record = {**common, 'platform': platform['id'], 'python': version}
                        if mutation == 'duplicate' and index == 0 and count == 0:
                            record['python'] = '3.11'
                        (directory / 'cell-result.json').write_text(json.dumps(record))
                with self.assertRaisesRegex(DistributionError, expected_messages[mutation]):
                    aggregate(Namespace(policy=policy_fixture, evidence=root, sdist=sdist, source_commit='a' * 40))
