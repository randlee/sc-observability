"""Regression coverage for the retained generated-artifact hash check."""
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
SPEC = importlib.util.spec_from_file_location('binding_artifacts', ROOT / 'scripts/ci/validate_binding_artifacts.py')
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)
PROVENANCE_SPEC = importlib.util.spec_from_file_location(
    'legacy_otlp_provenance', ROOT / 'scripts/ci/provision_legacy_otlp_provenance.py'
)
PROVENANCE = importlib.util.module_from_spec(PROVENANCE_SPEC)
PROVENANCE_SPEC.loader.exec_module(PROVENANCE)


class GenerationProofTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'checkout'
        self.evidence_path = self.root / 'bindings/generation-manifest.json'
        self.evidence = json.loads((ROOT / 'bindings/generation-manifest.json').read_text(encoding='utf-8'))
        paths = {*self.evidence['inputs'], *self.evidence['outputs'], 'bindings/generation-manifest.json'}
        for path in paths:
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, target)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        for path in self.evidence['inputs']:
            subprocess.run(['git', 'hash-object', '-w', path], cwd=self.root,
                           check=True, capture_output=True)

    def validate(self):
        with patch.object(VALIDATOR, 'ROOT', self.root), contextlib.redirect_stdout(io.StringIO()):
            VALIDATOR.main()


    def test_valid_generation_evidence(self):
        self.validate()

    def test_output_drift_remains_rejected(self):
        target = self.root / 'bindings/schema/v1.json'
        target.write_bytes(target.read_bytes() + b' ')
        with self.assertRaisesRegex(SystemExit, 'generated artifact hash drift'):
            self.validate()

    def test_source_blob_inventory_must_cover_every_input(self):
        self.evidence['source_blobs'].pop('bindings/generation-toolchain.toml')
        self.evidence_path.write_text(json.dumps(self.evidence))
        with self.assertRaisesRegex(SystemExit, 'generation source blob inventory drift'):
            self.validate()

    def test_unavailable_source_blob_fails_closed(self):
        self.evidence['source_blobs']['bindings/generation-toolchain.toml'] = '0' * 40
        self.evidence_path.write_text(json.dumps(self.evidence))
        with self.assertRaisesRegex(SystemExit, 'generation source blob unavailable'):
            self.validate()

    def test_fresh_clone_after_rebase_retains_source_evidence(self):
        repository = Path(self.temp.name) / 'rebased'
        repository.mkdir()

        def git(*args):
            return subprocess.check_output(
                ['git', '-c', 'user.name=Provenance Test',
                 '-c', 'user.email=provenance@example.invalid', *args],
                cwd=repository, stderr=subprocess.PIPE, text=True).strip()

        git('init', '-b', 'trunk')
        for path in {*self.evidence['inputs'], *self.evidence['outputs'],
                     'bindings/generation-manifest.json'}:
            target = repository / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(self.root / path, target)
        git('add', '.')
        git('commit', '-m', 'generation inputs and evidence')
        git('checkout', '-b', 'topic')
        (repository / 'topic.txt').write_text('topic\n')
        git('add', '.')
        git('commit', '-m', 'topic change')
        old_head = git('rev-parse', 'HEAD')
        git('checkout', 'trunk')
        (repository / 'base.txt').write_text('base\n')
        git('add', '.')
        git('commit', '-m', 'new base')
        git('checkout', 'topic')
        git('rebase', 'trunk')
        self.assertNotEqual(old_head, git('rev-parse', 'HEAD'))
        clone = Path(self.temp.name) / 'fresh'
        subprocess.run(['git', 'clone', '--depth=1', '--no-local', '--single-branch',
                        '--branch', 'topic', repository.as_uri(), str(clone)],
                       check=True, capture_output=True, timeout=30)
        unavailable = subprocess.run(['git', 'cat-file', '-e', old_head],
                                     cwd=clone, capture_output=True)
        self.assertNotEqual(unavailable.returncode, 0)
        with patch.object(VALIDATOR, 'ROOT', clone), contextlib.redirect_stdout(io.StringIO()):
            VALIDATOR.main()


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
