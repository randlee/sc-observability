#!/usr/bin/env python3
"""Crate-scoped API reports; approvals never waive execution or semver failures."""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / 'target/public-api'


def approval_for(crate: str, version: str, directory: Path, api_sha256: str) -> bool:
    for path in directory.glob('*.json'):
        record = json.loads(path.read_text())
        scoped = record.get('crates', {}).get(crate, {})
        if (record.get('schema_version') == 1 and record.get('candidate_version') == version
                and scoped.get('status') == 'approved' and scoped.get('reviewer')
                and scoped.get('evidence') and 'public-api' in scoped.get('scope', [])
                and scoped.get('api_sha256') == api_sha256):
            return True
    return False


def run(command: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True)


def registry_absent(crate: str) -> bool:
    request = urllib.request.Request(f'https://crates.io/api/v1/crates/{crate}', headers={'User-Agent': 'sc-observability-api-qualification/1.4.0'})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            if response.status != 200:
                raise ValueError(f'{crate}: unexpected registry response {response.status}')
            return False
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return True
        raise


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=('diff', 'semver', 'docs'))
    args = parser.parse_args()
    policy = json.loads((ROOT / 'release/public-api-policy.json').read_text())
    metadata_result = run(['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'])
    metadata_result.check_returncode()
    metadata = json.loads(metadata_result.stdout)
    public = {p['name']: p for p in metadata['packages'] if p.get('publish') != []}
    if set(public) != set(policy['crates']):
        raise ValueError('public API policy must name every public crate and exclude private crates')
    if any(package['version'] != policy['candidate_version'] for package in public.values()):
        raise ValueError('API policy candidate version differs from workspace')
    CACHE.mkdir(parents=True, exist_ok=True)
    if args.mode == 'docs':
        report = json.loads((CACHE / 'public-api-diff.json').read_text())
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
        baseline = settings['baseline_version']
        package = public[crate]
        if not any(settings['kind'] in target['kind'] for target in package['targets']):
            raise ValueError(f'{crate}: public target kind differs from policy')
        initial = baseline is None
        if initial and not registry_absent(crate):
            raise ValueError(f'{crate}: initial-release declaration conflicts with registry; set published baseline')
        proc_macro_semver = args.mode == 'semver' and settings['kind'] == 'proc-macro' and not initial
        if args.mode == 'diff' or initial or proc_macro_semver:
            command = ['cargo', 'public-api', '--manifest-path', package['manifest_path'], '-sss']
            if not initial:
                command.extend(['diff', baseline])
        else:
            same_minor = baseline.split('.')[:2] == package['version'].split('.')[:2]
            command = ['cargo', 'semver-checks', '--manifest-path', package['manifest_path'], '--baseline-version', baseline, '--release-type', 'patch' if same_minor else 'minor', '--default-features']
        result = run(command)
        output = result.stdout + result.stderr
        log_name = f'{crate}-{args.mode}.log'
        (CACHE / log_name).write_text(output)
        status = 'passed'
        if result.returncode != 0:
            status, failure = 'tool-error', True
        elif proc_macro_semver:
            # cargo-semver-checks rejects proc-macro targets. Require an unchanged
            # published export surface instead; never silently skip this crate.
            sections = ('Removed items from the public API', 'Changed items in the public API', 'Added items to the public API')
            if not all(section in result.stdout for section in sections):
                status, failure = 'tool-error', True
            elif any(line.startswith(('+', '-')) for line in result.stdout.splitlines()):
                status, failure = 'proc-macro-api-changed', True
            else:
                status = 'published-proc-macro-api-unchanged'
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
    print(f'public API {args.mode} validation passed')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
