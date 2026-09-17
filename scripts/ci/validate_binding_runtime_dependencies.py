#!/usr/bin/env python3
"""Resolve the native runtime's exact first-party boundary, including aliases/targets."""
import json
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXPECTED = {'sc-observability', 'sc-observability-types', 'sc-observability-dto', 'sc-observability-log'}
SUPPORT = {'arc-swap', 'serde_json'}

def validate_manifest(document, workspace):
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
    if runtime != EXPECTED | SUPPORT:
        raise ValueError(f'runtime dependency drift: {sorted(runtime)}')

def main():
    workspace = tomllib.loads((ROOT/'Cargo.toml').read_text())
    document = tomllib.loads((ROOT/'crates/sc-observability-binding-runtime/Cargo.toml').read_text())
    validate_manifest(document, workspace)
    metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'], cwd=ROOT, text=True))
    packages = {p['id']:p for p in metadata['packages']}
    runtime = next(p for p in packages.values() if p['name']=='sc-observability-binding-runtime')
    node = next(n for n in metadata['resolve']['nodes'] if n['id']==runtime['id'])
    direct = {packages[d['pkg']]['name'] for d in node['deps'] if any(k['kind']!='dev' for k in d['dep_kinds']) and packages[d['pkg']]['source'] is None}
    if direct != EXPECTED:
        raise ValueError(f'resolved workspace edge drift: {sorted(direct)}')
    print('BINDING_RUNTIME_DEPENDENCIES_PASS '+','.join(sorted(direct)))

if __name__ == '__main__':
    main()
