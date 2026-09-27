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
        self.evidence = json.loads((ROOT / 'bindings/generation-manifest.json').read_text(encoding='utf-8'))
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



if __name__ == '__main__':
    unittest.main()
