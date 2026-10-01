"""Regression coverage for immutable legacy OTLP source provenance."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
PROVENANCE_SPEC = importlib.util.spec_from_file_location(
    'legacy_otlp_provenance', ROOT / 'scripts/ci/provision_legacy_otlp_provenance.py'
)
PROVENANCE = importlib.util.module_from_spec(PROVENANCE_SPEC)
PROVENANCE_SPEC.loader.exec_module(PROVENANCE)


class LegacyOtlpProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'checkout'
        self.root.mkdir()
        for path in [
            'docs/plans/phase-d/legacy-otlp-provenance.json',
            'docs/observability/otlp/smoke-test.md',
            'docs/observability/otlp/legacy-dispositions.md',
            'scripts/ci/otlp_dev_install_smoke.py',
        ]:
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, target)
        shutil.copytree(
            ROOT / 'tests/fixtures/legacy-otlp-provenance',
            self.root / 'tests/fixtures/legacy-otlp-provenance',
        )
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)

    def validate(self):
        with contextlib.redirect_stdout(io.StringIO()):
            PROVENANCE.validate_non_grafana(self.root)

    def test_real_pinned_source_and_non_grafana_destinations_validate(self):
        self.validate()

    def test_rejects_a_pinned_source_hash_mutation(self):
        manifest = self.root / 'docs/plans/phase-d/legacy-otlp-provenance.json'
        value = json.loads(manifest.read_text())
        entry = next(item for item in value['entries'] if item.get('validation_scope') == 'd9-non-grafana')
        entry['sha256'] = '0' * 64
        manifest.write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, 'pinned source hash mismatch'):
            self.validate()

    def test_rejects_a_pinned_source_blob_mutation(self):
        manifest = self.root / 'docs/plans/phase-d/legacy-otlp-provenance.json'
        value = json.loads(manifest.read_text())
        entry = next(item for item in value['entries'] if item.get('validation_scope') == 'd9-non-grafana')
        entry['git_blob'] = '6524662aa9d37f21451759accef52c2cfea51c78'
        manifest.write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, 'pinned source blob mismatch'):
            self.validate()

    def test_rejects_an_unauthorized_destination_delta(self):
        destination = self.root / 'scripts/ci/otlp_dev_install_smoke.py'
        destination.write_text(destination.read_text() + '\n# ATM_DAEMON_BIN\n')
        with self.assertRaisesRegex(ValueError, 'transformed destination mismatch'):
            self.validate()

    def test_rejects_a_smoke_that_silently_accepts_cargo_failure(self):
        destination = self.root / 'scripts/ci/otlp_dev_install_smoke.py'
        destination.write_text(destination.read_text().replace('check=True', 'check=False'))
        with self.assertRaisesRegex(ValueError, 'transformed destination mismatch'):
            self.validate()

    def test_rejects_a_smoke_with_an_early_success_return(self):
        destination = self.root / 'scripts/ci/otlp_dev_install_smoke.py'
        destination.write_text(destination.read_text().replace(
            'def main() -> None:\n',
            'def main() -> None:\n    return\n',
        ))
        with self.assertRaisesRegex(ValueError, 'transformed destination mismatch'):
            self.validate()

    def test_rejects_an_unmapped_historical_responsibility(self):
        manifest = self.root / 'docs/plans/phase-d/legacy-otlp-provenance.json'
        value = json.loads(manifest.read_text())
        entry = next(item for item in value['entries'] if item.get('validation_scope') == 'd9-non-grafana'
                     and item['source'] == 'scripts/otel-dev-install-smoke.py')
        entry['named_transformation']['responsibility_mapping'].pop()
        manifest.write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, 'responsibility mapping mismatch'):
            self.validate()



if __name__ == '__main__':
    unittest.main()
