"""Regression coverage for the retained generated-artifact hash check."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
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

    def test_output_drift_remains_rejected(self):
        target = self.root / 'bindings/schema/v1.json'
        target.write_bytes(target.read_bytes() + b' ')
        with self.assertRaisesRegex(SystemExit, 'generated artifact hash drift'):
            self.validate()



if __name__ == '__main__':
    unittest.main()
