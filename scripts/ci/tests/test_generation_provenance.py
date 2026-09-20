"""History prerequisites and anti-tampering at the generation evidence boundary."""
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('binding_artifacts', ROOT / 'scripts/ci/validate_binding_artifacts.py')
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


class GenerationProofTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'checkout'
        subprocess.run(['git', 'clone', '--shared', '--no-checkout', str(ROOT), str(self.root)],
                       check=True, capture_output=True, timeout=30)
        self.evidence_path = self.root / 'bindings/generation-manifest.json'
        self.evidence = json.loads((ROOT / 'bindings/generation-manifest.json').read_text())
        paths = {*self.evidence['inputs'], *self.evidence['outputs'], 'bindings/generation-manifest.json'}
        for path in paths:
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, target)

    def validate(self):
        with patch.object(VALIDATOR, 'ROOT', self.root), contextlib.redirect_stdout(io.StringIO()):
            VALIDATOR.main()

    def test_valid_generation_evidence(self):
        self.validate()

    def test_input_and_matching_hash_cannot_replace_pinned_source(self):
        path = 'crates/sc-observability-dto/Cargo.toml'
        content = (self.root / path).read_bytes() + b'\n[features]\nunauthorized = []\n'
        (self.root / path).write_bytes(content)
        self.evidence['inputs'][path] = hashlib.sha256(content).hexdigest()
        self.evidence_path.write_text(json.dumps(self.evidence))
        with self.assertRaisesRegex(SystemExit, 'generation source/toolchain drift'):
            self.validate()

    def test_output_drift_remains_rejected(self):
        target = self.root / 'bindings/schema/v1.json'
        target.write_bytes(target.read_bytes() + b' ')
        with self.assertRaisesRegex(SystemExit, 'generated artifact hash drift'):
            self.validate()


class CheckoutHistoryTests(unittest.TestCase):
    def test_shallow_history_fails_closed_and_full_history_passes(self):
        with tempfile.TemporaryDirectory() as temp:
            destination = Path(temp) / 'shallow'
            metadata_record = json.loads(
                (ROOT / 'docs/plans/phase-c/manifest-metadata-adaptations.json').read_text()
            )
            historical_commit = subprocess.run(
                ['git', '-C', str(ROOT), 'log', '-1', '--format=%H', '--',
                 'docs/plans/phase-c/manifest-metadata-adaptations.json'],
                check=True, capture_output=True, text=True, timeout=30,
            ).stdout.strip()
            subprocess.run(['git', 'clone', '--depth=1', '--no-local', ROOT.as_uri(), str(destination)],
                           check=True, capture_output=True, timeout=60)
            command = [sys.executable, str(ROOT / 'scripts/ci/_log_release_adaptations.py'),
                       '--destination', str(destination)]
            rejected = subprocess.run(command, capture_output=True, text=True, timeout=60)
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn('rev-parse', rejected.stderr)
            subprocess.run(['git', '-C', str(destination), 'fetch', '--unshallow'],
                           check=True, capture_output=True, timeout=60)
            # The release-preparation version bump intentionally changes the
            # live manifest bytes.  Exercise this historical proof against the
            # immutable post-adaptation snapshot it records, while retaining
            # the full repository history required by the validator.
            subprocess.run(['git', '-C', str(destination), 'checkout', '--detach', historical_commit],
                           check=True, capture_output=True, timeout=60)
            accepted = subprocess.run(command, capture_output=True, text=True, timeout=60)
            self.assertEqual(accepted.returncode, 0, accepted.stderr)
            self.assertIn('historical provenance unchanged', accepted.stdout)


if __name__ == '__main__':
    unittest.main()
