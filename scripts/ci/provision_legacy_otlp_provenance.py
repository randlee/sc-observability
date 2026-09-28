#!/usr/bin/env python3
"""Import checksum-verified historical Git objects required by provenance tests."""
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/fixtures/legacy-otlp-provenance'


def main():
    inventory = json.loads((FIXTURE / 'objects.json').read_text())
    provenance = json.loads(
        (ROOT / 'docs/plans/phase-d/legacy-otlp-provenance.json').read_text()
    )
    if inventory['source_commit'] != provenance['source_commit']:
        raise ValueError('fixture source revision does not match provenance manifest')
    packed = (FIXTURE / 'source.pack').read_bytes()
    if hashlib.sha256(packed).hexdigest() != inventory['pack_sha256']:
        raise ValueError('legacy OTLP provenance pack checksum mismatch')
    # Import data only; do not check out or execute any historical source.
    subprocess.run(['git', 'index-pack', '--stdin'], input=packed, cwd=ROOT, check=True)
    for oid, expected in inventory['objects'].items():
        kind = subprocess.check_output(['git', 'cat-file', '-t', oid], cwd=ROOT, text=True).strip()
        if kind != expected['type']:
            raise ValueError(f'fixture object type mismatch: {oid}')
        if expected['path'] is not None:
            actual = subprocess.check_output(
                ['git', 'rev-parse', f"{inventory['source_commit']}:{expected['path']}"],
                cwd=ROOT, text=True,
            ).strip()
            if actual != oid:
                raise ValueError(f'fixture object path mismatch: {expected["path"]}')
    print('LEGACY_OTLP_PROVENANCE_SOURCE_READY: ' + inventory['source_commit'])


if __name__ == '__main__':
    main()
