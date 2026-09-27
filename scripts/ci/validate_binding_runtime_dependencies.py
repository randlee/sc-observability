#!/usr/bin/env python3
"""Resolve the native runtime's exact first-party boundary, including aliases/targets."""
import json
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SUPPORT = {'arc-swap', 'serde_json'}
ALLOWED_HOST_EDGES = {
    'sc-observability-tauri': {'tauri', 'tauri-runtime', 'tauri-plugin'},
    'sc-observability-py': {'pyo3'},
}

def load_boundary_manifest(package):
    matches = []
    for path in (ROOT / 'boundaries').glob('*/*.toml'):
        document = tomllib.loads(path.read_text(encoding='utf-8'))
        if document.get('owner_package') == package:
            matches.append((path, document))
    if len(matches) > 1:
        raise ValueError(f'{package} has multiple boundary manifests: {[str(path) for path, _ in matches]}')
    if not matches:
        return None
    return matches[0]

def boundary_dependencies(boundary, package):
    if boundary is None:
        raise ValueError(f'{package} boundary manifest is missing')
    if boundary.get('owner_package') != package:
        raise ValueError(f'{package} boundary manifest owner mismatch')
    try:
        dependencies = boundary['dependencies']['allowed_dependencies']
    except KeyError as error:
        raise ValueError(f'{package} boundary manifest lacks dependencies.allowed_dependencies') from error
    if not isinstance(dependencies, list) or not all(isinstance(name, str) for name in dependencies):
        raise ValueError(f'{package} boundary dependencies must be a list of package names')
    return set(dependencies)

def validate_manifest(document, workspace, boundary=None):
    runtime = set()
    for table in [document, *document.get('target', {}).values()]:
        for section in ('dependencies', 'build-dependencies', 'dev-dependencies'):
            for alias, declaration in table.get(section, {}).items():
                spec = declaration if isinstance(declaration, dict) else {}
                if spec.get('workspace'):
                    spec = workspace['workspace']['dependencies'][alias]
                    spec = spec if isinstance(spec, dict) else {}
                name = spec.get('package', alias)
                if name.startswith(('tauri', 'pyo3')):
                    raise ValueError(f'forbidden host dependency: {name}')
                if section != 'dev-dependencies':
                    runtime.add(name)
    expected = boundary_dependencies(boundary, 'sc-observability-binding-runtime') | SUPPORT
    if runtime != expected:
        raise ValueError(f'runtime dependency drift: {sorted(runtime)}')

def manifest_names(document, workspace=None):
    names = set()
    for table in [document, *document.get('target', {}).values()]:
        for section in ('dependencies', 'build-dependencies', 'dev-dependencies'):
            for alias, declaration in table.get(section, {}).items():
                spec = declaration if isinstance(declaration, dict) else {}
                if spec.get('workspace'):
                    if workspace is None:
                        raise ValueError(f'workspace dependency cannot be resolved for alias: {alias}')
                    try:
                        spec = workspace['workspace']['dependencies'][alias]
                    except KeyError as error:
                        raise ValueError(f'workspace dependency alias is undeclared: {alias}') from error
                    spec = spec if isinstance(spec, dict) else {}
                names.add(spec.get('package', alias))
    return names

def validate_consumer_manifest(document, package, workspace=None, boundary=None):
    actual = manifest_names(document, workspace)
    expected = boundary_dependencies(boundary, package)
    first_party = {name for name in actual if name.startswith('sc-observability')}
    if first_party != expected:
        raise ValueError(f'{package} first-party edge drift: {sorted(first_party)}')
    forbidden = {'tauri', 'tauri-runtime', 'pyo3', 'tauri-plugin'} - ALLOWED_HOST_EDGES[package]
    if actual & forbidden:
        raise ValueError(f'{package} forbidden host edge: {sorted(actual & forbidden)}')

def main():
    workspace = tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'))
    document = tomllib.loads((ROOT/'crates/sc-observability-binding-runtime/Cargo.toml').read_text(encoding='utf-8'))
    _, runtime_boundary = load_boundary_manifest('sc-observability-binding-runtime') or (None, None)
    validate_manifest(document, workspace, runtime_boundary)
    validated_consumers = []
    deferred_consumers = []
    for package, path in {
        'sc-observability-tauri': ROOT/'bindings/tauri/Cargo.toml',
        'sc-observability-py': ROOT/'bindings/python/sc-observability-py/Cargo.toml',
    }.items():
        boundary_match = load_boundary_manifest(package)
        if boundary_match is None:
            deferred_consumers.append(package)
            continue
        _, boundary = boundary_match
        validate_consumer_manifest(tomllib.loads(path.read_text(encoding='utf-8')), package, workspace, boundary)
        validated_consumers.append(package)
    metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'], cwd=ROOT, text=True))
    packages = {p['id']:p for p in metadata['packages']}
    runtime = next(p for p in packages.values() if p['name']=='sc-observability-binding-runtime')
    node = next(n for n in metadata['resolve']['nodes'] if n['id']==runtime['id'])
    direct = {packages[d['pkg']]['name'] for d in node['deps'] if any(k['kind']!='dev' for k in d['dep_kinds']) and packages[d['pkg']]['source'] is None}
    expected_runtime = boundary_dependencies(runtime_boundary, 'sc-observability-binding-runtime')
    if direct != expected_runtime:
        raise ValueError(f'resolved workspace edge drift: {sorted(direct)}')
    consumers = ','.join(sorted(validated_consumers)) or 'none'
    suffix = f' deferred={",".join(sorted(deferred_consumers))}' if deferred_consumers else ''
    print('BINDING_RUNTIME_DEPENDENCIES_PASS '+','.join(sorted(direct))+f' consumers={consumers}{suffix}')

if __name__ == '__main__':
    main()
