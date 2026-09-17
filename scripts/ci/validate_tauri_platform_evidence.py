#!/usr/bin/env python3
"""Reject incomplete, stale, or tampered real-Tauri qualification evidence."""
import argparse
import hashlib
import json
from pathlib import Path
from validate_binding_runtime import cases as native_cases, source_digest as native_source_digest

REQUIRED_MAIN = {
    'factory', 'try-log', 'log', 'flush', 'query', 'health',
    'correlated-rust-frontend', 'bigint-exact-query', 'recursive-redaction',
    'trusted-typescript-provenance', 'trusted-rust-provenance',
    'elevate-trace', 'repeat-unchanged', 'reduce-debug', 'below-baseline',
    'off-below-baseline', 'reset', 'level-forged-source', 'level-invalid-tag',
    'direct-denied-target', 'direct-denied-query', 'oversized-request',
    'deep-request', 'cyclic-value', 'getter-value', 'no-hidden-rejection',
    'minimal-exact-64k-normalization', 'exact-64k-request', 'one-byte-oversize-request',
    'bridge-health-coherence', 'owner-contention-queue-full', 'owner-contention-state-preserved',
    'actual-native-queue-full', 'full-diagnostic-preserves-change', 'actual-flush-slot-full',
    'host-responsive-during-blocked-io', 'query-timeout-and-overlap',
    'frontend-heartbeat-during-blocked-io', 'query-slot-retained-after-timeout',
    'native-flush-late-completion', 'actual-flush-zero-timeout',
    'actual-flush-after-timeout-overlap',
    'shutdown-contention-queue-full', 'level-during-shutdown-closed',
    'shutdown-level-state-preserved', 'admission-during-shutdown-closed',
    'native-shutdown-pending', 'native-shutdown-completed',
    'level-after-shutdown-closed', 'post-shutdown-health-retained',
}
REQUIRED_FORBIDDEN = {'forbidden-window-' + name for name in ('try_log', 'query', 'health', 'flush', 'level')}
REQUIRED_CAPPED = {'capped-baseline', 'capped-level-rejected', 'capped-payload-preserved', 'capped-state-preserved', 'capped-bridge-coherence', 'capped-accepted', 'capped-filtered', 'capped-flush', 'capped-no-hidden-rejection'}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(root, source=None):
    found = {}
    for path in root.rglob('platform.json'):
        report = json.loads(path.read_text(encoding='utf-8'))
        name = report['platform']
        if name in found:
            raise ValueError('duplicate platform evidence: ' + name)
        if report.get('status') != 'passed' or report.get('schema_version') != 1 or report.get('source_dirty') is not False:
            raise ValueError('failed or incompatible platform: ' + name)
        if source and report['source_commit'] != source:
            raise ValueError('source revision mismatch: ' + name)
        directory = path.parent
        def hashed(relative, expected):
            candidate = (directory / relative).resolve()
            if not candidate.is_relative_to(directory.resolve()) or digest(candidate) != expected:
                raise ValueError('artifact hash/path mismatch: ' + str(relative))
            return candidate
        hashed('bundle-manifest.json', report['bundle_manifest_sha256'])
        npm = report['npm_archive']
        hashed(npm['filename'], npm['sha256'])
        hashed('host-Cargo.lock', report['host_lock_sha256'])
        ipc = json.loads(hashed('ipc.json', report['ipc_sha256']).read_text(encoding='utf-8'))
        if set(ipc) != {'main', 'forbidden'}:
            raise ValueError('missing actual caller windows')
        for window, required in [('main', REQUIRED_MAIN), ('forbidden', REQUIRED_FORBIDDEN)]:
            record = ipc[window]
            if not record['passed'] or record['uncaught'] or not all(case['passed'] for case in record['records']):
                raise ValueError('failed IPC result: ' + name + '/' + window)
            if not required <= {case['name'] for case in record['records']}:
                raise ValueError('skipped IPC cases: ' + name + '/' + window)
        capped = json.loads(hashed('capped/ipc.json', report['capped_ipc_sha256']).read_text(encoding='utf-8'))
        for window, required in [('main', REQUIRED_CAPPED), ('forbidden', REQUIRED_FORBIDDEN)]:
            record = capped[window]
            if not record['passed'] or record['uncaught'] or not all(case['passed'] for case in record['records']) or not required <= {case['name'] for case in record['records']}:
                raise ValueError('missing or failed capped level IPC: ' + name + '/' + window)
        policies = json.loads(hashed('policy-results.json', report['policy_results_sha256']).read_text(encoding='utf-8'))
        if not policies['passed'] or len(policies['records']) != 15 or not all(case['passed'] for case in policies['records']):
            raise ValueError('host policy fixture missing or failed: ' + name)
        faults = json.loads(hashed('fault-results.json', report['fault_results_sha256']).read_text(encoding='utf-8'))
        fixture_path = Path(__file__).resolve().parents[2] / 'bindings/conformance/v1/schema-cases.json'
        if report['conformance_fixture_sha256'] != digest(fixture_path):
            raise ValueError('stale canonical conformance fixtures: ' + name)
        required_fixtures = {'schema-' + case['id'] for case in json.loads(fixture_path.read_text(encoding='utf-8'))}
        if not required_fixtures <= {case['name'] for case in faults['results']}:
            raise ValueError('canonical conformance fixture skipped: ' + name)
        if not faults['passed'] or not faults['results'] or not all(case['passed'] for case in faults['results']):
            raise ValueError('incomplete fault/conformance evidence: ' + name)
        if not all(report['isolation'].get(key) is True for key in ('checkout', 'cargo_cache', 'network')):
            raise ValueError('missing isolation probes: ' + name)
        if not report['jsonl']:
            raise ValueError('missing real JSONL: ' + name)
        for relative, expected in report['jsonl'].items():
            hashed(relative, expected)
        bundle = json.loads((directory / 'bundle-manifest.json').read_text(encoding='utf-8'))
        if bundle['source_commit'] != report['source_commit']:
            raise ValueError('stale source bundle: ' + name)
        for package in bundle['packages']:
            hashed('rust-archives/' + Path(package['archive']).name, package['archive_sha256'])
        native = json.loads((directory / 'native-runtime' / (name.lower() + '.json')).read_text(encoding='utf-8'))
        if native['source_commit'] != report['source_commit'] or native['runtime_source_sha256'] != native_source_digest():
            raise ValueError('stale native runtime evidence: ' + name)
        for profile in ('debug', 'release'):
            cell = native['profiles'][profile]
            if cell['status'] != 'passed' or cell['cases'] != native_cases() or not cell['helper_counts']:
                raise ValueError('missing native lifecycle/resource fixture: ' + name + '/' + profile)
            hashed('native-runtime/' + cell['log'], cell['sha256'])
        found[name] = report
    if set(found) != {'Darwin', 'Linux', 'Windows'}:
        raise ValueError('required platforms missing: ' + ', '.join(sorted({'Darwin', 'Linux', 'Windows'} - found.keys())))
    if len({report['source_commit'] for report in found.values()}) != 1:
        raise ValueError('platforms exercised different source revisions')
    if len({report['npm_archive']['sha256'] for report in found.values()}) != 1:
        raise ValueError('platforms exercised different npm artifacts')
    if len({json.dumps(report['rust_archives'], sort_keys=True) for report in found.values()}) != 1:
        raise ValueError('platforms exercised different Rust artifacts')
    return {'status': 'passed', 'source_commit': next(iter(found.values()))['source_commit'],
            'platforms': {key: {'cases': value['case_count'], 'npm_sha256': value['npm_archive']['sha256'],
                                'rust_archives': value['rust_archives']} for key, value in found.items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--source')
    args = parser.parse_args()
    print(json.dumps(validate(args.directory, args.source), indent=2, sort_keys=True))
    print('TAURI_THREE_PLATFORM_QUALIFICATION_PASSED')


if __name__ == '__main__':
    main()
