#!/usr/bin/env python3
"""Create an immutable maturin sdist using the sole shared Cargo bundle helper."""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import tomli_w
from _python_distribution import DistributionError, confined, digest, extract_sdist, tomllib, verify_source

ROOT = Path(__file__).resolve().parents[2]
PROJECT = Path('bindings/python/sc-observability-py')


def run(arguments: list[str], cwd: Path, log: Path) -> None:
    with log.open('a') as output:
        output.write(json.dumps(arguments) + '\n')
        output.flush()
        result = subprocess.run(arguments, cwd=cwd, stdout=output, stderr=subprocess.STDOUT)
    if result.returncode:
        raise DistributionError(f'command failed ({result.returncode}); see {log}')


def prepare(source: Path, output: Path, allow_incomplete_runtime: bool = False) -> dict:
    source, output = source.resolve(), output.resolve()
    if output.exists():
        raise DistributionError('refusing to overwrite an immutable distribution stage')
    if subprocess.check_output(['git', 'status', '--porcelain'], cwd=source, text=True).strip():
        raise DistributionError('distribution source must be committed and clean')
    source_sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip()
    project = source / PROJECT
    helper = source / 'scripts/ci/build_binding_source_bundle.py'
    if not helper.is_file():
        raise DistributionError('shared B.3 bundle helper must be inherited before packaging')
    for relative in ('python/sc_observability/generated/__init__.pyi', 'python/sc_observability/py.typed', 'tests'):
        if not (project / relative).exists():
            raise DistributionError(f'full B.4 runtime package is not ready: {relative}')
    output.mkdir(parents=True)
    staging = output / 'source'
    staging.mkdir()
    bundle = staging / 'rust-bundle'
    log = output / 'prepare.log'
    run([sys.executable, str(helper), '--root-manifest', str(project / 'Cargo.toml'),
         '--output', str(bundle)], source, log)
    evidence = json.loads((bundle / 'manifest.json').read_text())
    roots = [entry for entry in evidence['packages'] if entry['name'] == 'sc-observability-py']
    if len(roots) != 1:
        raise DistributionError('bundle lacks the Python extension/embedding root')
    packaged = bundle / roots[0]['root']
    for path in packaged.iterdir():
        if path.name in ('.cargo_vcs_info.json', 'Cargo.toml.orig', 'Cargo.lock'):
            continue
        target = staging / path.name
        if path.is_dir():
            shutil.copytree(path, target)
        else:
            shutil.copyfile(path, target)
    # Python package data and the unchanged runtime tests are explicit sdist inputs.
    for relative in ('python', 'tests', 'examples'):
        if (project / relative).is_dir():
            if any(path.is_symlink() for path in (project / relative).rglob('*')):
                raise DistributionError(f'symlink in Python source inputs: {relative}')
            shutil.copytree(project / relative, staging / relative, dirs_exist_ok=True)
    shutil.copyfile(source / 'LICENSE', staging / 'LICENSE')
    suite_path = project / 'qualification-suite.json'
    if suite_path.is_file():
        suite = json.loads(suite_path.read_text())
    elif allow_incomplete_runtime:
        suite = {'schema_version': 1, 'runtime_complete': False, 'pytest_paths': ['tests'],
                 'typing_paths': ['tests/typing/test_result_narrowing.py'],
                 'embedding_manifest': 'examples/rust-python-logging/Cargo.toml'}
    else:
        raise DistributionError('missing B.4 runtime qualification contract')
    if (suite.get('schema_version') != 1 or (suite.get('runtime_complete') is not True and not allow_incomplete_runtime)
            or suite.get('pytest_paths') != ['tests'] or not suite.get('typing_paths')):
        raise DistributionError('B.4 full-runtime qualification contract is not complete')
    (staging / 'qualification-suite.json').write_text(json.dumps(suite, indent=2) + '\n')
    embedding = confined(source, suite['embedding_manifest'])
    if not embedding.is_file():
        raise DistributionError('missing real Rust embedding fixture')
    if any(path.is_symlink() for path in embedding.parent.rglob('*')):
        raise DistributionError('symlink in embedding source inputs')
    shutil.copytree(embedding.parent, staging / 'embedding', ignore=shutil.ignore_patterns('target'))
    workspace = tomllib.loads((source / 'Cargo.toml').read_text())['workspace']
    embedded = tomllib.loads((staging / 'embedding/Cargo.toml').read_text())
    for key, value in list(embedded['package'].items()):
        if isinstance(value, dict) and value.get('workspace'):
            embedded['package'][key] = workspace['package'][key]
    for table in [embedded, *embedded.get('target', {}).values()]:
        for section in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            for name, spec in list(table.get(section, {}).items()):
                if isinstance(spec, dict) and spec.get('workspace'):
                    inherited = workspace['dependencies'][name]
                    combined = dict(inherited) if isinstance(inherited, dict) else {'version': inherited}
                    combined['features'] = sorted(set(combined.get('features', [])) | set(spec.get('features', [])))
                    combined.update({key: value for key, value in spec.items() if key not in ('workspace', 'features')})
                    spec = combined
                if isinstance(spec, dict):
                    if 'path' in spec and 'version' not in spec:
                        raise DistributionError(f'embedding dependency lacks version: {name}')
                    spec.pop('path', None)
                table[section][name] = spec
    if embedded.get('lints', {}).get('workspace'):
        embedded['lints'] = workspace.get('lints', {})
    embedded['workspace'] = {}
    embedded['patch'] = {'crates-io': {entry['name']: {'path': '../rust-bundle/' + entry['root']}
                                     for entry in evidence['packages']}}
    (staging / 'embedding/Cargo.toml').write_text(tomli_w.dumps(embedded))
    manifest = tomllib.loads((staging / 'Cargo.toml').read_text())
    manifest['workspace'] = {}
    # Cargo's own source listing must not recursively package bundled .crate metadata.
    # Maturin's explicit sdist includes below retain the complete bundle unchanged.
    manifest['package']['exclude'] = sorted(set(manifest['package'].get('exclude', [])) | {
        'rust-bundle/**', 'embedding/**', 'qualification/**', 'distribution-manifest.json'})
    manifest.setdefault('patch', {})['crates-io'] = {
        entry['name']: {'path': f"rust-bundle/{entry['root']}"}
        for entry in evidence['packages'] if entry['name'] != 'sc-observability-py'
    }
    (staging / 'Cargo.toml').write_text(tomli_w.dumps(manifest))
    # Source replacement contains only published dependencies; first-party crates use root patches.
    config = tomllib.loads((bundle / '.cargo/config.toml').read_text())
    for value in config.get('source', {}).values():
        if 'directory' in value:
            value['directory'] = 'rust-bundle/' + value['directory']
    config['net'] = {'offline': True}
    (staging / '.cargo').mkdir(exist_ok=True)
    (staging / '.cargo/config.toml').write_text(tomli_w.dumps(config))
    pyproject = tomllib.loads((project / 'pyproject.toml').read_text())
    maturin = pyproject.setdefault('tool', {}).setdefault('maturin', {})
    maturin['features'] = sorted(set(maturin.get('features', [])) | {'pyo3/extension-module'})
    maturin['include'] = [{'path': item, 'format': 'sdist'} for item in (
        'rust-bundle/**/*', '.cargo/config.toml', 'Cargo.lock', 'LICENSE', 'tests/**/*',
        'examples/**/*', 'embedding/**/*', 'qualification/**/*', 'qualification-suite.json',
        'distribution-manifest.json')]
    (staging / 'pyproject.toml').write_text(tomli_w.dumps(pyproject))
    qualification = staging / 'qualification'
    qualification.mkdir(exist_ok=True)
    for filename in ('_python_distribution.py', '_python_sandbox.py',
                     'validate_python_distribution.py', 'build_binding_source_bundle.py',
                     '_log_staging.py', 'python-packaging-requirements.txt'):
        shutil.copyfile(source / 'scripts/ci' / filename, qualification / filename)
    shutil.copyfile(source / 'release/python-platform-policy.json', qualification / 'platform-policy.json')
    shutil.copyfile(bundle / 'Cargo.lock', staging / 'Cargo.lock')
    run(['cargo', 'metadata', '--offline', '--format-version', '1'], staging, log)
    shutil.copyfile(bundle / 'Cargo.lock', staging / 'embedding/Cargo.lock')
    run(['cargo', 'metadata', '--offline', '--format-version', '1', '--manifest-path', str(staging / 'embedding/Cargo.toml')], staging, log)
    run(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1'], staging, log)
    files = {path.relative_to(staging).as_posix(): digest(path)
             for path in sorted(staging.rglob('*')) if path.is_file()}
    record = {'schema_version': 1, 'source_commit': source_sha, 'version': roots[0]['version'],
              'publication': 'pending_B.7', 'development_only': allow_incomplete_runtime, 'bundle_manifest_sha256': digest(bundle / 'manifest.json'),
              'files': files, 'runtime_suite': suite}
    (staging / 'distribution-manifest.json').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
    verify_source(staging)
    run([sys.executable, '-m', 'maturin', 'sdist', '--manifest-path', str(staging / 'Cargo.toml'),
         '--out', str(output / 'dist')], staging, log)
    sdists = list((output / 'dist').glob('*.tar.gz'))
    if len(sdists) != 1:
        raise DistributionError('maturin must produce exactly one sdist')
    extracted = extract_sdist(sdists[0], output / 'verification')
    verify_source(extracted)
    result = {'schema_version': 1, 'source_commit': source_sha, 'version': roots[0]['version'],
              'development_only': allow_incomplete_runtime, 'sdist': sdists[0].name, 'sdist_sha256': digest(sdists[0]),
              'distribution_manifest_sha256': digest(staging / 'distribution-manifest.json'),
              'prepare_log_sha256': digest(log), 'publication': 'pending_B.7'}
    (output / 'sdist-result.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--allow-incomplete-runtime', action='store_true',
                        help='development artifacts only; never accepted by final matrix qualification')
    args = parser.parse_args()
    print(json.dumps(prepare(args.source, args.output, args.allow_incomplete_runtime), indent=2))


if __name__ == '__main__':
    main()
