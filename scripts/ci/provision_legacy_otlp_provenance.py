#!/usr/bin/env python3
"""Validate the checked-in legacy OTLP provenance source and D9 dispositions."""
import base64
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load_source(root=ROOT):
    fixture = root / 'tests/fixtures/legacy-otlp-provenance'
    inventory = json.loads((fixture / 'objects.json').read_text())
    provenance = json.loads((root / 'docs/plans/phase-d/legacy-otlp-provenance.json').read_text())
    if inventory['source_commit'] != provenance['source_commit']:
        raise ValueError('fixture source revision does not match provenance manifest')
    packed = (fixture / 'source.pack').read_bytes()
    if hashlib.sha256(packed).hexdigest() != inventory['pack_sha256']:
        raise ValueError('legacy OTLP provenance pack checksum mismatch')
    # Import data only; do not check out or execute any historical source.
    subprocess.run(['git', 'index-pack', '--stdin'], input=packed, cwd=root, check=True)
    for oid, expected in inventory['objects'].items():
        kind = subprocess.check_output(['git', 'cat-file', '-t', oid], cwd=root, text=True).strip()
        if kind != expected['type']:
            raise ValueError(f'fixture object type mismatch: {oid}')
        if expected['path'] is not None:
            actual = subprocess.check_output(
                ['git', 'rev-parse', f"{inventory['source_commit']}:{expected['path']}"],
                cwd=root, text=True,
            ).strip()
            if actual != oid:
                raise ValueError(f'fixture object path mismatch: {expected["path"]}')
    return inventory, provenance


def validate_entry(entry, inventory, provenance, root):
    source = entry['source']
    oid = entry['git_blob']
    if oid not in inventory['objects']:
        raise ValueError(f'provenance source is absent from fixture inventory: {source}')
    actual = subprocess.check_output(
        ['git', 'rev-parse', f"{provenance['source_commit']}:{source}"], cwd=root, text=True,
    ).strip()
    if actual != oid:
        raise ValueError(f'pinned source blob mismatch: {source}')
    content = subprocess.check_output(['git', 'cat-file', 'blob', oid], cwd=root)
    if hashlib.sha256(content).hexdigest() != entry['sha256']:
        raise ValueError(f'pinned source hash mismatch: {source}')

    destination = root / entry['destination']
    if not destination.is_file():
        raise ValueError(f'provenance destination missing: {entry["destination"]}')
    if entry['disposition'] == 'transplant-and-adapt':
        validate_named_transformation(entry, content, destination)
        return
    rendered = destination.read_text(encoding='utf-8')
    if content.decode('utf-8') == rendered:
        raise ValueError(f'disposition was not applied: {source}')
    marker = f"source: {source}"
    if marker not in rendered or f"git_blob: {oid}" not in rendered:
        raise ValueError(f'provenance marker missing: {entry["destination"]}')
    if f"sha256: {entry['sha256']}" not in rendered:
        raise ValueError(f'provenance hash marker missing: {entry["destination"]}')
    if f"disposition: {entry['disposition']}" not in rendered:
        raise ValueError(f'disposition marker missing: {entry["destination"]}')

    deltas = entry.get('allowed_deltas')
    if not isinstance(deltas, list) or not deltas:
        raise ValueError(f'allowed delta contract missing: {source}')
    for delta in deltas:
        name = delta.get('name')
        required = delta.get('required_destination_fragments')
        forbidden = delta.get('forbidden_destination_fragments')
        if not isinstance(name, str) or not name or not isinstance(required, list) or not isinstance(forbidden, list):
            raise ValueError(f'malformed allowed delta contract: {source}')
        if any(not isinstance(fragment, str) or not fragment for fragment in [*required, *forbidden]):
            raise ValueError(f'malformed allowed delta fragment: {source}')
        if f'allowed_delta: {name}' not in rendered:
            raise ValueError(f'named allowed delta missing from destination: {name}')
        missing = [fragment for fragment in required if fragment not in rendered]
        if missing:
            raise ValueError(f'allowed delta obligation missing ({name}): {missing[0]}')
        present = [fragment for fragment in forbidden if fragment in rendered]
        if present:
            raise ValueError(f'unauthorized delta ({name}): {present[0]}')
def validate_named_transformation(entry, source, destination):
    contract = entry.get('named_transformation')
    if not isinstance(contract, dict):
        raise ValueError(f'named transformation missing: {entry["source"]}')
    names = [delta.get('name') for delta in entry.get('allowed_deltas', [])]
    if contract.get('name') != 'replace-dev-install-with-hermetic-worktree-smoke':
        raise ValueError(f'unknown named transformation: {entry["source"]}')
    if contract.get('named_deltas') != names:
        raise ValueError(f'named transformation delta mismatch: {entry["source"]}')
    if contract.get('base_sha256') != hashlib.sha256(source).hexdigest():
        raise ValueError(f'named transformation base mismatch: {entry["source"]}')
    if contract.get('operation') != 'whole-file-replacement':
        raise ValueError(f'unsupported named transformation: {entry["source"]}')
    transformed = apply_named_transformation(entry['source'], source, contract)
    if destination.read_bytes() != transformed:
        raise ValueError(f'transformed destination mismatch: {entry["destination"]}')
    validate_responsibility_mapping(entry['source'], contract)


def apply_named_transformation(source_path, source, contract):
    """Apply the one approved, source-hash-bound D9 replacement contract."""
    if contract.get('base_sha256') != hashlib.sha256(source).hexdigest():
        raise ValueError(f'named transformation base mismatch: {source_path}')
    try:
        transformed = base64.b64decode(contract['replacement_base64'], validate=True)
    except (KeyError, TypeError, ValueError) as error:
        raise ValueError(f'malformed named transformation: {source_path}') from error
    if contract.get('result_sha256') != hashlib.sha256(transformed).hexdigest():
        raise ValueError(f'named transformation result hash mismatch: {source_path}')
    return transformed


def validate_responsibility_mapping(source_path, contract):
    """Keep every historical operational responsibility explicit and honest."""
    expected = {
        'live-collector-delivery': 'covered-by-current-public-factory-test',
        'local-log-and-otel-mirror-retention': 'application-owned-outside-d9-no-atm-core',
        'collector-outage-command-success-and-local-logging': 'application-owned-outside-d9-no-atm-core',
    }
    mapping = contract.get('responsibility_mapping')
    if not isinstance(mapping, list):
        raise ValueError(f'named transformation responsibility mapping missing: {source_path}')
    actual = {row.get('responsibility'): row for row in mapping if isinstance(row, dict)}
    if set(actual) != set(expected) or len(actual) != len(mapping):
        raise ValueError(f'named transformation responsibility mapping mismatch: {source_path}')
    for responsibility, status in expected.items():
        row = actual[responsibility]
        if row.get('status') != status:
            raise ValueError(f'named transformation responsibility status mismatch: {responsibility}')
        if not isinstance(row.get('historical_evidence'), str) or not row['historical_evidence']:
            raise ValueError(f'named transformation historical responsibility missing: {responsibility}')
        if status == 'covered-by-current-public-factory-test':
            if row.get('current_evidence') != 'public_legacy_http_json_factory_exports_decoded_trace_counter_and_gauge':
                raise ValueError(f'named transformation current evidence mismatch: {responsibility}')
        elif (
            row.get('authorized_disposition')
            != 'D8 source matrix: application-owned-dev-install-environment'
            or not isinstance(row.get('reason'), str)
            or not row['reason']
        ):
            raise ValueError(f'named transformation application-owned responsibility mismatch: {responsibility}')


def validate_non_grafana(root=ROOT):
    inventory, provenance = load_source(root)
    for entry in provenance['entries']:
        is_grafana = 'grafana' in entry['destination'].lower()
        if is_grafana:
            if entry.get('deferred') is not True or not entry.get('deferred_reason'):
                raise ValueError(f'Grafana provenance must remain explicitly deferred: {entry["source"]}')
        elif entry.get('validation_scope') == 'd9-non-grafana':
            validate_entry(entry, inventory, provenance, root)
    print('LEGACY_OTLP_NON_GRAFANA_PROVENANCE_VALID: pinned source and named deltas')


def main():
    validate_non_grafana()


if __name__ == '__main__':
    main()
