#!/usr/bin/env python3
"""Build and execute immutable Python distributions outside every checkout."""
from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

from _python_distribution import (DistributionError, actual_cell, confined, digest,
                                  extract_sdist, inspect_wheel, verify_source, runtime_options, fault_paths, release_wheel, tomllib)
from _python_sandbox import Sandbox, registered_checkouts


def execute(command: list[str], cwd: Path | None = None) -> None:
    subprocess.run(command, cwd=cwd, check=True)


def policy_at(root: Path) -> dict:
    return json.loads((root / 'qualification/platform-policy.json').read_text())


def verify_resolution(metadata: dict, artifact: Path) -> list[dict]:
    result = []
    for package in metadata['packages']:
        path = Path(package['manifest_path']).resolve()
        if not path.is_relative_to(artifact.resolve()):
            raise DistributionError(f'ambient dependency resolution: {path}')
        result.append({key: package[key] for key in ('name', 'version', 'source', 'manifest_path')})
    return result


def linkage(wheel: Path, policy: dict, sandbox: Sandbox, directory: Path) -> dict:
    details = inspect_wheel(wheel, policy, policy.get('candidate_version', '1.4.0'))
    with zipfile.ZipFile(wheel) as archive:
        native = directory / Path(details['native_member']).name
        native.write_bytes(archive.read(details['native_member']))
    if platform.system() == 'Linux':
        output = sandbox.run([sys.executable, '-m', 'auditwheel', 'show', str(wheel)], directory)
        # Maturin's auditwheel repair enforces symbol floors; retain its independent inspection.
        versions = [tuple(map(int, match.split('.'))) for match in re.findall(r'GLIBC_(\d+\.\d+)', output)]
        if not versions or max(versions) > (2, 28) or 'libpython' in output:
            raise DistributionError('wheel exceeds manylinux_2_28 or links libpython')
    elif platform.system() == 'Darwin':
        output = sandbox.run(['otool', '-L', str(native)], directory)
        if 'Python.framework' in output or 'libpython' in output:
            raise DistributionError('extension links an interpreter-specific Python library')
        load = sandbox.run(['otool', '-l', str(native)], directory)
        # The wheel tag is exact; inspect the actual LC_BUILD_VERSION/LC_VERSION_MIN command.
        deployment = re.findall(r'(?:LC_BUILD_VERSION[\s\S]*?minos|LC_VERSION_MIN_MACOSX[\s\S]*?version)\s+(\d+\.\d+)', load)
        if not deployment or any(tuple(map(int, item.split('.'))) > tuple(map(int, policy['deployment_target'].split('.'))) for item in deployment):
            raise DistributionError('native binary exceeds declared macOS deployment target')
        output += '\n' + load
    else:
        import pefile
        image = pefile.PE(str(native))
        try:
            dependencies = [entry.dll.decode() for entry in image.DIRECTORY_ENTRY_IMPORT]
        finally:
            image.close()
        if any(re.fullmatch(r'python\d{2,}\.dll', name.lower()) for name in dependencies):
            raise DistributionError('abi3 wheel links interpreter-specific Python DLL')
        output = '\n'.join(dependencies)
    return {**details, 'linked_libraries': output}


def verify_embedding_features(metadata: dict) -> None:
    if any('extension-module' in node['features'] for node in metadata['resolve']['nodes']):
        raise DistributionError('extension-only features leaked into embedding link settings')


def negative_cases(root: Path, scratch: Path, sandbox: Sandbox, metadata: dict) -> dict:
    bundle = json.loads((root / 'rust-bundle/manifest.json').read_text())
    results = {}
    by_id = {package['id']: package for package in metadata['packages']}
    target_ids = {dependency['pkg'] for node in metadata['resolve']['nodes']
                  for dependency in node['deps']
                  if any(kind.get('target') for kind in dependency['dep_kinds'])}
    target_registry = [package for identity, package in by_id.items()
                       if identity in target_ids and package.get('source')]
    registry = [package for package in by_id.values() if package.get('source')]
    if not target_registry or not registry:
        raise DistributionError('target-dependent locked registry proof is missing')
    dependency = next(entry for entry in bundle['packages']
                      if entry['name'] == 'sc-observability-binding-runtime')
    cases = {
        'missing-unpublished': ('rust-bundle/' + dependency['root'], True),
        'missing-registry': (Path(registry[0]['manifest_path']).parent.relative_to(root).as_posix(), True),
        'missing-target-registry': (Path(target_registry[0]['manifest_path']).parent.relative_to(root).as_posix(), True),
        'missing-stubs': ('python/sc_observability/generated/__init__.pyi', False),
        'missing-py-typed': ('python/sc_observability/py.typed', False),
    }
    for name, (relative, build_must_fail) in cases.items():
        copy = scratch / name
        shutil.copytree(root, copy, ignore=shutil.ignore_patterns('target', '__pycache__'))
        victim = confined(copy, relative)
        shutil.rmtree(victim) if victim.is_dir() else victim.unlink()
        try:
            verify_source(copy)
        except DistributionError:
            pass
        else:
            raise DistributionError(f'{name} incorrectly passed integrity verification')
        if build_must_fail:
            sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline', '--format-version', '1'],
                        copy, expect_failure=True)
            sandbox.run([sys.executable, '-m', 'maturin', 'build', '--locked', '--offline',
                         '--out', str(scratch / 'rejected-wheels')], copy, expect_failure=True)
            sandbox.run([sandbox.cargo, 'build', '--locked', '--offline'], copy / 'embedding',
                        expect_failure=True)
        results[name] = {'integrity_rejected': True, 'offline_resolution_rejected': build_must_fail,
                         'wheel_and_embedding_builds_rejected': build_must_fail}
        shutil.rmtree(copy)
    for name, relative, content in (
        ('stale-lock', 'Cargo.lock', b'\n# deliberately stale frozen lock\n'),
        ('tampered-source', 'src/lib.rs', b'\n// deliberate byte tampering\n'),
        ('escaping-manifest', 'Cargo.toml', b'\n[dependencies.escape]\nversion="1"\npath="../../outside"\n'),
    ):
        copy = scratch / name
        shutil.copytree(root, copy)
        with confined(copy, relative).open('ab') as stream:
            stream.write(content)
        try:
            verify_source(copy)
        except DistributionError:
            results[name] = {'integrity_rejected': True}
        else:
            raise DistributionError(f'{name} incorrectly passed verification')
        shutil.rmtree(copy)
    import tomli_w
    from _python_distribution import tomllib
    copy = scratch / 'extension-link-flags'
    shutil.copytree(root, copy)
    manifest_path = copy / 'embedding/Cargo.toml'
    manifest = tomllib.loads(manifest_path.read_text())
    dependency = manifest['dependencies']['pyo3']
    dependency.setdefault('features', []).append('extension-module')
    manifest_path.write_text(tomli_w.dumps(manifest))
    wrong_link = json.loads(sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline',
                                         '--format-version', '1'], copy / 'embedding'))
    try:
        verify_embedding_features(wrong_link)
    except DistributionError:
        results['extension-link-flags'] = {'extension_flags_rejected': True}
    else:
        raise DistributionError('extension-only embedding flags passed qualification')
    shutil.rmtree(copy)
    return results


def run_embedding(root: Path, scratch: Path, sandbox: Sandbox, python: str) -> dict:
    """Build the real bundled host with this interpreter and an empty Cargo cache."""
    interpreter = json.loads(sandbox.run([python, '-I', '-c',
        'import json,sys; print(json.dumps({"python":f"{sys.version_info.major}.{sys.version_info.minor}",'
        '"python_full":sys.version,"base_prefix":sys.base_prefix}))'], scratch))
    keys = ('CARGO_HOME', 'CARGO_TARGET_DIR', 'PYTHONPATH', 'PYTHONHOME', 'PYO3_PYTHON')
    previous = {key: sandbox.env.get(key) for key in keys}
    sandbox.env.update(CARGO_HOME=str(scratch / 'embedding-cargo-home'),
                       CARGO_TARGET_DIR=str(scratch / 'embedding-target'),
                       PYTHONPATH=str(root / 'python'), PYTHONHOME=interpreter['base_prefix'],
                       PYO3_PYTHON=python)
    try:
        metadata = json.loads(sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline',
                                          '--format-version', '1'], root / 'embedding'))
        resolution = verify_resolution(metadata, root)
        verify_embedding_features(metadata)
        sandbox.run([sandbox.cargo, 'run', '--locked', '--offline', '--release'], root / 'embedding')
    finally:
        for key, value in previous.items():
            if value is None:
                sandbox.env.pop(key, None)
            else:
                sandbox.env[key] = value
    verify_source(root)
    return {'status': 'passed', **interpreter, 'executable': python, 'resolution': resolution}


def build(args) -> None:
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    checkouts = registered_checkouts(args.checkout)
    with tempfile.TemporaryDirectory(prefix='b4a-build-') as temporary:
        scratch = Path(temporary).resolve()
        root = extract_sdist(args.sdist, scratch / 'unpacked')
        source = verify_source(root)
        policy = policy_at(root)
        actual = actual_cell(policy)
        selected = {**next(item for item in policy['platforms'] if item['id'] == args.platform),
                    'expected_requires_python': source['expected_requires_python'],
                    'candidate_version': source['version']}
        if actual['platform'] != args.platform:
            raise DistributionError('build runner architecture differs from requested platform')
        if args.platform == 'windows-arm64':
            from python_arm64 import require_native_windows_arm64
            require_native_windows_arm64()
        # Tools were provisioned before entering isolation. Cargo uses a fresh empty home/target.
        with Sandbox(scratch, checkouts) as sandbox:
            if 'deployment_target' in selected:
                sandbox.env['MACOSX_DEPLOYMENT_TARGET'] = selected['deployment_target']
            probes = sandbox.prove_denials(sys.executable, args.checkout)
            metadata = json.loads(sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline',
                                              '--format-version', '1'], root))
            resolution = verify_resolution(metadata, root)
            sandbox.env['PYTHONHOME'] = sys.base_prefix
            native_output = sandbox.run([sandbox.cargo, 'test', '--locked', '--offline'], root)
            native_count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', native_output)))
            if not native_count:
                raise DistributionError('native runtime suite executed no tests')
            del sandbox.env['PYTHONHOME']
            command = [sys.executable, '-m', 'maturin', 'build', '--locked', '--offline', '--release',
                       '--out', str(scratch / 'wheels')]
            if selected.get('rust_target'):
                command += ['--target', selected['rust_target']]
            if args.platform.startswith('linux'):
                command += ['--compatibility', 'manylinux_2_28']
            sandbox.run(command, root)
            wheels = list((scratch / 'wheels').glob('*.whl'))
            if len(wheels) != 1:
                raise DistributionError('expected exactly one ABI wheel for the platform')
            features = tomllib.loads((root / 'pyproject.toml').read_text())['tool']['maturin']['features']
            linked = {**linkage(wheels[0], selected, sandbox, scratch), 'role': 'production',
                      'maturin_features': features, 'publication': 'pending_B.7'}
            release_wheel(linked)
            instrumented = None
            if fault_paths(source['runtime_suite']):
                private_command = command.copy()
                private_command[private_command.index('--out') + 1] = str(scratch / 'instrumented-wheels')
                private_command += ['--features', ','.join(sorted(set(features) | {'test-hooks'}))]
                sandbox.run(private_command, root)
                private_wheels = list((scratch / 'instrumented-wheels').glob('*.whl'))
                if len(private_wheels) != 1:
                    raise DistributionError('expected exactly one private fault companion')
                instrumented = {**linkage(private_wheels[0], selected, sandbox, scratch),
                                'role': 'instrumented', 'publication': 'never',
                                'maturin_features': sorted(set(features) | {'test-hooks'}),
                                'path': 'instrumented/' + private_wheels[0].name}
                if instrumented['sha256'] == linked['sha256']:
                    raise DistributionError('instrumented companion is not distinct from production')
                (output / 'instrumented').mkdir()
                shutil.copyfile(private_wheels[0], output / instrumented['path'])
            embedded = run_embedding(root, scratch, sandbox, sys.executable)
            negatives = negative_cases(root, scratch, sandbox, metadata)
            verify_source(root)
            record = {'schema_version': 1, 'status': 'passed', 'development_only': source.get('development_only', False), 'source_commit': source['source_commit'],
                      'sdist_sha256': digest(args.sdist), 'platform': args.platform,
                      'expected_requires_python': source['expected_requires_python'],
                      'build_interpreter': actual, 'wheel': linked, 'instrumented': instrumented, 'resolution': resolution,
                      'isolation': probes, 'native_runtime_tests': 'passed', 'native_test_count': native_count, 'embedding': 'passed', 'embedding_interpreter': embedded, 'negative_results': negatives,
                      'commands': sandbox.commands, 'publication': 'pending_B.7'}
            shutil.copyfile(wheels[0], output / wheels[0].name)
        (output / 'build-result.json').write_text(json.dumps(record, indent=2) + '\n')


def cell(args) -> None:
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    checkouts = registered_checkouts(args.checkout)
    with tempfile.TemporaryDirectory(prefix='b4a-installed-') as temporary:
        scratch = Path(temporary).resolve()
        root = extract_sdist(args.sdist, scratch / 'unpacked')
        source = verify_source(root)
        if (source.get('development_only') or not source['runtime_suite'].get('runtime_complete')) and not args.allow_incomplete_runtime:
            raise DistributionError('development/incomplete runtime artifact cannot qualify an installed cell')
        policy = policy_at(root)
        actual = actual_cell(policy)
        if actual['platform'] == 'windows-arm64':
            from python_arm64 import require_native_windows_arm64
            require_native_windows_arm64()
        selected = {**next(item for item in policy['platforms'] if item['id'] == actual['platform']),
                    'expected_requires_python': source['expected_requires_python'],
                    'candidate_version': source['version']}
        wheel = inspect_wheel(args.wheel, selected, source['version'])
        execute([sys.executable, '-m', 'venv', str(scratch / 'venv')])
        python = str(scratch / 'venv' / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python'))
        execute([python, '-m', 'pip', 'install', '--disable-pip-version-check', str(args.wheel)])
        # Check runtime imports before pytest/mypy can accidentally supply an undeclared dependency.
        with Sandbox(scratch, checkouts) as sandbox:
            sandbox.run([python, '-I', '-c', 'import sc_observability, sc_observability._native; print(sc_observability.__file__)'], scratch)
        execute([python, '-m', 'pip', 'install', '--disable-pip-version-check',
                 '-r', str(root / 'qualification/python-packaging-requirements.txt')])
        contract = source['runtime_suite']
        private_paths = fault_paths(contract)
        private_python, private_wheel = None, None
        if private_paths:
            if not args.instrumented_wheel:
                raise DistributionError('private fault companion is required by the source contract')
            private_wheel = inspect_wheel(args.instrumented_wheel, selected, source['version'])
            if private_wheel['sha256'] == wheel['sha256']:
                raise DistributionError('instrumented wheel cannot substitute for production')
            execute([sys.executable, '-m', 'venv', str(scratch / 'fault-venv')])
            private_python = str(scratch / 'fault-venv' / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python'))
            execute([private_python, '-m', 'pip', 'install', '--disable-pip-version-check',
                     str(args.instrumented_wheel), '-r', str(root / 'qualification/python-packaging-requirements.txt')])
        suite = scratch / 'suite'
        suite.mkdir()
        for relative in ('tests', 'examples'):
            if (root / relative).exists():
                shutil.copytree(root / relative, suite / relative)
        contract = source['runtime_suite']
        if (not contract.get('typing_paths') or
                ((source.get('development_only') or not contract.get('runtime_complete')) and not args.allow_incomplete_runtime)):
            raise DistributionError('full runtime/type suite contract is incomplete')
        with Sandbox(scratch, checkouts) as sandbox:
            probes = sandbox.prove_denials(python, args.checkout)
            imported = sandbox.run([python, '-I', '-c',
                'import pathlib,sys,sc_observability,sc_observability._native as n; '
                'root=pathlib.Path(sys.prefix).resolve(); '
                'assert pathlib.Path(sc_observability.__file__).resolve().is_relative_to(root); '
                'assert pathlib.Path(n.__file__).resolve().is_relative_to(root); '
                'assert not any(name.startswith("_test") for name in dir(n)), "private hooks leaked into production"; '
                'print(n.__file__)'], suite)
            flags, environment = runtime_options(contract)
            sandbox.env.update(environment)
            sandbox.env['SC_OBSERVABILITY_RUNTIME_TEST'] = '1'
            junit = scratch / 'runtime.xml'
            paths = [str(confined(suite, path)) for path in contract['pytest_paths']]
            ignored = ['--ignore=' + str(confined(suite, path)) for path in private_paths]
            sandbox.run([python, *flags, '-m', 'pytest', *paths, *ignored, '-ra', '--junitxml', str(junit)], suite)
            tree = ET.parse(junit)
            cases = tree.findall('.//testcase')
            if not cases or tree.findall('.//skipped') or tree.findall('.//failure') or tree.findall('.//error'):
                raise DistributionError('full runtime suite missing, skipped or failed')
            typed = [str(confined(suite, path)) for path in contract['typing_paths']]
            sandbox.run([python, '-I', '-m', 'mypy', '--strict', '--no-incremental',
                         '--cache-dir', str(scratch / 'mypy-cache'), *typed], suite)
            fault_result = None
            if private_python:
                private_identity = json.loads(sandbox.run([private_python, '-I', '-c',
                    'import json,pathlib,sys,sc_observability._native as n; '
                    'assert pathlib.Path(n.__file__).resolve().is_relative_to(pathlib.Path(sys.prefix).resolve()); '
                    'assert any(name.startswith("_test") for name in dir(n)); '
                    'print(json.dumps({"python_full":sys.version,"native":n.__file__}))'], suite))
                if private_identity['python_full'] != actual['python_full']:
                    raise DistributionError('fault companion interpreter differs from production cell')
                fault_junit = scratch / 'fault-runtime.xml'
                sandbox.run([private_python, *flags, '-m', 'pytest',
                             *[str(confined(suite, path)) for path in private_paths],
                             '-ra', '--junitxml', str(fault_junit)], suite)
                fault_tree = ET.parse(fault_junit)
                fault_cases = fault_tree.findall('.//testcase')
                if not fault_cases or any(fault_tree.findall('.//' + kind) for kind in ('skipped', 'failure', 'error')):
                    raise DistributionError('private fault suite missing, skipped or failed')
                fault_result = {'status': 'passed', 'role': 'instrumented', 'publication': 'never',
                                'wheel': private_wheel, 'executable': private_python,
                                'python_full': private_identity['python_full'], 'test_count': len(fault_cases),
                                'test_cases': sorted(case.attrib.get('classname', '') + '::' + case.attrib['name'] for case in fault_cases)}
                shutil.copyfile(fault_junit, output / 'fault-runtime.xml')
            embedded = None
            if contract.get('embedding_in_each_cell'):
                embedded = run_embedding(root, scratch, sandbox, python)
                if embedded['python_full'] != actual['python_full']:
                    raise DistributionError('embedding interpreter differs from installed cell')
            record = {'schema_version': 1, 'status': 'passed', **actual,
                      'development_only': args.allow_incomplete_runtime or source.get('development_only', False),
                      'source_commit': source['source_commit'], 'sdist_sha256': digest(args.sdist),
                      'expected_requires_python': source['expected_requires_python'],
                      'wheel': wheel, 'runtime_suite': contract, 'test_count': len(cases),
                      'test_cases': sorted(case.attrib.get('classname', '') + '::' + case.attrib['name'] for case in cases),
                      'installed_extension': imported.strip(), 'production_hooks_absent': True,
                      'fault_companion': fault_result, 'isolation': probes, 'embedding': embedded,
                      'typecheck': 'passed', 'interpreter_flags': flags, 'runtime_environment': environment,
                      'commands': sandbox.commands, 'publication': 'pending_B.7'}
            shutil.copyfile(junit, output / 'runtime.xml')
        (output / 'cell-result.json').write_text(json.dumps(record, indent=2) + '\n')


def aggregate(args) -> None:
    policy = json.loads(args.policy.read_text())
    expected = {(p['id'], python) for p in policy['platforms'] for python in policy['interpreters']}
    build_paths = list(args.evidence.rglob('build-result.json'))
    cell_paths = list(args.evidence.rglob('cell-result.json'))
    builds = [json.loads(path.read_text()) for path in build_paths]
    cells = [json.loads(path.read_text()) for path in cell_paths]
    if len(policy['platforms']) != 6 or len(builds) != 6 or len(cells) != 30:
        raise DistributionError('all six builds and all 30 execution cells are required')
    if {(cell['platform'], cell['python']) for cell in cells} != expected:
        raise DistributionError('matrix contains missing, duplicate or unsupported cells')
    wheel_hashes = {build['platform']: build['wheel']['sha256'] for build in builds}
    if len(wheel_hashes) != 5:
        raise DistributionError('missing or duplicate platform wheels')
    for item in builds + cells:
        if (item.get('status') != 'passed' or item.get('development_only') or item.get('source_commit') != args.source_commit
                or item.get('sdist_sha256') != digest(args.sdist)
                or not all(item['isolation'].get(key) is True for key in ('checkout', 'cargo_cache', 'network'))):
            raise DistributionError('mixed source/artifacts or incomplete isolation evidence')
    with tempfile.TemporaryDirectory(prefix='b4a-aggregate-') as temporary:
        root = extract_sdist(args.sdist, Path(temporary) / 'source')
        source = verify_source(root)
        contract = source['runtime_suite']
        if (source.get('development_only') or not contract.get('runtime_complete')
                or source['source_commit'] != args.source_commit):
            raise DistributionError('source contract is not a completed qualification candidate')
        runtime_options(contract)
        production_features = sorted(tomllib.loads((root / 'pyproject.toml').read_text())['tool']['maturin']['features'])
        expected_requires_python = source['expected_requires_python']
    if any(item.get('expected_requires_python') != expected_requires_python for item in builds + cells):
        raise DistributionError('source and build/cell Requires-Python metadata disagree')
    for item in cells:
        if item.get('runtime_suite') != contract:
            raise DistributionError('cell contract differs from the immutable source contract')
        if contract.get('embedding_in_each_cell'):
            embedded = item.get('embedding') or {}
            if (embedded.get('status') != 'passed' or embedded.get('python_full') != item['python_full']
                    or embedded.get('python') != item['python']
                    or not any(command['command'][1:] == ['run', '--locked', '--offline', '--release']
                               and command['exit_code'] == 0 for command in item['commands'])):
                raise DistributionError('missing interpreter-matched embedded-host execution')
        if item['wheel']['sha256'] != wheel_hashes[item['platform']] or item.get('typecheck') != 'passed':
            raise DistributionError('interpreter cell did not execute its shared ABI wheel and type suite')
    for item in cells:
        if item.get('production_hooks_absent') is not True:
            raise DistributionError('production wheel lacks an executed private-hook absence proof')
    private_hashes = {build['platform']: (build.get('instrumented') or {}).get('sha256') for build in builds}
    private_cases = set()
    for path, item in zip(cell_paths, cells):
        if fault_paths(contract):
            fault = item.get('fault_companion') or {}
            if (fault.get('status') != 'passed' or fault.get('role') != 'instrumented'
                    or fault.get('publication') != 'never' or fault.get('python_full') != item['python_full']
                    or fault.get('wheel', {}).get('sha256') != private_hashes[item['platform']]
                    or fault.get('wheel', {}).get('sha256') == item['wheel']['sha256']):
                raise DistributionError('missing separately identified same-interpreter fault companion')
            private_xml = ET.parse(path.with_name('fault-runtime.xml'))
            names = sorted(case.attrib.get('classname', '') + '::' + case.attrib['name'] for case in private_xml.findall('.//testcase'))
            if (not names or names != fault.get('test_cases') or len(names) != fault.get('test_count')
                    or any(private_xml.findall('.//' + kind) for kind in ('skipped', 'failure', 'error'))
                    or not any(command['command'][0] == fault.get('executable') and 'pytest' in command['command']
                               and command['exit_code'] == 0 for command in item['commands'])):
                raise DistributionError('missing raw zero-skip private fault execution evidence')
            private_cases.add(tuple(names))
        xml = ET.parse(path.with_name('runtime.xml'))
        actual_cases = sorted(case.attrib.get('classname', '') + '::' + case.attrib['name'] for case in xml.findall('.//testcase'))
        if (actual_cases != item['test_cases'] or len(actual_cases) != item['test_count']
                or not actual_cases or any(xml.findall('.//' + kind) for kind in ('skipped', 'failure', 'error'))):
            raise DistributionError('raw JUnit evidence disagrees with the cell result')
        for tool in ('pytest', 'mypy'):
            if not any(tool in command['command'] and command['exit_code'] == 0 for command in item['commands']):
                raise DistributionError('missing successful runtime/type command log')
    if fault_paths(contract) and len(private_cases) != 1:
        raise DistributionError('cells executed different private fault suites')
    cases = {tuple(cell['test_cases']) for cell in cells}
    if len(cases) != 1:
        raise DistributionError('interpreter/platform cells executed different runtime suites')
    publication_wheels = []
    production_paths = []
    for path, build in zip(build_paths, builds):
        production = release_wheel(build['wheel'])
        if sorted(production.get('maturin_features', [])) != production_features:
            raise DistributionError('production feature identity differs from immutable source')
        publication_wheels.append({'platform': build['platform'], **production})
        selected = next(item for item in policy['platforms'] if item['id'] == build['platform'])
        wheel = confined(path.parent, build['wheel']['wheel'])
        production_paths.append(wheel)
        selected = {**selected, 'expected_requires_python': expected_requires_python,
                    'candidate_version': policy['candidate_version']}
        inspected = inspect_wheel(wheel, selected, policy['candidate_version'])
        if fault_paths(contract):
            private = build.get('instrumented') or {}
            if (private.get('role') != 'instrumented' or private.get('publication') != 'never'
                    or private.get('maturin_features') != sorted(set(production_features) | {'test-hooks'})):
                raise DistributionError('fault companion lacks exact non-public feature identity')
            private_inspection = inspect_wheel(confined(path.parent, private['path']), selected, policy['candidate_version'])
            if private_inspection['sha256'] != private.get('sha256') or private['sha256'] == inspected['sha256']:
                raise DistributionError('retained fault companion identity disagrees with build evidence')
        if inspected['sha256'] != build['wheel']['sha256']:
            raise DistributionError('retained wheel checksum differs from build evidence')
        if build.get('embedding') != 'passed' or build.get('native_runtime_tests') != 'passed' or len(build.get('negative_results', {})) != 9:
            raise DistributionError('missing embedding or negative-artifact execution evidence')
    publication = {'schema_version': 1, 'source_commit': args.source_commit, 'publication': 'pending_B.7',
                   'sdist_sha256': digest(args.sdist), 'sdist': {'path': 'dist/' + args.sdist.name, 'sha256': digest(args.sdist)},
                   'wheels': [{**wheel, 'path': 'dist/' + wheel['wheel']} for wheel in publication_wheels]}
    if getattr(args, 'publication_dir', None):
        dist = args.publication_dir / 'dist'
        dist.mkdir(parents=True, exist_ok=False)
        for artifact in [args.sdist, *production_paths]:
            shutil.copyfile(artifact, dist / artifact.name)
    if getattr(args, 'output', None):
        args.output.write_text(json.dumps(publication, indent=2) + '\n')
    print('B4A_QUALIFIED: six ABI wheels, 30 installed full-suite cells, offline sdist and embedding')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='mode', required=True)
    for mode in ('build', 'cell'):
        child = commands.add_parser(mode)
        child.add_argument('--sdist', type=Path, required=True)
        child.add_argument('--output', type=Path, required=True)
        child.add_argument('--checkout', type=Path, required=True)
        if mode == 'build':
            child.add_argument('--platform', required=True)
        else:
            child.add_argument('--wheel', type=Path, required=True)
            child.add_argument('--instrumented-wheel', type=Path)
            child.add_argument('--allow-incomplete-runtime', action='store_true',
                               help='execute provisional suites; aggregate still rejects these results')
    child = commands.add_parser('aggregate')
    child.add_argument('--policy', type=Path, required=True)
    child.add_argument('--sdist', type=Path, required=True)
    child.add_argument('--evidence', type=Path, required=True)
    child.add_argument('--source-commit', required=True)
    child.add_argument('--output', type=Path, help='write production-only future publication inventory')
    child.add_argument('--publication-dir', type=Path, help='copy only qualified sdist and production wheels under dist/')
    args = parser.parse_args()
    {'build': build, 'cell': cell, 'aggregate': aggregate}[args.mode](args)


if __name__ == '__main__':
    main()
