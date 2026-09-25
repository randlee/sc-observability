"""Reject drift even when an adaptation record has internally consistent hashes."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from _log_metadata_adaptations import (
    BRIDGE, CONSUMER, MANIFESTS, RECORD, MetadataAdaptations, blob, validate_delta,
)


def snapshots():
    root = '''[workspace.package]
version = "1.4.0"
homepage = "https://example.test"
repository = "https://example.test/source"
[workspace.dependencies]
sc-observability-log = { path = "crates/sc-observability-log", version = "1.4.0" }
'''
    before = {"Cargo.toml": root.encode()}
    after = {"Cargo.toml": root.replace('[workspace.package]\n',
                                          '[workspace.package]\nauthors = ["Rand Lee"]\n').encode()}
    for path in MANIFESTS - {"Cargo.toml"}:
        body = f'[package]\nname = "{Path(path).parent.name}"\nversion.workspace = true\n'
        body += 'publish = false\n'
        if path == CONSUMER:
            body += '[dependencies]\nsc-observability-log = { path = "../sc-observability-log" }\n'
        before[path] = body.encode()
        changed = body.replace('[package]\n', '[package]\nauthors.workspace = true\n')
        if path == CONSUMER:
            changed = changed.replace('sc-observability-log = { path = "../sc-observability-log" }',
                                      'sc-observability-log.workspace = true')
        after[path] = changed.encode()
    return before, after


class DeltaTests(unittest.TestCase):
    def test_accepts_metadata_and_equivalent_inherited_dependency(self):
        validate_delta(*snapshots())

    def test_rejects_unrelated_manifest_changes_with_consistent_snapshots(self):
        for addition in ('[features]\nextra = []\n', '[lib]\npath = "evil.rs"\n',
                         '[build-dependencies]\nnew = "1"\n'):
            with self.subTest(addition=addition):
                before, after = snapshots()
                after[BRIDGE] += addition.encode()
                with self.assertRaisesRegex(ValueError, 'exceeds Phase C'):
                    validate_delta(before, after)

    def test_rejects_publish_or_package_version_change(self):
        for old, new in ((b'publish = false', b'publish = true'),
                         (b'version.workspace = true', b'version = "2.0.0"')):
            with self.subTest(new=new):
                before, after = snapshots()
                after[BRIDGE] = after[BRIDGE].replace(old, new)
                with self.assertRaises(ValueError):
                    validate_delta(before, after)

    def test_rejects_undeclared_comments_and_non_boolean_inheritance(self):
        for old, new in ((b'[package]', b'# unrelated edit\n[package]'),
                         (b'authors.workspace = true', b'authors.workspace = 1')):
            with self.subTest(new=new):
                before, after = snapshots()
                after[BRIDGE] = after[BRIDGE].replace(old, new)
                with self.assertRaisesRegex(ValueError, 'exact Phase C'):
                    validate_delta(before, after)

    def test_rejects_literal_metadata_and_overwriting_existing_metadata(self):
        before, after = snapshots()
        after[BRIDGE] = after[BRIDGE].replace(b'authors.workspace = true', b'authors = ["Someone"]')
        with self.assertRaisesRegex(ValueError, 'must inherit'):
            validate_delta(before, after)
        before, after = snapshots()
        before[BRIDGE] += b'homepage = "https://old.test"\n'
        after[BRIDGE] += b'homepage.workspace = true\n'
        with self.assertRaisesRegex(ValueError, 'exceeds Phase C'):
            validate_delta(before, after)

    def test_rejects_dependency_options_features_path_and_version_drift(self):
        for old, new in (
            (b'version = "1.4.0" }', b'version = "2.0.0" }'),
            (b'path = "crates/sc-observability-log"', b'path = "crates/other"'),
            (b'version = "1.4.0" }', b'version = "1.4.0", features = ["extra"] }'),
            (b'version = "1.4.0" }', b'version = "1.4.0", default-features = false }'),
            (b'version = "1.4.0" }', b'version = "1.4.0", optional = true }'),
        ):
            with self.subTest(new=new):
                before, after = snapshots()
                after['Cargo.toml'] = after['Cargo.toml'].replace(old, new)
                with self.assertRaises(ValueError):
                    validate_delta(before, after)
        before, after = snapshots()
        before[CONSUMER] = before[CONSUMER].replace(b'path = "../sc-observability-log"',
                                                 b'path = "../sc-observability-log", features = ["lost"]')
        with self.assertRaisesRegex(ValueError, 'options/features'):
            validate_delta(before, after)


class ImmutableEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.test')
        self.before, self.after = snapshots()
        self.write(self.before)
        self.git('add', '.')
        self.git('commit', '-qm', 'before')
        before_commit = self.git('rev-parse', 'HEAD')
        self.write(self.after)
        self.git('add', '.')
        self.git('commit', '-qm', 'metadata')
        self.record = {
            'schema_version': 1, 'kind': 'phase_c_workspace_metadata',
            'historical_provenance': 'docs/plans/phase-b/import-provenance.json',
            'release_adaptations': 'docs/plans/phase-b/release-adaptations-b-2.json',
            'reason': 'Synthetic metadata evidence', 'before_commit': before_commit,
            'after_commit': self.git('rev-parse', 'HEAD'),
            'manifests': {p: {'before_blob': blob(self.before[p]), 'after_blob': blob(self.after[p])}
                          for p in MANIFESTS},
        }
        self.record_path = self.root / RECORD
        self.record_path.parent.mkdir(parents=True)
        self.save()

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.root), *args], check=True,
                              capture_output=True, text=True, timeout=30).stdout.strip()

    def write(self, inventory):
        for path, content in inventory.items():
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)

    def save(self):
        self.record_path.write_text(json.dumps(self.record))

    def validate(self):
        return MetadataAdaptations(self.root, self.record_path)

    def test_exact_history_and_inventory_chain(self):
        stage = self.validate()
        expected = {p: blob(self.before[p]) for p in (CONSUMER, BRIDGE,
                                      'crates/sc-observability-log-macros/Cargo.toml')}
        expected['runtime.rs'] = 'unchanged'
        actual = stage.apply(expected)
        self.assertEqual(actual['runtime.rs'], 'unchanged')
        self.assertEqual(actual[CONSUMER], blob(self.after[CONSUMER]))
        expected[CONSUMER] = 'f' * 40
        with self.assertRaisesRegex(ValueError, 'historical/release proof'):
            stage.apply(expected)

    def test_rejects_forged_before_and_after_blobs(self):
        original = copy.deepcopy(self.record)
        for side in ('before_blob', 'after_blob'):
            with self.subTest(side=side):
                self.record = copy.deepcopy(original)
                self.record['manifests'][BRIDGE][side] = '0' * 40
                self.save()
                with self.assertRaisesRegex(ValueError, 'immutable snapshot'):
                    self.validate()

    def test_rejects_live_drift_even_with_matching_recorded_hash(self):
        data = self.after[BRIDGE] + b'[features]\nmalicious = []\n'
        (self.root / BRIDGE).write_bytes(data)
        with self.assertRaisesRegex(ValueError, 'exact after snapshot'):
            self.validate()
        self.record['manifests'][BRIDGE]['after_blob'] = blob(data)
        self.save()
        with self.assertRaisesRegex(ValueError, 'immutable snapshot'):
            self.validate()

    def test_rejects_runtime_path_in_record(self):
        self.record['manifests']['crates/sc-observability-log/src/lib.rs'] = {'before_blob': '0' * 40}
        self.save()
        with self.assertRaisesRegex(ValueError, 'evidence scope'):
            self.validate()

    def test_rejects_runtime_change_in_evidence_commit(self):
        path = self.root / 'crates/sc-observability-log/src/lib.rs'
        path.parent.mkdir()
        path.write_text('fn changed_runtime() {}')
        self.git('add', str(path))
        self.git('commit', '--amend', '--no-edit', '-q')
        self.record['after_commit'] = self.git('rev-parse', 'HEAD')
        self.save()
        with self.assertRaisesRegex(ValueError, 'outside declared manifests'):
            self.validate()

    def test_rejects_floating_commit_and_symlinked_manifest(self):
        self.record['after_commit'] = 'HEAD'
        self.save()
        with self.assertRaisesRegex(ValueError, 'immutable commits'):
            self.validate()
        self.record['after_commit'] = self.git('rev-parse', 'HEAD')
        self.save()
        target = self.root / BRIDGE
        target.unlink()
        target.symlink_to(self.root / CONSUMER)
        with self.assertRaisesRegex(ValueError, 'regular confined'):
            self.validate()


if __name__ == '__main__':
    unittest.main()
