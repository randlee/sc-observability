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
                                  extract_sdist, inspect_wheel, verify_source, runtime_options)
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
    details = inspect_wheel(wheel, policy, '1.4.0')
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
        dependencies = [entry.dll.decode() for entry in image.DIRECTORY_ENTRY_IMPORT]
        if any(re.fullmatch(r'python\d{2,}\.dll', name.lower()) for name in dependencies):
            raise DistributionError('abi3 wheel links interpreter-specific Python DLL')
        output = '\n'.join(dependencies)
    return {**details, 'linked_libraries': output}


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
        'missing-registry': (str(Path(registry[0]['manifest_path']).parent.relative_to(root)), True),
        'missing-target-registry': (str(Path(target_registry[0]['manifest_path']).parent.relative_to(root)), True),
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
    return results


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
        selected = next(item for item in policy['platforms'] if item['id'] == args.platform)
        if actual['platform'] != args.platform:
            raise DistributionError('build runner architecture differs from requested platform')
        # Tools were provisioned before entering isolation. Cargo uses a fresh empty home/target.
        with Sandbox(scratch, checkouts) as sandbox:
            if 'deployment_target' in selected:
                sandbox.env['MACOSX_DEPLOYMENT_TARGET'] = selected['deployment_target']
            probes = sandbox.prove_denials(sys.executable, args.checkout)
            metadata = json.loads(sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline',
                                              '--format-version', '1'], root))
            resolution = verify_resolution(metadata, root)
            command = [sys.executable, '-m', 'maturin', 'build', '--locked', '--offline', '--release',
                       '--out', str(scratch / 'wheels')]
            if args.platform.startswith('linux'):
                command += ['--compatibility', 'manylinux_2_28']
            sandbox.run(command, root)
            wheels = list((scratch / 'wheels').glob('*.whl'))
            if len(wheels) != 1:
                raise DistributionError('expected exactly one ABI wheel for the platform')
            linked = linkage(wheels[0], selected, sandbox, scratch)
            # Run the same Rust host example with normal rlib linking; never extension flags.
            sandbox.env['PYTHONPATH'] = str(root / 'python')
            embedded = json.loads(sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline',
                                              '--format-version', '1'], root / 'embedding'))
            verify_resolution(embedded, root)
            if any('extension-module' in node['features'] for node in embedded['resolve']['nodes']):
                raise DistributionError('extension-only features leaked into embedding link settings')
            sandbox.run([sandbox.cargo, 'run', '--locked', '--offline', '--release'], root / 'embedding')
            del sandbox.env['PYTHONPATH']
            negatives = negative_cases(root, scratch, sandbox, metadata)
            record = {'schema_version': 1, 'status': 'passed', 'development_only': source.get('development_only', False), 'source_commit': source['source_commit'],
                      'sdist_sha256': digest(args.sdist), 'platform': args.platform,
                      'build_interpreter': actual, 'wheel': linked, 'resolution': resolution,
                      'isolation': probes, 'embedding': 'passed', 'negative_results': negatives,
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
        if source.get('development_only') or not source['runtime_suite'].get('runtime_complete'):
            raise DistributionError('development/incomplete runtime artifact cannot qualify an installed cell')
        actual = actual_cell(policy_at(root))
        selected = next(item for item in policy_at(root)['platforms'] if item['id'] == actual['platform'])
        wheel = inspect_wheel(args.wheel, selected, source['version'])
        execute([sys.executable, '-m', 'venv', str(scratch / 'venv')])
        python = str(scratch / 'venv' / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python'))
        execute([python, '-m', 'pip', 'install', '--disable-pip-version-check', str(args.wheel)])
        # Check runtime imports before pytest/mypy can accidentally supply an undeclared dependency.
        with Sandbox(scratch, checkouts) as sandbox:
            sandbox.run([python, '-I', '-c', 'import sc_observability, sc_observability._native; print(sc_observability.__file__)'], scratch)
        execute([python, '-m', 'pip', 'install', '--disable-pip-version-check',
                 '-r', str(root / 'qualification/python-packaging-requirements.txt')])
        suite = scratch / 'suite'
        suite.mkdir()
        for relative in ('tests', 'examples'):
            if (root / relative).exists():
                shutil.copytree(root / relative, suite / relative)
        contract = source['runtime_suite']
        if source.get('development_only') or not contract.get('runtime_complete') or not contract.get('typing_paths'):
            raise DistributionError('full runtime/type suite contract is incomplete')
        with Sandbox(scratch, checkouts) as sandbox:
            probes = sandbox.prove_denials(python, args.checkout)
            imported = sandbox.run([python, '-I', '-c',
                'import pathlib,sys,sc_observability,sc_observability._native as n; '
                'root=pathlib.Path(sys.prefix).resolve(); '
                'assert pathlib.Path(sc_observability.__file__).resolve().is_relative_to(root); '
                'assert pathlib.Path(n.__file__).resolve().is_relative_to(root); '
                'print(n.__file__)'], suite)
            flags, environment = runtime_options(contract)
            sandbox.env.update(environment)
            sandbox.env['SC_OBSERVABILITY_RUNTIME_TEST'] = '1'
            junit = scratch / 'runtime.xml'
            paths = [str(confined(suite, path)) for path in contract['pytest_paths']]
            sandbox.run([python, *flags, '-m', 'pytest', *paths, '-ra', '--junitxml', str(junit)], suite)
            tree = ET.parse(junit)
            cases = tree.findall('.//testcase')
            if not cases or tree.findall('.//skipped') or tree.findall('.//failure') or tree.findall('.//error'):
                raise DistributionError('full runtime suite missing, skipped or failed')
            typed = [str(confined(suite, path)) for path in contract['typing_paths']]
            sandbox.run([python, '-I', '-m', 'mypy', '--strict', '--no-incremental',
                         '--cache-dir', str(scratch / 'mypy-cache'), *typed], suite)
            record = {'schema_version': 1, 'status': 'passed', **actual,
                      'source_commit': source['source_commit'], 'sdist_sha256': digest(args.sdist),
                      'wheel': wheel, 'runtime_suite': contract, 'test_count': len(cases),
                      'test_cases': sorted(case.attrib.get('classname', '') + '::' + case.attrib['name'] for case in cases),
                      'installed_extension': imported.strip(), 'isolation': probes,
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
    if len(builds) != 5 or len(cells) != 25:
        raise DistributionError('all five builds and all 25 execution cells are required')
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
    for item in cells:
        if item['wheel']['sha256'] != wheel_hashes[item['platform']] or item.get('typecheck') != 'passed':
            raise DistributionError('interpreter cell did not execute its shared ABI wheel and type suite')
    for path, item in zip(cell_paths, cells):
        xml = ET.parse(path.with_name('runtime.xml'))
        actual_cases = sorted(case.attrib.get('classname', '') + '::' + case.attrib['name'] for case in xml.findall('.//testcase'))
        if (actual_cases != item['test_cases'] or len(actual_cases) != item['test_count']
                or not actual_cases or any(xml.findall('.//' + kind) for kind in ('skipped', 'failure', 'error'))):
            raise DistributionError('raw JUnit evidence disagrees with the cell result')
        for tool in ('pytest', 'mypy'):
            if not any(tool in command['command'] and command['exit_code'] == 0 for command in item['commands']):
                raise DistributionError('missing successful runtime/type command log')
    cases = {tuple(cell['test_cases']) for cell in cells}
    if len(cases) != 1:
        raise DistributionError('interpreter/platform cells executed different runtime suites')
    for path, build in zip(build_paths, builds):
        selected = next(item for item in policy['platforms'] if item['id'] == build['platform'])
        wheel = confined(path.parent, build['wheel']['wheel'])
        inspected = inspect_wheel(wheel, selected, policy['candidate_version'])
        if inspected['sha256'] != build['wheel']['sha256']:
            raise DistributionError('retained wheel checksum differs from build evidence')
        if build.get('embedding') != 'passed' or len(build.get('negative_results', {})) != 8:
            raise DistributionError('missing embedding or negative-artifact execution evidence')
    print('B4A_QUALIFIED: five ABI wheels, 25 installed full-suite cells, offline sdist and embedding')


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
    child = commands.add_parser('aggregate')
    child.add_argument('--policy', type=Path, required=True)
    child.add_argument('--sdist', type=Path, required=True)
    child.add_argument('--evidence', type=Path, required=True)
    child.add_argument('--source-commit', required=True)
    args = parser.parse_args()
    {'build': build, 'cell': cell, 'aggregate': aggregate}[args.mode](args)


if __name__ == '__main__':
    main()
