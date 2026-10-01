#!/usr/bin/env python3
"""Crate-scoped API reports; approvals never waive execution or semver failures."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import tomllib
import subprocess
import sys
import traceback
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / 'target/public-api'

# This is the sole approved exception to workspace API-policy qualification.
# Keep the inventory metadata exact so the release gate cannot turn the
# standalone Tauri deferral into a general exclusion mechanism.
APPROVED_DEFERRED_STANDALONE_PACKAGES = [
    {
        'package': 'sc-observability-tauri',
        'baselineVersion': '1.4.1',
        'reason': (
            'The Tauri adapter is a separate workspace and remains pending its '
            'standalone API/publication qualification in '
            'release/bindings-artifacts.toml; it is not one of this candidate\'s '
            'nine workspace API packages.'
        ),
    },
]


def approval_for(crate: str, version: str, directory: Path, api_sha256: str) -> bool:
    for path in directory.glob('*.json'):
        record = json.loads(path.read_text(encoding='utf-8'))
        scoped = record.get('crates', {}).get(crate, {})
        if (record.get('schema_version') == 1 and record.get('candidate_version') == version
                and scoped.get('status') == 'approved' and scoped.get('reviewer')
                and scoped.get('evidence') and 'public-api' in scoped.get('scope', [])
                and scoped.get('api_sha256') == api_sha256):
            return True
    return False


def run(command: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True,
                          env={**os.environ, "CARGO_TERM_COLOR": "never"})


def registry_absent(crate: str) -> bool:
    request = urllib.request.Request(f'https://crates.io/api/v1/crates/{crate}', headers={'User-Agent': 'sc-observability-api-qualification/1.5.0'})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            if response.status != 200:
                raise ValueError(f'{crate}: unexpected registry response {response.status}')
            return False
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return True
        raise


def validate_api_package_roster(policy: dict, inventory: dict, publish_artifacts: dict) -> None:
    """Require workspace candidates plus the exact approved deferral to cover every published crate."""
    candidate = inventory.get('qualificationCandidate')
    if not isinstance(candidate, dict):
        raise ValueError('release inventory must define qualificationCandidate')

    candidate_names = candidate.get('packages')
    deferred = candidate.get('deferredStandalonePackages')
    artifact_crates = publish_artifacts.get('crates')
    if not isinstance(candidate_names, list) or any(
            not isinstance(name, str) or not name for name in candidate_names):
        raise ValueError('qualificationCandidate.packages must be a list of package names')
    if not isinstance(deferred, list) or any(not isinstance(item, dict) for item in deferred):
        raise ValueError(
            'qualificationCandidate.deferredStandalonePackages must be a list of package records'
        )
    deferred_names = [item.get('package') for item in deferred]
    if any(not isinstance(name, str) or not name for name in deferred_names):
        raise ValueError('deferred standalone API package records must name a package')
    if not isinstance(artifact_crates, list) or any(not isinstance(item, dict) for item in artifact_crates):
        raise ValueError('publish-artifacts manifest must define a crates list')
    published_names = [item.get('package') for item in artifact_crates]
    if any(not isinstance(name, str) or not name for name in published_names):
        raise ValueError('every publish-artifacts crate must name a package')

    all_names = candidate_names + deferred_names
    if len(candidate_names) != len(set(candidate_names)):
        raise ValueError('qualificationCandidate.packages contains duplicate packages')
    if len(deferred_names) != len(set(deferred_names)):
        raise ValueError('qualificationCandidate.deferredStandalonePackages contains duplicate packages')
    if len(published_names) != len(set(published_names)):
        raise ValueError('publish-artifacts manifest contains duplicate crate packages')
    if len(all_names) != len(set(all_names)):
        raise ValueError('candidate and deferred API package sets overlap')
    if deferred != APPROVED_DEFERRED_STANDALONE_PACKAGES:
        raise ValueError('deferred standalone API package metadata differs from the exact approved exemption')

    policy_names = policy.get('crates')
    if not isinstance(policy_names, dict) or set(policy_names) != set(candidate_names):
        raise ValueError('public API policy must name exactly the qualification candidate packages')
    if set(all_names) != set(published_names):
        missing = sorted(set(published_names) - set(all_names))
        unknown = sorted(set(all_names) - set(published_names))
        raise ValueError(
            'candidate and deferred API package sets must exactly match publish-artifacts crates '
            f'(omitted={missing}, unknown={unknown})'
        )


def validate_compatible_policy(policy: dict, root: Path | None = None) -> None:
    """Fail closed unless the active candidate is a compatible 1.x release."""
    root = ROOT if root is None else root
    path = root / 'release/public-api-major-breaks.toml'
    manifest = tomllib.loads(path.read_text(encoding='utf-8'))
    inventory = json.loads((root / 'release/release-inventory.json').read_text(encoding='utf-8'))
    publish_artifacts = tomllib.loads((root / 'release/publish-artifacts.toml').read_text(encoding='utf-8'))
    validate_api_package_roster(policy, inventory, publish_artifacts)
    candidate = policy.get('candidate_version', '')
    if (policy.get('schema_version') != 1
            or manifest.get('schema_version') != 1
            or not isinstance(candidate, str)
            or manifest.get('candidate_version') != candidate
            or manifest.get('baseline_version') != '1.4.1'
            or not re.fullmatch(r'1\.\d+\.\d+', candidate)
            or tuple(map(int, candidate.split('.'))) <= (1, 4, 1)):
        raise ValueError('compatible API policy and break manifest must select a release after the frozen 1.4.1 baseline')
    if not policy.get('crates') or any(
            settings.get('baseline_version') != '1.4.1'
            for settings in policy['crates'].values()):
        raise ValueError('every published crate requires the actual frozen 1.4.1 baseline')
    entries = manifest.get('breaks')
    if not isinstance(entries, list) or entries:
        raise ValueError('compatible 1.x release cannot accept enumerated breaking API exceptions')
    return None


def compatible_diff_problems(output: str) -> list[str]:
    """Allow additive public-api rows while rejecting removals and signature changes."""
    sections = ('Removed items from the public API', 'Changed items in the public API',
                'Added items to the public API')
    if not all(section in output for section in sections):
        raise ValueError('public-api output lacks required diff sections')
    section = None
    problems = []
    for line in output.splitlines():
        if line in sections:
            section = line
        elif line.startswith('-'):
            problems.append(f'incompatible API removal or change: {line[1:]}')
        elif line.startswith('+') and section != 'Added items to the public API':
            problems.append(f'incompatible API signature change: {line[1:]}')
    return problems


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=('diff', 'semver', 'docs'))
    parser.add_argument('--crate', action='append', dest='crates',
                        help='Qualify only named crates; never a whole-workspace approval')
    args = parser.parse_args()
    policy = json.loads((ROOT / 'release/public-api-policy.json').read_text(encoding='utf-8'))
    metadata_result = run(['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'])
    metadata_result.check_returncode()
    metadata = json.loads(metadata_result.stdout)
    public = {p['name']: p for p in metadata['packages'] if p.get('publish') != []}
    if set(public) != set(policy['crates']):
        raise ValueError('public API policy must name every public crate and exclude private crates')
    if any(package['version'] != policy['candidate_version'] for package in public.values()):
        raise ValueError('API policy candidate version differs from workspace')
    if args.crates and not set(args.crates) <= set(public):
        raise ValueError('selected crate is not in public API policy')
    if args.crates and args.mode == 'docs':
        raise ValueError('documentation approval requires the full crate report')
    if args.mode in ('semver', 'docs'):
        validate_compatible_policy(policy)
    CACHE.mkdir(parents=True, exist_ok=True)
    if args.mode == 'docs':
        report = json.loads((CACHE / 'public-api-diff.json').read_text(encoding='utf-8'))
        head = run(['git', 'rev-parse', 'HEAD']).stdout.strip()
        if report.get('source_commit') != head or report.get('candidate_version') != policy['candidate_version']:
            raise ValueError('API diff report is stale; rerun diff at this source revision')
        if set(report.get('crates', {})) != set(policy['crates']):
            raise ValueError('API diff report omits a required crate')
        missing = [crate for crate, item in report['crates'].items()
                   if item['status'] in ('changed', 'initial-public-api')
                   and not approval_for(crate, policy['candidate_version'], ROOT / 'docs/api-approvals', item['api_sha256'])]
        failed = [crate for crate, item in report['crates'].items() if item['status'] == 'tool-error']
        if failed or missing:
            print(f'API documentation gate: tool_errors={failed}, missing_scoped_approvals={missing}', file=sys.stderr)
            return 1
        print('public API docs validation passed (all affected crates explicitly approved)')
        return 0
    report = {'source_commit': run(['git', 'rev-parse', 'HEAD']).stdout.strip(),
              'candidate_version': policy['candidate_version'], 'crates': {}}
    failure, changes = False, False
    lines = []
    for crate, settings in policy['crates'].items():
        if args.crates and crate not in args.crates:
            continue
        baseline = settings['baseline_version']
        package = public[crate]
        if not any(settings['kind'] in target['kind'] for target in package['targets']):
            raise ValueError(f'{crate}: public target kind differs from policy')
        initial = baseline is None
        if initial and not registry_absent(crate):
            raise ValueError(f'{crate}: initial-release declaration conflicts with registry; set published baseline')
        proc_macro_semver = args.mode == 'semver' and settings['kind'] == 'proc-macro' and not initial
        if args.mode == 'diff' or initial or proc_macro_semver:
            command = ['cargo', 'public-api', '--manifest-path', package['manifest_path']]
            command.append('-sss')
            if not initial:
                command.extend(['diff', baseline])
        else:
            same_minor = baseline.split('.')[:2] == package['version'].split('.')[:2]
            command = ['cargo', 'semver-checks', '--manifest-path', package['manifest_path'], '--baseline-version', baseline, '--release-type', 'patch' if same_minor else 'minor', '--all-features']
        result = run(command)
        output = result.stdout + result.stderr
        log_name = f'{crate}-{args.mode}.log'
        (CACHE / log_name).write_text(output)
        status = 'passed'
        if result.returncode != 0:
            status, failure = 'tool-error', True
        elif proc_macro_semver:
            # cargo-semver-checks does not qualify proc-macro targets. Compare
            # against the published API and allow additions, while rejecting
            # every removal and changed signature.
            try:
                problems = compatible_diff_problems(result.stdout)
            except ValueError:
                status, failure = 'tool-error', True
            else:
                if problems:
                    status, failure = 'proc-macro-api-incompatible', True
                    with (CACHE / log_name).open('a') as log:
                        log.write('\n' + '\n'.join(problems) + '\n')
                else:
                    status = 'compatible-proc-macro-api'
        elif initial:
            if not result.stdout.strip():
                status, failure = 'tool-error', True
            else:
                status = 'initial-public-api' if args.mode == 'diff' else 'initial-release-no-published-baseline'
                changes = True
        elif args.mode == 'diff':
            # cargo-public-api returns zero for a successful diff, including additions.
            # Empty sections explicitly report '(none)'; any actual +/- line is a change.
            status = 'changed' if any(line.startswith(('+', '-')) for line in result.stdout.splitlines()) else 'unchanged'
            changes |= status == 'changed'
        report['crates'][crate] = {'status': status, 'baseline_version': baseline, 'kind': settings['kind'],
                                   'command': command, 'exit_code': result.returncode, 'log': log_name,
                                   'api_sha256': hashlib.sha256(result.stdout.encode()).hexdigest()}
        lines.append(f'{crate}: {status} (baseline={baseline}, exit={result.returncode}, log={log_name})')
    (CACHE / f'public-api-{args.mode}.json').write_text(json.dumps(report, indent=2) + '\n')
    (CACHE / f'public-api-{args.mode}.txt').write_text('\n'.join(lines) + '\n')
    print('\n'.join(lines))
    if failure:
        return 2  # Cannot be mistaken for an actionable API diff or waived by approvals.
    if args.mode == 'diff' and changes:
        print('public API diff report generated (diffs detected; crate-scoped review required)')
        return 1
    print(f'public API {args.mode} validation passed' + (' (selected crates only)' if args.crates else ''))
    return 0


def cli() -> int:
    """Run the validator and distinguish crashes from actionable API diffs."""
    try:
        return main()
    except Exception:
        traceback.print_exc()
        return 3


if __name__ == '__main__':
    raise SystemExit(cli())
