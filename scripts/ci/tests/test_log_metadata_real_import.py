"""Opt-in real historical proof: SC_LOG_IMPORT_SOURCE points to accepted BTIT Git history."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts/ci'))
from _log_metadata_adaptations import BRIDGE, CONSUMER, MANIFESTS, RECORD
from _log_staging import PACKAGES
from validate_log_import import IMPORT_CRATE_PREFIXES, validate_import


@unittest.skipUnless(os.environ.get('SC_LOG_IMPORT_SOURCE'), 'requires explicit accepted BTIT source repo')
class RealImportTests(unittest.TestCase):
    def test_historical_import_and_five_negative_mutations(self):
        provenance = json.loads((ROOT / 'docs/plans/phase-b/import-provenance.json').read_text())
        post = json.loads((ROOT / 'docs/plans/phase-b/post-import-adaptations.json').read_text())
        handoff = (ROOT / 'docs/plans/phase-b/handoff-b-p3.md').read_text()
        candidate_version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
        with tempfile.TemporaryDirectory(prefix='phase-c-provenance-proof-') as temp:
            destination = Path(temp) / 'destination'
            subprocess.run(['git', 'clone', '--shared', '--no-checkout', str(ROOT), str(destination)],
                           check=True, capture_output=True, timeout=30)
            for path in IMPORT_CRATE_PREFIXES:
                shutil.copytree(ROOT / path, destination / path)
            paths = [*MANIFESTS, str(RECORD), 'LICENSE', 'docs/plans/phase-b/release-adaptations-b-2.json']
            paths += [f'crates/{name}/LICENSE' for name in PACKAGES]
            for path in paths:
                target = destination / path
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / path, target)

            def check():
                validate_import(provenance, Path(os.environ['SC_LOG_IMPORT_SOURCE']), destination,
                                handoff, doc_repo=ROOT, post_import_adaptations=post,
                                release_adaptations=destination / 'docs/plans/phase-b/release-adaptations-b-2.json')

            check()
            print('PASS actual accepted BTIT -> immutable Phase B -> Phase C metadata chain')
            record = json.loads((destination / RECORD).read_text())
            record['manifests'][BRIDGE]['before_blob'] = '0' * 40
            cases = [
                ('runtime source drift', 'crates/sc-observability-log/src/lib.rs',
                 lambda data: data + b'\npub fn unauthorized_runtime_change() {}\n'),
                ('unrelated manifest feature drift', BRIDGE,
                 lambda data: data + b'\n[features]\nunauthorized = []\n'),
                ('consumer dependency option drift', CONSUMER,
                 lambda data: data.replace(b'sc-observability-log.workspace = true',
                     b'sc-observability-log = { workspace = true, default-features = false }')),
                ('workspace version drift', 'Cargo.toml',
                 lambda data: data.replace(
                     f'version = "{candidate_version}"'.encode(), b'version = "2.0.0"')),
                ('forged before evidence', str(RECORD), lambda _: json.dumps(record).encode()),
            ]
            for label, path, change in cases:
                with self.subTest(label=label):
                    target = destination / path
                    original = target.read_bytes()
                    target.write_bytes(change(original))
                    try:
                        with self.assertRaises((SystemExit, ValueError)) as caught:
                            check()
                        print(f'PASS rejected {label}: {caught.exception}')
                    finally:
                        target.write_bytes(original)
            check()
            print('PASS restored actual import after five negative mutations')


if __name__ == '__main__':
    unittest.main()
