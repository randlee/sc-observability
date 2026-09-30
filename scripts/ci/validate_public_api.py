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


def major_breaks(policy: dict) -> list[dict]:
    """Read exact reviewed API changes; patterns and crate-wide waivers are unsupported."""
    path = ROOT / 'release/public-api-major-breaks.toml'
    manifest = tomllib.loads(path.read_text(encoding='utf-8'))
    if (manifest.get('schema_version') != 1
            or manifest.get('candidate_version') != policy['candidate_version']
            or manifest.get('baseline_version') != '1.4.1'):
        raise ValueError('major-break manifest must match candidate and frozen 1.4.1 baseline')
    entries = manifest.get('breaks', [])
    ids, removals = set(), set()
    for entry in entries:
        if (not all(isinstance(entry.get(key), str) and entry[key].strip()
                    for key in ('id', 'crate', 'old', 'reason', 'authority', 'migration'))
                or not isinstance(entry.get('new'), str)
                or entry['crate'] not in policy['crates']
                or entry['id'] in ids or (entry['crate'], entry['old']) in removals):
            raise ValueError('major-break manifest contains an invalid or duplicate entry')
        ids.add(entry['id'])
        removals.add((entry['crate'], entry['old']))
    return entries


def check_major_diff(crate: str, output: str, entries: list[dict]) -> list[str]:
    """Require exact old/new public-api lines for every removal or changed signature."""
    sections = ('Removed items from the public API', 'Changed items in the public API',
                'Added items to the public API')
    if not all(section in output for section in sections):
        raise ValueError('public-api output lacks required diff sections')
    removed = {line[1:] for line in output.splitlines() if line.startswith('-')}
    added = {line[1:] for line in output.splitlines() if line.startswith('+')}
    scoped = [entry for entry in entries if entry['crate'] == crate]
    listed = {entry['old'] for entry in scoped}
    problems = [f'unlisted API break: {item}' for item in sorted(removed - listed)]
    for entry in scoped:
        if entry['old'] not in removed:
            problems.append(f"stale break entry: {entry['id']}")
        elif entry['new'] and entry['new'] not in added:
            problems.append(f"replacement differs from reviewed break: {entry['id']}")
    return problems


def approved_structural_items(crate: str, entries: list[dict]) -> set[tuple[str, ...]]:
    """Project exact reviewed API lines into cargo-semver-checks diagnostic tuples."""
    approved = set()
    for entry in entries:
        if entry['crate'] != crate:
            continue
        old, new = entry['old'], entry['new']
        module = re.fullmatch(r'pub mod (sc_observability_otlp::(?:constants|error_codes))', old)
        constant = re.fullmatch(r'pub const sc_observability_otlp::(constants|error_codes)::([A-Z_]+): .+', old)
        trait = re.fullmatch(r'impl core::panic::unwind_safe::(RefUnwindSafe|UnwindSafe) for sc_observability_log::LogControl', old)
        if module and not new:
            approved.add(('module_missing', module[1]))
        elif constant and not new:
            approved.add(('pub_module_level_const_missing', constant[2], constant[1] + '.rs'))
        elif trait and new == old.replace('impl ', 'impl !', 1):
            approved.add(('auto_trait_impl_removed', 'LogControl', trait[1]))
        elif (old == 'pub struct sc_observability_otlp::OtelConfig'
              and new == '#[non_exhaustive] ' + old):
            approved.add(('struct_marked_non_exhaustive', 'OtelConfig'))
    return approved


def structural_diagnostics_are_enumerated(crate: str, output: str, entries: list[dict],
                                           returncode: int, stderr: str = '') -> bool:
    """Require exact coverage of every finding in real cargo-semver-checks stdout.

    Findings exit with status 1. Crashes/signals and build errors cannot become
    approved breaks, even when they also emit a complete approved diagnostic.
    Progress on stderr is kept separate from the strictly parsed finding rows.
    """
    approved = approved_structural_items(crate, entries)
    if returncode != 1 or not approved:
        return False
    if re.search(r"(?im)^\s*(?:error(?:\[|:)|could not compile|process didn't exit|command failed|thread .* panicked)", output + stderr):
        return False
    if re.search(r'(?m)^\s*(?:--- failure |Failed in:)', stderr):
        return False  # Findings belong on stdout; never ignore extra stderr rows.
    patterns = {
        'module_missing': r'  mod (sc_observability_otlp::[a-z_]+), previously in file .+:[0-9]+',
        'pub_module_level_const_missing': r'  ([A-Z_]+) in file .*/sc-observability-otlp-1\.4\.1/src/([^/]+\.rs):[0-9]+',
        'struct_marked_non_exhaustive': r'  struct ([A-Za-z_][A-Za-z_0-9]*) in .*/crates/sc-observability-otlp/src/config\.rs:[0-9]+',
        'auto_trait_impl_removed': r'  type ([A-Za-z_][A-Za-z_0-9]*) is no longer ([A-Za-z_][A-Za-z_0-9]*), in .*/crates/sc-observability-log/src/control\.rs:[0-9]+',
    }
    # Split only on complete, unindented headers. Any stray or malformed header
    # remains in a row/description and fails validation below.
    blocks = re.split(r'(?m)^--- failure ', output.strip())
    if blocks[0] or len(blocks) == 1:
        return False
    observed, rules = set(), set()
    for block in blocks[1:]:
        match = re.fullmatch(r'([a-z0-9_]+): [^\n]+ ---\n\nDescription:\n(.+?)\n\nFailed in:\n(.+?)\s*', block, re.DOTALL)
        if not match or match[1] not in patterns or match[1] in rules:
            return False
        rule, description, rows = match.groups()
        if '---' in description or 'Failed in:' in description:
            return False
        rules.add(rule)
        for row in rows.splitlines():
            item = re.fullmatch(patterns[rule], row)
            if not item:
                return False
            key = (rule, *item.groups())
            if key in observed or key not in approved:
                return False
            observed.add(key)
    return observed == approved


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
    entries = major_breaks(policy) if args.mode == 'semver' and any(
        item['baseline_version'] and item['baseline_version'].split('.')[0] != policy['candidate_version'].split('.')[0]
        for item in policy['crates'].values()) else []
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
        major_semver = args.mode == 'semver' and not initial and baseline.split('.')[0] != package['version'].split('.')[0]
        if major_semver and baseline != '1.4.1':
            raise ValueError('major release requires frozen 1.4.1 baseline for every crate')
        proc_macro_semver = args.mode == 'semver' and settings['kind'] == 'proc-macro' and not initial
        if args.mode == 'diff' or initial or proc_macro_semver or major_semver:
            command = ['cargo', 'public-api', '--manifest-path', package['manifest_path']]
            if not major_semver:
                command.append('-sss')
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
        elif major_semver:
            problems = check_major_diff(crate, result.stdout, entries)
            if problems:
                status, failure = 'unlisted-or-mismatched-major-break', True
                with (CACHE / log_name).open('a') as log:
                    log.write('\n' + '\n'.join(problems) + '\n')
            else:
                status = 'enumerated-major-breaks-verified'
                # The exact-line manifest complements the structural checker;
                # it must never waive a required-trait-item or other failure
                # that a text API diff alone may not classify as breaking.
                if settings['kind'] != 'proc-macro':
                    structural = run(['cargo', 'semver-checks', '--manifest-path',
                                      package['manifest_path'], '--baseline-version', baseline,
                                      '--release-type', 'minor', '--default-features'])
                    with (CACHE / log_name).open('a') as log:
                        log.write('\nStructural semver check:\n' + structural.stdout + structural.stderr)
                    # A small set of D18 structural diagnostics are the direct
                    # manifestation of enumerated compatibility retirement. Keep
                    # all other structural failures fatal: the textual diff
                    # cannot authorize an unrelated trait or layout break.
                    if structural.returncode:
                        if not structural_diagnostics_are_enumerated(
                                crate, structural.stdout, entries, structural.returncode, structural.stderr):
                            status, failure = 'structural-semver-failed', True
                        else:
                            with (CACHE / log_name).open('a') as log:
                                log.write('\nStructural failures match enumerated D18 compatibility breaks.\n')
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
