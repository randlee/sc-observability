"""Narrow B.2 release-only proof composed after historical import validation."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

from _log_staging import PACKAGES

PUBLIC_IMPORTS = ('sc-observability-log', 'sc-observability-log-macros')


def blob(content: bytes) -> str:
    return hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest()


def apply_release_adaptations(expected: dict[str, str], destination: Path, record_path: Path) -> dict[str, str]:
    """Prove only root-license additions and false-to-true public package flags."""
    record = json.loads(record_path.read_text())
    if record.get('schema_version') != 1 or record.get('candidate_version') != '1.4.0':
        raise ValueError('unsupported B.2 release adaptation record')
    root_license = destination / 'LICENSE'
    if root_license.is_symlink() or hashlib.sha256(root_license.read_bytes()).hexdigest() != record['root_license_sha256']:
        raise ValueError('root LICENSE differs from the recorded release source')
    expected_licenses = {f'crates/{name}/LICENSE' for name in PACKAGES}
    if set(record['license_copies']) != expected_licenses:
        raise ValueError('release license adaptations must name exactly six public crates')
    result = dict(expected)
    for relative, declared in record['license_copies'].items():
        path = destination / relative
        if path.is_symlink() or path.read_bytes() != root_license.read_bytes() or declared != blob(root_license.read_bytes()):
            raise ValueError(f'release license copy mismatch: {relative}')
        if any(relative.startswith(f'crates/{name}/') for name in PUBLIC_IMPORTS):
            if relative in expected:
                raise ValueError('license addition unexpectedly replaces an imported file')
            result[relative] = declared
    expected_manifests = {f'crates/{name}/Cargo.toml' for name in PUBLIC_IMPORTS}
    if set(record['publish_flags']) != expected_manifests:
        raise ValueError('release flags may affect only log and macros packages')
    for relative, item in record['publish_flags'].items():
        path = destination / relative
        if path.is_symlink():
            raise ValueError('release manifest may not be a symlink')
        after = path.read_bytes()
        if after.count(b'publish = true\n') != 1:
            raise ValueError(f'missing singular public release flag: {relative}')
        before = after.replace(b'publish = true\n', b'publish = false\n', 1)
        if (blob(before) != expected.get(relative) or blob(before) != item['before_blob']
                or blob(after) != item['after_blob']):
            raise ValueError(f'change exceeds approved publish-only adaptation: {relative}')
        result[relative] = item['after_blob']
    return result


def main() -> None:
    import argparse
    from validate_log_import import validate_adaptations
    parser = argparse.ArgumentParser()
    parser.add_argument('--destination', type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    provenance = json.loads((args.destination / 'docs/plans/phase-b/import-provenance.json').read_text())
    expected = validate_adaptations(provenance.get('adaptations', []), provenance['file_inventory'], args.destination)
    apply_release_adaptations(expected, args.destination, args.destination / 'docs/plans/phase-b/release-adaptations-b-2.json')
    print('B.2 release-only LICENSE and publish-flag adaptations verified; historical provenance unchanged')


if __name__ == '__main__':
    main()
