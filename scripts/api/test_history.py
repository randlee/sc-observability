"""Focused comparator tests; the normal Cargo fixture proves real metadata changes."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from scripts.api import history


class HistoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git('init', '-q')
        self.git('config', 'user.name', 'API test')
        self.git('config', 'user.email', 'api-test@example.invalid')
        self.entry = {'package': 'example', 'version': '1.5.0-rc.1', 'features': [],
                      'artifact': str(self.root / 'built.rlib')}
        (self.root / 'source.rs').write_text('pub fn method() {}\n')
        (self.root / 'built.rlib').write_bytes(b'current compiled metadata fixture')
        self.path = history.snapshot_path(self.entry, self.root)
        self.path.parent.mkdir(parents=True)
        self.snapshot = {'schema': 1, 'package': 'example', 'version': '1.5.0-rc.1',
                         'format': 'rustc-1.94.1-metadata/v1', **history.encode_families({'none': ['method()']})}
        self.path.write_text(json.dumps(self.snapshot))
        self.git('add', '.')
        self.git('commit', '-qm', 'accepted prerelease')
        self.base = self.git('rev-parse', 'HEAD').strip()
        # Checks must compare the candidate against a preceding accepted
        # revision, never the candidate's own tree.
        (self.root / 'source.rs').write_text('pub fn method() {} // candidate\n')
        self.git('add', 'source.rs')
        self.git('commit', '-qm', 'candidate source')
        self.record = {'entries': [self.entry], 'fixture': None,
                       'source_sha256': history.source_fingerprint(self.root)}
        self.rows = {self.entry['artifact']: ['method()']}
        self.read = patch.object(history, 'read_surfaces', side_effect=lambda _: self.rows)
        self.read.start()
        self.addCleanup(self.read.stop)

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, text=True)

    def check(self, **kwargs):
        return history.check_current(self.record, self.base, self.root, **kwargs)

    def test_current_api_is_compared_not_snapshot_to_itself(self):
        self.rows[self.entry['artifact']] = ['method(u64)']
        with self.assertRaisesRegex(history.ApiError, r'example 1.5.0-rc.1.*API differs'):
            self.check()

    def test_candidate_commit_cannot_be_its_own_accepted_base(self):
        with self.assertRaisesRegex(history.ApiError, 'resolves to the candidate'):
            history.accepted_history('HEAD', self.root)

    def test_missing_accepted_base_fails_closed(self):
        with patch.dict(os.environ, {'SC_API_ACCEPTED_BASE': ''}):
            with self.assertRaisesRegex(history.ApiError, 'requires a PR base or prior revision'):
                history.accepted_base(root=self.root)

    def test_missing_trusted_baseline_fails_closed_with_the_history_diagnostic(self):
        with self.assertRaisesRegex(history.ApiError, 'requires a PR base or prior revision'):
            history.accepted_base('origin/develop', self.root)

    def test_explicit_prior_revision_is_a_valid_accepted_base(self):
        self.assertEqual(history.accepted_base('HEAD^', self.root), self.base)

    def test_code_and_accepted_snapshot_change_without_version_increment_fails(self):
        (self.root / 'source.rs').write_text('pub fn method(_: u64) {}\n')
        self.rows[self.entry['artifact']] = ['method(u64)']
        self.snapshot.update(history.encode_families({'none': ['method(u64)']}))
        self.path.write_text(json.dumps(self.snapshot))
        self.record['source_sha256'] = history.source_fingerprint(self.root)
        with self.assertRaisesRegex(history.ApiError, 'immutable accepted API history changed'):
            self.check()

    def test_trusted_baseline_rejects_snapshot_tampering_across_multiple_candidate_commits(self):
        self.snapshot.update(history.encode_families({'none': ['method(u64)']}))
        self.path.write_text(json.dumps(self.snapshot))
        self.git('add', str(self.path.relative_to(self.root)))
        self.git('commit', '-qm', 'tamper accepted snapshot')
        (self.root / 'source.rs').write_text('pub fn method(_: u64) {}\n')
        self.rows[self.entry['artifact']] = ['method(u64)']
        self.git('add', 'source.rs')
        self.git('commit', '-qm', 'candidate API change')
        self.record['source_sha256'] = history.source_fingerprint(self.root)
        with self.assertRaisesRegex(history.ApiError, 'immutable accepted API history changed'):
            self.check()

    def test_accepted_deletion_is_rejected(self):
        self.path.unlink()
        with self.assertRaisesRegex(history.ApiError, 'immutable accepted API history changed'):
            self.check()

    def test_new_version_keeps_accepted_prerelease(self):
        previous = self.path.read_bytes()
        self.entry['version'] = '1.5.0-rc.2'
        self.rows[self.entry['artifact']] = ['method(u64)']
        self.check(capture=True)
        self.assertEqual(self.path.read_bytes(), previous)
        self.assertTrue(history.snapshot_path(self.entry, self.root).is_file())
        self.check()

    def test_source_mutation_rejects_old_artifacts_before_reader(self):
        (self.root / 'source.rs').write_text('pub fn different() {}\n')
        with self.assertRaisesRegex(history.ApiError, 'source changed'):
            self.check()

    def test_untracked_source_also_invalidates(self):
        (self.root / 'new.rs').write_text('pub struct Added;')
        with self.assertRaisesRegex(history.ApiError, 'source changed'):
            self.check()

    def test_unchanged_capture_is_deterministic(self):
        previous = self.path.read_bytes()
        self.check()
        self.check(capture=True)
        self.assertEqual(self.path.read_bytes(), previous)

    def test_missing_configuration_does_not_claim_coverage(self):
        self.entry['features'] = ['another']
        with self.assertRaisesRegex(history.ApiError, 'another'):
            self.check()

    def test_failed_metadata_reader_is_not_a_skip(self):
        with patch.object(history, 'read_surfaces', side_effect=history.ApiError('incomplete metadata')):
            with self.assertRaisesRegex(history.ApiError, 'incomplete metadata'):
                self.check()

    def test_inspection_path_has_no_build_subprocess(self):
        original = subprocess.run
        commands = []
        def checked(command, **kwargs):
            commands.append(command)
            self.assertEqual(command[0], 'git')
            return original(command, **kwargs)
        with patch.object(subprocess, 'run', side_effect=checked):
            self.check()
        self.assertTrue(commands)


class ArtifactTests(unittest.TestCase):
    def test_artifact_digest_tampering_is_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / 'built.rlib'
            artifact.write_bytes(b'old')
            checksum = history.digest_file(artifact)
            artifact.write_bytes(b'new')
            with self.assertRaisesRegex(history.ApiError, 'artifact changed'):
                history.read_surfaces({'entries': [{'artifact': str(artifact), 'artifact_sha256': checksum}], 'fixture': None})

    def test_manifest_version_is_selected_without_cargo(self):
        entries = history.catalog()
        self.assertEqual(len(entries), 10)
        workspace = tomllib.loads((history.ROOT / 'Cargo.toml').read_text())['workspace']['package']
        for path, item in entries.items():
            declared = tomllib.loads(Path(path).read_text())['package']['version']
            self.assertEqual(item['version'], workspace['version'] if isinstance(declared, dict) else declared)


if __name__ == '__main__':
    unittest.main()
