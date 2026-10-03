#!/usr/bin/env python3
"""ADR-022: compare the compiled public API of every published crate across release targets.

`collect` renders one compiled API surface per (package, feature selection,
target) with the pinned nightly rustdoc and the pinned public-api renderer.
`compare` is the single authoritative comparison: it rebuilds the expected
inventory from `release/publish-artifacts.toml` and Cargo metadata, requires
every expected cell from the same source commit and toolchain, and fails on
any row difference between targets. Missing, duplicate, malformed, empty or
failed inputs are failures, never skips.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shlex
import subprocess
import sys
import tomllib
from collections import Counter
from itertools import combinations
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = ROOT / 'release/publish-artifacts.toml'
TOOLCHAIN_PATH = Path(__file__).with_name('public-api-toolchain')
RENDERER_DIR = ROOT / 'scripts/ci/fixtures/public-api-parity/surface-renderer'
EVIDENCE_DIRNAME = 'public-api-parity'
CELL_SCHEMA = 1
RUSTDOC_ARGS = ('-Z', 'unstable-options', '--output-format', 'json', '--document-hidden-items')
LIBRARY_KINDS = {'lib', 'rlib', 'dylib', 'cdylib', 'proc-macro'}
REQUIRED_CELL_KEYS = (
    'schema', 'package', 'lib_name', 'crate_types', 'target', 'selection', 'source_commit',
    'toolchain', 'rustdoc_args', 'renderer', 'status', 'rows', 'row_count', 'rows_sha256',
)
LOG_TAIL = 4000


class ParityError(Exception):
    """The comparison cannot pass: incomplete, inconsistent or unequal inputs."""


# --- expected inventory -----------------------------------------------------

def load_manifest(path: Path = MANIFEST_PATH) -> dict:
    return tomllib.loads(path.read_text(encoding='utf-8'))


def release_targets(manifest: dict) -> list[str]:
    """Union of binary release targets and published wheel targets."""
    targets: set[str] = set()
    for entry in manifest.get('release_targets', []):
        targets.add(entry['target'])
    for distribution in manifest.get('python_distributions', []):
        for wheel in distribution.get('wheels', []):
            targets.add(wheel['target'])
    if not targets:
        raise ParityError('publish-artifacts manifest declares no release targets')
    return sorted(targets)


def published_packages(manifest: dict) -> list[dict]:
    """Every `publish = true` Rust package with its manifest path."""
    packages = []
    for entry in manifest.get('crates', []):
        if entry.get('publish') is not True:
            continue
        if not entry.get('package') or not entry.get('cargo_toml'):
            raise ParityError(f'published crate entry lacks package or cargo_toml: {entry}')
        packages.append({'package': entry['package'], 'cargo_toml': entry['cargo_toml']})
    names = [item['package'] for item in packages]
    if len(names) != len(set(names)):
        raise ParityError('publish-artifacts manifest lists a published crate twice')
    if not packages:
        raise ParityError('publish-artifacts manifest lists no published crates')
    return packages


def platform_target(manifest: dict, platform_id: str) -> str:
    """Rust target triple of a wheel platform id (the qualification matrix key)."""
    matches = {
        wheel['target']
        for distribution in manifest.get('python_distributions', [])
        for wheel in distribution.get('wheels', [])
        if wheel.get('id') == platform_id
    }
    if len(matches) != 1:
        raise ParityError(f'platform id {platform_id!r} maps to {sorted(matches)} release targets')
    return matches.pop()


def feature_closure(features: dict[str, list[str]], selected) -> frozenset[str]:
    """Crate-local features enabled by `selected`, as `cfg(feature = ...)` sees them."""
    resolved: set[str] = set()
    stack = list(selected)
    while stack:
        name = stack.pop()
        if name in resolved:
            continue
        if name not in features:
            raise ParityError(f'unknown feature {name!r}')
        resolved.add(name)
        for dependency in features[name]:
            if dependency.startswith('dep:'):
                continue
            if '/' in dependency:
                package, _, _ = dependency.partition('/')
                # `pkg/feat` also enables the optional dependency and, when the
                # crate declares a feature of that name, that feature.
                if not package.endswith('?') and package in features:
                    stack.append(package)
                continue
            stack.append(dependency)
    return frozenset(resolved)


def selection_id(resolved) -> str:
    return '+'.join(sorted(resolved)) if resolved else 'none'


def feature_selections(features: dict[str, list[str]]) -> list[dict]:
    """Every distinct crate-local feature set, each with the cargo flags that select it.

    The ordinary default invocation is always included. Explicit selections are
    deduplicated only when their resolved feature sets are identical.
    """
    for name, dependencies in features.items():
        if not isinstance(name, str) or not isinstance(dependencies, list):
            raise ParityError(f'malformed feature declaration {name!r}: {dependencies!r}')
    names = sorted(features)
    selections: dict[frozenset[str], dict] = {}
    default_key = feature_closure(features, ['default']) if 'default' in features else frozenset()
    selections[default_key] = {'id': selection_id(default_key), 'resolved': sorted(default_key), 'flags': []}
    for size in range(len(names) + 1):
        for combo in combinations(names, size):
            key = feature_closure(features, combo)
            if key in selections:
                continue
            flags = ['--no-default-features']
            if combo:
                flags.extend(['--features', ','.join(combo)])
            selections[key] = {'id': selection_id(key), 'resolved': sorted(key), 'flags': flags}
    ordered = sorted(selections.values(), key=lambda item: (len(item['resolved']), item['resolved']))
    ids = [item['id'] for item in ordered]
    if len(ids) != len(set(ids)):
        raise ParityError('feature selection ids collide')
    return ordered


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(command, cwd=cwd, text=True, capture_output=True,
                          env={**(env or os.environ), 'CARGO_TERM_COLOR': 'never'})


def package_library(cargo_toml: Path, root: Path = ROOT) -> dict:
    """Library target, crate types, version and declared features from Cargo metadata."""
    result = run(['cargo', 'metadata', '--format-version', '1', '--no-deps', '--locked',
                  '--manifest-path', str(cargo_toml)], cwd=root)
    if result.returncode != 0:
        raise ParityError(f'cargo metadata failed for {cargo_toml}: {result.stderr.strip()[-LOG_TAIL:]}')
    metadata = json.loads(result.stdout)
    wanted = cargo_toml.resolve()
    packages = [item for item in metadata['packages'] if Path(item['manifest_path']).resolve() == wanted]
    if len(packages) != 1:
        raise ParityError(f'{cargo_toml} does not identify exactly one package')
    package = packages[0]
    libraries = [target for target in package['targets'] if LIBRARY_KINDS & set(target['kind'])]
    if len(libraries) != 1:
        raise ParityError(f'{package["name"]} must define exactly one library target, found {len(libraries)}')
    library = libraries[0]
    return {
        'name': package['name'],
        'version': package['version'],
        'lib_name': library['name'].replace('-', '_'),
        'crate_types': sorted(library['crate_types']),
        'features': package['features'],
    }


def expected_inventory(root: Path = ROOT, manifest: dict | None = None) -> dict:
    """Complete expected (package, selection, target) set for this source checkout."""
    manifest = load_manifest(root / 'release/publish-artifacts.toml') if manifest is None else manifest
    packages = {}
    for entry in published_packages(manifest):
        library = package_library(root / entry['cargo_toml'], root)
        if library['name'] != entry['package']:
            raise ParityError(f'{entry["cargo_toml"]} names {library["name"]}, manifest says {entry["package"]}')
        packages[entry['package']] = {
            'cargo_toml': entry['cargo_toml'],
            'version': library['version'],
            'lib_name': library['lib_name'],
            'crate_types': library['crate_types'],
            'selections': feature_selections(library['features']),
        }
    return {'schema': CELL_SCHEMA, 'targets': release_targets(manifest), 'packages': packages}


# --- extraction --------------------------------------------------------------

def pinned_toolchain(path: Path = TOOLCHAIN_PATH) -> str:
    toolchain = path.read_text(encoding='utf-8').strip()
    if not toolchain.startswith('nightly-'):
        raise ParityError(f'{path} must pin an exact nightly toolchain, found {toolchain!r}')
    return toolchain


def source_commit(root: Path = ROOT) -> str:
    result = run(['git', 'rev-parse', 'HEAD'], cwd=root)
    if result.returncode != 0:
        raise ParityError('cannot determine the source commit')
    return result.stdout.strip()


def renderer_binary(renderer_dir: Path = RENDERER_DIR) -> Path:
    suffix = '.exe' if sys.platform == 'win32' else ''
    return renderer_dir / 'target/release' / f'public-api-surface-renderer{suffix}'


def ensure_renderer(renderer_dir: Path = RENDERER_DIR, root: Path = ROOT) -> Path:
    """Build the pinned renderer on the stable toolchain; its lockfile fixes the library versions."""
    result = run(['cargo', 'build', '--release', '--locked', '--manifest-path', str(renderer_dir / 'Cargo.toml')],
                 cwd=root)
    if result.returncode != 0:
        raise ParityError(f'renderer build failed: {result.stderr.strip()[-LOG_TAIL:]}')
    binary = renderer_binary(renderer_dir)
    if not binary.is_file():
        raise ParityError(f'renderer binary missing after build: {binary}')
    return binary


def render_rows(renderer: Path, rustdoc_json: Path, root: Path = ROOT) -> dict:
    """Run the renderer; returns its JSON document or raises ParityError with its diagnostics."""
    result = run([str(renderer), str(rustdoc_json)], cwd=root)
    if result.returncode != 0:
        raise ParityError(f'renderer exit {result.returncode}: {result.stderr.strip()[-LOG_TAIL:]}')
    try:
        rendering = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ParityError(f'renderer produced malformed JSON: {error}') from error
    if (rendering.get('schema') != 1 or not isinstance(rendering.get('rows'), list)
            or not isinstance(rendering.get('unresolved_item_ids'), list)):
        raise ParityError('renderer document lacks schema 1 rows')
    return rendering


def xwin_environment(target: str, base: dict[str, str] | None = None) -> dict[str, str]:
    """Environment for cross-documenting a Windows target from a non-Windows host via cargo-xwin."""
    result = run(['cargo', 'xwin', 'env', '--target', target], cwd=ROOT)
    if result.returncode != 0:
        raise ParityError(f'cargo xwin env failed for {target}: {result.stderr.strip()[-LOG_TAIL:]}')
    env = dict(os.environ if base is None else base)
    for line in result.stdout.splitlines():
        line = line.strip().rstrip(';')
        if not line.startswith('export '):
            continue
        parts = shlex.split(line[len('export '):])
        if len(parts) != 1 or '=' not in parts[0]:
            raise ParityError(f'unparseable cargo xwin env line: {line!r}')
        key, _, value = parts[0].partition('=')
        env[key] = value
    return env


def extraction_environment(base: dict[str, str] | None = None) -> dict[str, str]:
    env = dict(os.environ if base is None else base)
    # Rustdoc flags come only from RUSTDOC_ARGS so every producer renders the same way.
    env.pop('RUSTDOCFLAGS', None)
    env['CARGO_TERM_COLOR'] = 'never'
    return env


def rustdoc_output(target_dir: Path, target: str, lib_name: str, crate_types: list[str]) -> Path:
    # Procedural macros are compiled for the host and documented under the host doc directory.
    if 'proc-macro' in crate_types:
        return target_dir / 'doc' / f'{lib_name}.json'
    return target_dir / target / 'doc' / f'{lib_name}.json'


def extract_surface(*, package: str, cargo_toml: Path, lib_name: str, crate_types: list[str], target: str,
                    selection: dict, target_dir: Path, toolchain: str, renderer: Path, commit: str,
                    env: dict[str, str], root: Path = ROOT) -> dict:
    """Document one (package, selection, target) and render its public API rows into a cell."""
    command = ['cargo', f'+{toolchain}', 'rustdoc', '--locked', '--lib', '--manifest-path', str(cargo_toml),
               '--target', target, '--target-dir', str(target_dir), *selection['flags'], '--', *RUSTDOC_ARGS]
    cell = {
        'schema': CELL_SCHEMA, 'package': package, 'lib_name': lib_name, 'crate_types': list(crate_types),
        'target': target, 'selection': selection, 'source_commit': commit, 'toolchain': toolchain,
        'rustdoc_args': list(RUSTDOC_ARGS), 'renderer': None, 'host': platform.platform(),
        'command': command, 'status': 'extraction-error', 'rows': [], 'row_count': 0, 'rows_sha256': None,
        'unresolved_item_ids': [], 'external_item_ids': 0, 'log': '',
    }
    result = run(command, cwd=root, env=env)
    output = (result.stdout + result.stderr)
    cell['log'] = output[-LOG_TAIL:]
    if result.returncode != 0:
        return cell
    json_path = rustdoc_output(target_dir, target, lib_name, crate_types)
    if not json_path.is_file():
        cell['status'] = 'missing-output'
        cell['log'] = f'expected rustdoc JSON at {json_path}\n' + cell['log']
        return cell
    try:
        rendering = render_rows(renderer, json_path, root)
    except ParityError as error:
        cell['status'] = 'render-error'
        cell['log'] = f'{error}\n' + cell['log']
        return cell
    cell['renderer'] = rendering['renderer']
    cell['unresolved_item_ids'] = rendering['unresolved_item_ids']
    cell['external_item_ids'] = rendering['external_item_ids']
    cell['rows'] = rendering['rows']
    cell['row_count'] = len(rendering['rows'])
    cell['rows_sha256'] = rows_digest(rendering['rows'])
    if rendering['unresolved_item_ids']:
        cell['status'] = 'incomplete-extraction'
    elif not rendering['rows']:
        cell['status'] = 'empty-extraction'
    else:
        cell['status'] = 'ok'
        cell['log'] = ''
    return cell


def write_cell(output: Path, cell: dict) -> Path:
    path = output / EVIDENCE_DIRNAME / cell['target'] / cell['package'] / f"{cell['selection']['id']}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(cell, indent=1, sort_keys=True) + '\n', encoding='utf-8')
    return path


def collect(inventory: dict, targets: list[str], output: Path, *, packages: list[str] | None = None,
            selections: list[str] | None = None, target_dir: Path, toolchain: str, renderer: Path,
            commit: str, root: Path = ROOT, xwin: bool = False, log=None) -> list[dict]:
    """Extract every requested cell; returns the cells (failures included) after writing them."""
    unknown_targets = sorted(set(targets) - set(inventory['targets']))
    if unknown_targets:
        raise ParityError(f'targets {unknown_targets} are not release targets {inventory["targets"]}')
    wanted = inventory['packages'] if packages is None else {name: inventory['packages'][name] for name in packages}
    cells = []
    for target in targets:
        env = extraction_environment()
        if xwin and target.endswith('-pc-windows-msvc') and sys.platform != 'win32':
            env = extraction_environment(xwin_environment(target))
        producer = {'schema': CELL_SCHEMA, 'target': target, 'source_commit': commit, 'toolchain': toolchain,
                    'host': platform.platform(), 'xwin': bool(xwin), 'cells': []}
        for name, package in wanted.items():
            for selection in package['selections']:
                if selections is not None and selection['id'] not in selections:
                    continue
                cell = extract_surface(
                    package=name, cargo_toml=root / package['cargo_toml'], lib_name=package['lib_name'],
                    crate_types=package['crate_types'], target=target, selection=selection,
                    target_dir=target_dir, toolchain=toolchain, renderer=renderer, commit=commit, env=env,
                    root=root)
                path = write_cell(output, cell)
                producer['cells'].append({'package': name, 'selection': selection['id'], 'status': cell['status'],
                                          'row_count': cell['row_count'], 'path': str(path.relative_to(output))})
                cells.append(cell)
                if log is not None:
                    print(f"{target} {name} [{selection['id']}]: {cell['status']} rows={cell['row_count']}", file=log)
                    log.flush()
        producer_path = output / EVIDENCE_DIRNAME / target / 'producer.json'
        producer_path.parent.mkdir(parents=True, exist_ok=True)
        producer_path.write_text(json.dumps(producer, indent=1, sort_keys=True) + '\n', encoding='utf-8')
    return cells


# --- comparison --------------------------------------------------------------

def load_cells(evidence: Path) -> list[dict]:
    """Every cell under any `public-api-parity` directory; malformed documents fail."""
    paths = sorted(path for path in evidence.rglob('*.json')
                   if EVIDENCE_DIRNAME in path.parts and path.name != 'producer.json')
    if not paths:
        raise ParityError(f'no public API cells found under {evidence}')
    cells = []
    for path in paths:
        try:
            cell = json.loads(path.read_text(encoding='utf-8'))
        except (OSError, json.JSONDecodeError) as error:
            raise ParityError(f'malformed cell {path}: {error}') from error
        if not isinstance(cell, dict) or cell.get('schema') != CELL_SCHEMA:
            raise ParityError(f'cell {path} is not a schema {CELL_SCHEMA} document')
        missing = [key for key in REQUIRED_CELL_KEYS if key not in cell]
        if missing:
            raise ParityError(f'cell {path} lacks {missing}')
        if not isinstance(cell['rows'], list) or not all(isinstance(row, str) for row in cell['rows']):
            raise ParityError(f'cell {path} rows must be strings')
        cell['_path'] = str(path)
        cells.append(cell)
    return cells


def cell_key(cell: dict) -> tuple[str, str, str]:
    return cell['package'], cell['selection']['id'], cell['target']


def rows_digest(rows: list[str]) -> str:
    """Order-independent integrity check that preserves duplicate rows."""
    return hashlib.sha256(json.dumps(sorted(rows), ensure_ascii=True).encode()).hexdigest()


def row_differences(reference: list[str], candidate: list[str]) -> list[str]:
    """Rows whose multiplicity differs; `-` only in the reference, `+` only in the candidate."""
    before, after = Counter(reference), Counter(candidate)
    lines = []
    for row in sorted(set(before) | set(after)):
        delta = after[row] - before[row]
        if delta < 0:
            lines.append(f'-{abs(delta)}x {row}' if abs(delta) > 1 else f'- {row}')
        elif delta > 0:
            lines.append(f'+{delta}x {row}' if delta > 1 else f'+ {row}')
    return lines


def failure_summary(log: str, limit: int = 300) -> str:
    """One line for a failed cell: the last line mentioning an error, else the last non-empty line."""
    lines = [line.strip() for line in str(log).splitlines() if line.strip()]
    if not lines:
        return '(no log)'
    chosen = next((line for line in reversed(lines) if 'error' in line.lower()), lines[-1])
    return chosen if len(chosen) <= limit else chosen[:limit] + '...'


def assert_public_api_equal(cells: list[dict], expected: dict, *, commit: str | None = None) -> dict:
    """The single authoritative comparison; raises ParityError with every problem found."""
    problems: list[str] = []
    by_key: dict[tuple[str, str, str], dict] = {}
    for cell in cells:
        key = cell_key(cell)
        label = f"{key[0]} [{key[1]}] {key[2]}"
        if key in by_key:
            problems.append(f'duplicate cell for {label}: {cell.get("_path")} and {by_key[key].get("_path")}')
            continue
        by_key[key] = cell
        if cell['status'] != 'ok':
            problems.append(f'{label}: status {cell["status"]}: {failure_summary(cell.get("log", ""))}')
        elif not cell['rows']:
            problems.append(f'{label}: extraction produced no public API rows')
        if cell['status'] == 'ok' and 'rows_sha256' in cell:
            if cell.get('row_count') != len(cell['rows']) or cell['rows_sha256'] != rows_digest(cell['rows']):
                problems.append(f'{label}: row count or digest differs from recorded extraction')
        if cell.get('unresolved_item_ids'):
            problems.append(f'{label}: unresolved rustdoc item ids {cell["unresolved_item_ids"]}')

    identities = {
        'source_commit': {cell['source_commit'] for cell in cells},
        'toolchain': {cell['toolchain'] for cell in cells},
        'rustdoc_args': {tuple(cell['rustdoc_args']) for cell in cells},
        # Cells that failed before rendering carry no renderer identity; their status is already a problem.
        'renderer': {json.dumps(cell['renderer'], sort_keys=True) for cell in cells if cell['renderer'] is not None},
    }
    for name, values in identities.items():
        if len(values) != 1:
            problems.append(f'cells disagree on {name}: {sorted(map(str, values))}')
    if identities['toolchain'] != {pinned_toolchain()}:
        problems.append('cells do not use the pinned toolchain')
    if identities['rustdoc_args'] != {RUSTDOC_ARGS}:
        problems.append('cells do not use the required rustdoc flags')
    if commit is not None and identities['source_commit'] != {commit}:
        problems.append(f'cells were extracted from {sorted(identities["source_commit"])}, expected {commit}')

    expected_keys = {
        (name, selection['id'], target)
        for name, package in expected['packages'].items()
        for selection in package['selections']
        for target in expected['targets']
    }
    for key in sorted(expected_keys - set(by_key)):
        problems.append(f'missing cell for {key[0]} [{key[1]}] {key[2]}')
    for key in sorted(set(by_key) - expected_keys):
        problems.append(f'unexpected cell for {key[0]} [{key[1]}] {key[2]} (not in the release inventory)')
    for key in sorted(expected_keys & set(by_key)):
        cell, package = by_key[key], expected['packages'][key[0]]
        selection = next(item for item in package['selections'] if item['id'] == key[1])
        if cell['selection'].get('resolved') != selection['resolved'] or cell['selection'].get('flags') != selection['flags']:
            problems.append(f'{key[0]} [{key[1]}] {key[2]}: selection {cell["selection"]} differs from expected {selection}')
        if cell['crate_types'] != package['crate_types']:
            problems.append(f'{key[0]} [{key[1]}] {key[2]}: crate types differ from expected')
        if cell['lib_name'] != package['lib_name']:
            problems.append(f'{key[0]} [{key[1]}] {key[2]}: library {cell["lib_name"]} differs from {package["lib_name"]}')

    reference_target = expected['targets'][0]
    comparisons = 0
    if not problems:
        for name, package in expected['packages'].items():
            for selection in package['selections']:
                reference = by_key[(name, selection['id'], reference_target)]
                for target in expected['targets'][1:]:
                    candidate = by_key[(name, selection['id'], target)]
                    comparisons += 1
                    differences = row_differences(reference['rows'], candidate['rows'])
                    if differences:
                        problems.append(
                            f'{name} [{selection["id"]}]: public API differs between {reference_target} and {target}:\n  '
                            + '\n  '.join(differences))
    report = {
        'schema': CELL_SCHEMA,
        'source_commit': next(iter(identities['source_commit'])) if len(identities['source_commit']) == 1 else None,
        'toolchain': next(iter(identities['toolchain'])) if len(identities['toolchain']) == 1 else None,
        'targets': expected['targets'],
        'reference_target': reference_target,
        'packages': {name: {'selections': len(package['selections']),
                            'rows': {target: by_key[(name, package['selections'][0]['id'], target)]['row_count']
                                     for target in expected['targets']
                                     if (name, package['selections'][0]['id'], target) in by_key}}
                     for name, package in expected['packages'].items()},
        'cells': len(cells),
        'expected_cells': len(expected_keys),
        'comparisons': comparisons,
        'problems': problems,
        'status': 'equal' if not problems else 'failed',
    }
    if problems:
        raise ParityError('\n'.join(problems))
    return report


# --- command line -------------------------------------------------------------

def command_inventory(args) -> int:
    inventory = expected_inventory(ROOT)
    print(json.dumps(inventory, indent=1, sort_keys=True))
    return 0


def command_collect(args) -> int:
    manifest = load_manifest()
    inventory = expected_inventory(ROOT, manifest)
    targets = list(args.targets or [])
    if args.platform_id:
        targets.append(platform_target(manifest, args.platform_id))
    if not targets:
        raise ParityError('collect requires --target or --platform-id')
    unknown = sorted(set(args.packages or []) - set(inventory['packages']))
    if unknown:
        raise ParityError(f'packages {unknown} are not published crates')
    renderer = ensure_renderer()
    commit = source_commit()
    toolchain = pinned_toolchain()
    cells = collect(inventory, targets, args.output, packages=args.packages, target_dir=args.target_dir,
                    toolchain=toolchain, renderer=renderer, commit=commit, xwin=args.xwin, log=sys.stdout)
    failed = [cell for cell in cells if cell['status'] != 'ok']
    print(f'collected {len(cells)} cells for {targets} at {commit} with {toolchain}; failed={len(failed)}')
    for cell in failed:
        print(f"FAILED {cell['target']} {cell['package']} [{cell['selection']['id']}]: {cell['status']}\n{cell['log']}",
              file=sys.stderr)
    return 1 if failed else 0


def command_compare(args) -> int:
    expected = expected_inventory(ROOT)
    cells = load_cells(args.evidence)
    try:
        report = assert_public_api_equal(cells, expected, commit=args.source_commit)
    except ParityError as error:
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps({'status': 'failed', 'problems': str(error).splitlines()}, indent=1) + '\n')
        print(f'public API parity FAILED:\n{error}', file=sys.stderr)
        return 1
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, sort_keys=True) + '\n', encoding='utf-8')
    print(f"public API parity passed: {report['cells']} cells, {report['comparisons']} target comparisons across "
          f"{len(report['targets'])} targets and {len(report['packages'])} packages at {report['source_commit']}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('inventory', help='print the expected package/selection/target inventory').set_defaults(
        func=command_inventory)
    collect_parser = commands.add_parser('collect', help='extract public API surfaces for release targets')
    collect_parser.add_argument('--target', dest='targets', action='append', help='release target triple')
    collect_parser.add_argument('--platform-id', help='wheel platform id from release/publish-artifacts.toml')
    collect_parser.add_argument('--package', dest='packages', action='append', help='restrict to a published crate')
    collect_parser.add_argument('--output', type=Path, required=True, help='evidence directory to write')
    collect_parser.add_argument('--target-dir', type=Path, default=ROOT / 'target/public-api-parity')
    collect_parser.add_argument('--xwin', action='store_true',
                                help='document Windows targets from a non-Windows host through cargo-xwin')
    collect_parser.set_defaults(func=command_collect)
    compare_parser = commands.add_parser('compare', help='require identical public APIs across all release targets')
    compare_parser.add_argument('--evidence', type=Path, required=True, help='directory containing producer outputs')
    compare_parser.add_argument('--source-commit', help='full source SHA every cell must come from')
    compare_parser.add_argument('--report', type=Path, help='write the comparison report here')
    compare_parser.set_defaults(func=command_compare)
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except ParityError as error:
        print(f'error: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
