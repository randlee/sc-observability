#!/usr/bin/env python3
"""Release-cut setup ONLY: build configuration families and capture API history.

Never called by the unit checker or CI. Ordinary checks consume already-built
artifacts; this explicitly invoked publishing setup may compile all families.
"""
import argparse
import json
import subprocess
import sys
import time

from history import ApiError, ROOT, check_current, collect_artifacts, source_fingerprint
from scripts.ci.public_api_parity import expected_inventory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader-record', type=str, default='target/api/current-build.json')
    parser.add_argument('--accepted-base', required=True)
    parser.add_argument('--package', action='append', help='optional release-cut subset; report partial coverage')
    args = parser.parse_args()
    prior = json.loads((ROOT / args.reader_record).read_text())
    if prior['source_sha256'] != source_fingerprint():
        raise ApiError('build the API helper with current normal workspace unit setup first')
    inventory = expected_inventory()
    names = args.package or list(inventory['packages'])
    if set(names) - set(inventory['packages']):
        parser.error('unknown published package')
    started = time.monotonic()
    results = []
    for name in names:
        package = inventory['packages'][name]
        for selection in package['selections']:
            before = source_fingerprint()
            operation = 'build' if 'proc-macro' in package['crate_types'] else 'check'
            command = ['cargo', operation, '--locked', '--lib', '--manifest-path',
                       str(ROOT / package['cargo_toml']), *selection['flags'],
                       '--message-format=json-render-diagnostics']
            t = time.monotonic()
            completed = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            setup = time.monotonic() - t
            if completed.returncode:
                raise ApiError(f'{name}[{selection["id"]}] release-cut setup failed:\n{completed.stderr}')
            if source_fingerprint() != before:
                raise ApiError('source changed during release-cut setup')
            messages = [json.loads(line) for line in completed.stdout.splitlines() if line.startswith('{')]
            record = collect_artifacts(messages, prior_reader=prior)
            # Dependencies may have different feature unions during this setup.
            # Capture the requested package; each dependency has its own family
            # iteration in the manifest-driven release inventory.
            record['entries'] = [e for e in record['entries'] if e['package'] == name]
            if len(record['entries']) != 1:
                raise ApiError(f'{name}: expected one selected built library')
            record.update(source_sha256=before, sysroot=prior['sysroot'])
            checked = check_current(record, args.accepted_base, capture=True)
            row = {'package': name, 'requested_family': selection['id'], 'setup_seconds': round(setup, 3), **checked}
            results.append(row)
            print(json.dumps(row), flush=True)
    print(json.dumps({'release_cut_seconds': round(time.monotonic() - started, 3),
                      'families_captured': len(results),
                      'declared_families': sum(len(p['selections']) for p in inventory['packages'].values())}))


if __name__ == '__main__':
    try:
        main()
    except (ApiError, OSError, subprocess.SubprocessError) as error:
        print(f'release-cut API capture FAILED: {error}', file=sys.stderr)
        raise SystemExit(1)
