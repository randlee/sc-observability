"""Compare current compiled API artifacts; never invoke a build or rustdoc.

The normal unit runner supplies exact Cargo compiler-artifact messages only
AFTER Cargo exits. Committed snapshots are never used as the current surface.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from scripts.api.api_rows import row_differences


class ApiError(Exception):
    pass


def git(root, *args):
    result = subprocess.run(['git', *args], cwd=root, capture_output=True)
    if result.returncode:
        raise ApiError(result.stderr.decode(errors='replace').strip())
    return result.stdout


def accepted_base(value=None, root=ROOT):
    """Resolve a non-vacuous accepted-history revision.

    CI supplies the PR base or the previous pushed SHA. A run without either
    cannot prove history immutability and must fail rather than compare a
    candidate branch against its immediate parent.
    """
    candidate = value or os.environ.get('SC_API_ACCEPTED_BASE')
    if candidate == '0000000000000000000000000000000000000000':
        candidate = None
    if not candidate:
        raise ApiError('accepted API history requires a PR base or prior revision')
    try:
        resolved = git(root, 'rev-parse', '--verify', f'{candidate}^{{commit}}').decode().strip()
    except ApiError as error:
        raise ApiError('accepted API history requires a PR base or prior revision') from error
    current = git(root, 'rev-parse', '--verify', 'HEAD^{commit}').decode().strip()
    if resolved == current:
        raise ApiError('accepted API history base resolves to the candidate commit; supply a prior revision')
    return resolved


def accepted_history(accepted_base_value=None, root=ROOT):
    """Return immutable accepted snapshots after checking every tracked byte."""
    base = accepted_base(accepted_base_value, root)
    accepted = {}
    for name in git(root, 'ls-tree', '-r', '--name-only', base, '--', 'schema/api').decode().splitlines():
        if name.endswith('.json'):
            accepted[name] = git(root, 'show', f'{base}:{name}')
            path = root / name
            if not path.is_file() or path.read_bytes() != accepted[name]:
                raise ApiError(f'immutable accepted API history changed: {name} (baseline {base}); '
                               'retain it and add a new package version')
    return accepted


def source_fingerprint(root=ROOT):
    # Include untracked source inputs too; exclude generated Cargo outputs via
    # git's ignore rules. API history itself is the comparison target, not input
    # to compiling the runtime libraries.
    paths = git(root, 'ls-files', '-z', '--cached', '--others', '--exclude-standard').split(b'\0')
    digest = hashlib.sha256()
    for raw in sorted(set(paths)):
        if not raw or raw.startswith(b'schema/api/'):
            continue
        path = root / os.fsdecode(raw)
        digest.update(raw + b'\0')
        digest.update(path.read_bytes() if path.is_file() else b'<deleted>')
    return digest.hexdigest()


def catalog(root=ROOT):
    manifest = tomllib.loads((root / 'release/publish-artifacts.toml').read_text())
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']
    result = {}
    for item in manifest['crates']:
        if item.get('publish') is not True:
            continue
        path = root / item['cargo_toml']
        cargo = tomllib.loads(path.read_text())
        package = cargo['package']
        version = package['version']
        if isinstance(version, dict):
            version = workspace['version']
        result[str(path.resolve())] = {
            'package': package['name'], 'version': version,
            'lib_name': cargo.get('lib', {}).get('name', package['name'].replace('-', '_')),
        }
    return result


def digest_file(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def collect_artifacts(messages, root=ROOT, prior_reader=None):
    published = catalog(root)
    entries = {}
    helper = prior_reader['helper'] if prior_reader else None
    fixture = None
    for message in messages:
        if message.get('reason') != 'compiler-artifact':
            continue
        target = message['target']
        if target['name'] == 'sc_observability_api_test' and message['profile']['test']:
            helper = message['executable']
        paths = [name for name in message['filenames'] if name.endswith('.rlib')]
        if not paths:
            paths = [name for name in message['filenames'] if name.endswith('.rmeta')]
        if 'proc-macro' in target['kind']:
            dynamic = [name for name in message['filenames'] if Path(name).suffix in ('.so', '.dylib', '.dll')]
            paths = dynamic or paths
        if message['profile']['test'] or not paths:
            continue
        if len(paths) != 1:
            raise ApiError(f'ambiguous library outputs for {target["name"]}: {paths}')
        path = str(Path(message['manifest_path']).resolve())
        entry = {**published.get(path, {}), 'lib_name': target['name'], 'artifact': paths[0],
                 'features': sorted(message['features']), 'artifact_sha256': digest_file(paths[0])}
        if target['name'] == 'sc_api_mutation_fixture':
            fixture = entry
        if path in published:
            key = (entry['package'], tuple(entry['features']))
            previous = entries.get(key)
            if previous and previous['artifact'] != entry['artifact']:
                raise ApiError(f'multiple active artifacts for {key}; split distinct compilation configurations')
            entries[key] = entry
    if prior_reader and digest_file(prior_reader['helper']) != prior_reader['helper_sha256']:
        raise ApiError('release-cut reader changed since its normal unit build')
    if not helper or not Path(helper).is_file():
        raise ApiError('normal workspace unit build did not produce the API reader test executable')
    if not entries:
        raise ApiError('normal workspace unit build supplied no published library artifacts')
    return {'entries': sorted(entries.values(), key=lambda e: (e['package'], e['features'])),
            'helper': helper, 'helper_sha256': digest_file(helper), 'fixture': fixture}


def compiler_environment(sysroot):
    env = dict(os.environ)
    # Windows needs the compiler DLL directories. Preserve Unix executable
    # resolution, including rustup proxies, for the ordinary Cargo command.
    if sys.platform == 'win32':
        env['PATH'] = os.pathsep.join([str(Path(sysroot) / 'bin'),
                                     str(Path(sysroot) / 'lib'), env.get('PATH', '')])
    return env


def read_surfaces(record):
    entries = record['entries'] + ([record['fixture']] if record.get('fixture') else [])
    for entry in entries:
        if digest_file(entry['artifact']) != entry['artifact_sha256']:
            raise ApiError(f'built artifact changed after unit build: {entry["artifact"]}')
    if digest_file(record['helper']) != record['helper_sha256']:
        raise ApiError('metadata reader changed after unit build')
    env = compiler_environment(record['sysroot'])
    env['SC_API_READER_REQUEST'] = json.dumps(entries)
    result = subprocess.run([record['helper'], '--ignored', '--exact', 'metadata_dump', '--nocapture'],
                            env=env, text=True, capture_output=True, timeout=55)
    if result.returncode:
        raise ApiError(f'compiled metadata inspection failed:\n{result.stdout}\n{result.stderr}')
    surfaces = {}
    for line in result.stdout.splitlines():
        if line.startswith('SC_API_ROWS='):
            data = json.loads(line.removeprefix('SC_API_ROWS='))
            if data['artifact'] in surfaces or not data['rows']:
                raise ApiError('duplicate or empty metadata-reader surface')
            surfaces[data['artifact']] = data['rows']
    if set(surfaces) != {entry['artifact'] for entry in entries}:
        raise ApiError('metadata reader did not return every requested artifact')
    return surfaces


def assert_mutations(rows):
    prefix = 'sc_api_mutation_fixture::'
    def selected(module):
        return sorted(row.replace(prefix + module, prefix + 'selected') for row in rows
                      if prefix + module + '::' in row and not row.startswith(('impl ', 'bound impl ')))
    if any('NotExported' in row for row in rows):
        raise ApiError('private trait leaked into the public API projection')
    baseline = selected('baseline')
    for module, marker in [('signature', 'signature '), ('field', 'field '),
                           ('trait_bound', 'bound '), ('visibility', 'Alias')]:
        changed = row_differences(baseline, selected(module))
        if not any(marker in row for row in changed):
            raise ApiError(f'compiled mutation fixture did not detect {module}')
    if not any(row.startswith('auto ' + prefix + 'auto_trait::Value !') and row.endswith('::Send') for row in rows):
        raise ApiError('compiled auto-trait fixture did not detect loss of Send')


def encode_families(families):
    # A canonical shared row table avoids repeating the same API 18 times in
    # feature-family history. Family indices are derived data, never selectors.
    rows = sorted({row for family_rows in families.values() for row in family_rows})
    indices = {row: index for index, row in enumerate(rows)}
    return {'rows': rows, 'families': {name: [indices[row] for row in sorted(set(values))]
                                     for name, values in sorted(families.items())}}


def decode_families(document):
    rows = document.get('rows')
    families = document.get('families')
    if not isinstance(rows, list) or not all(isinstance(row, str) for row in rows) or rows != sorted(set(rows)):
        raise ApiError('malformed canonical API row table')
    if not isinstance(families, dict):
        raise ApiError('malformed API configuration families')
    result = {}
    for name, indices in families.items():
        if (not isinstance(name, str) or not isinstance(indices, list)
                or not all(type(index) is int and 0 <= index < len(rows) for index in indices)
                or indices != sorted(set(indices))):
            raise ApiError(f'malformed API row indices for {name}')
        result[name] = [rows[index] for index in indices]
    return result


def snapshot_path(entry, root=ROOT):
    return root / 'schema/api' / ('rust-' + entry['package']) / (entry['version'] + '.json')


def family(entry):
    return '+'.join(entry['features']) or 'none'


def check_current(record, accepted_base, root=ROOT, capture=False):
    started = time.monotonic()
    if source_fingerprint(root) != record['source_sha256']:
        raise ApiError('source changed since the completed unit build; rerun normal unit tests')
    # One shared check rejects code + snapshot edits at one version for every
    # API surface before any runtime-specific comparison proceeds.
    accepted = accepted_history(accepted_base, root)
    t = time.monotonic()
    surfaces = read_surfaces(record)
    inspection = time.monotonic() - t
    if record.get('fixture'):
        assert_mutations(surfaces[record['fixture']['artifact']])
    updates = {}
    problems = []
    for entry in record['entries']:
        path = snapshot_path(entry, root)
        selected = family(entry)
        current = sorted(surfaces[entry['artifact']])
        expected = updates.get(path)
        if expected is None:
            expected = json.loads(path.read_text()) if path.exists() else {
                'schema': 1, 'package': entry['package'], 'version': entry['version'],
                'format': 'rustc-1.94.1-metadata/v1', 'families': {}, 'rows': []}
        if (expected.get('schema'), expected.get('package'), expected.get('version'), expected.get('format')) != (
                1, entry['package'], entry['version'], 'rustc-1.94.1-metadata/v1'):
            raise ApiError(f'malformed API snapshot: {path}')
        decoded = decode_families(expected)
        previous = decoded.get(selected)
        differences = row_differences(previous or [], current)
        if differences or previous is None:
            label = f'{entry["package"]} {entry["version"]} [{selected}]'
            if capture:
                if path.relative_to(root).as_posix() in accepted:
                    raise ApiError(f'{label}: accepted snapshot cannot be overwritten; increment the package version')
                decoded[selected] = current
                expected.update(encode_families(decoded))
                updates[path] = expected
            else:
                problems.append(f'{label}: API differs\n' + '\n'.join(differences[:25]) +
                                '\nIntentional changes require a new manifest version and release-cut snapshot; retain accepted history.')
    if problems:
        raise ApiError('\n'.join(problems))
    if source_fingerprint(root) != record['source_sha256']:
        raise ApiError('source changed during API verification')
    for path, snapshot in updates.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(snapshot, indent=2, sort_keys=True) + '\n')
    total = time.monotonic() - started
    if total >= 60:
        raise ApiError(f'API check exceeded 60 seconds: {total:.3f}s')
    return {'libraries': len(record['entries']), 'inspection_seconds': round(inspection, 3),
            'total_seconds': round(total, 3), 'builds_in_check': 0,
            'covered_families': [f'{e["package"]}[{family(e)}]' for e in record['entries']]}


def main():
    """Small cross-language boundary for history and row-diff consumers."""
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--accepted-base')
    parser.add_argument('--check-accepted-history', action='store_true')
    parser.add_argument('--row-differences', action='store_true')
    args = parser.parse_args()
    if args.check_accepted_history == args.row_differences:
        parser.error('choose exactly one operation')
    try:
        if args.check_accepted_history:
            print(json.dumps(sorted(accepted_history(args.accepted_base))))
        else:
            payload = json.load(sys.stdin)
            print(json.dumps(row_differences(payload['reference'], payload['candidate'])))
    except (ApiError, KeyError, TypeError, json.JSONDecodeError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
