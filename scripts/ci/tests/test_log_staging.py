"""Negative cases at the immutable archive, resolution and approval boundaries."""
import io
import json
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from _log_staging import PACKAGES, inspect_archive, sha256, verify_stage
from validate_log_staged_consumer import validate_resolution
from validate_public_api import approval_for
from prepare_runtime_level_staged_packages import candidate_workspace_manifest, normalized_lock

VERSION = '1.4.0'
SOURCE = 'a' * 40


def archive(path, name, *, dependency='', source=SOURCE, extra=None):
    body = f'[package]\nname = "{name}"\nversion = "{VERSION}"\nlicense = "MIT"\n'
    if name == 'sc-observability-log':
        body += f'\n[dependencies.sc-observability-log-macros]\nversion = "={VERSION}"\n'
    body += dependency
    files = {'Cargo.toml': body.encode(), 'LICENSE': b'MIT', '.cargo_vcs_info.json': json.dumps({'git': {'sha1': source, 'dirty': False}}).encode()}
    if extra:
        files.update(extra)
    with tarfile.open(path, 'w:gz') as output:
        for relative, content in files.items():
            member = tarfile.TarInfo(f'{name}-{VERSION}/{relative}')
            member.size = len(content)
            output.addfile(member, io.BytesIO(content))


class StageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        items = []
        for name in PACKAGES:
            path = self.root / f'{name}.crate'
            archive(path, name)
            items.append({'name': name, 'version': VERSION, 'archive': path.name, 'archive_sha256': sha256(path), **inspect_archive(path, name, VERSION, SOURCE)})
        self.manifest = {'schema_version': 1, 'candidate_version': VERSION, 'source_commit': SOURCE, 'publication': 'pending_B.7', 'packages': items}
        self.save()

    def save(self):
        (self.root / 'stage-manifest.json').write_text(json.dumps(self.manifest))

    def test_accepts_exact_six_package_stage(self):
        verify_stage(self.root, VERSION, SOURCE)

    def test_rejects_archive_tampering(self):
        path = self.root / self.manifest['packages'][0]['archive']
        path.write_bytes(path.read_bytes() + b'tampered')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            verify_stage(self.root, VERSION)

    def test_rejects_wrong_candidate_version(self):
        with self.assertRaisesRegex(ValueError, 'version'):
            verify_stage(self.root, '1.3.0')

    def test_rejects_changed_source_even_with_updated_manifest(self):
        self.manifest['source_commit'] = 'b' * 40
        self.save()
        with self.assertRaisesRegex(ValueError, 'source commit'):
            verify_stage(self.root, VERSION)

    def test_rejects_wrong_expected_source(self):
        with self.assertRaisesRegex(ValueError, 'source commit'):
            verify_stage(self.root, VERSION, 'b' * 40)

    def test_rejects_private_package_leak(self):
        self.manifest['packages'].append({'name': 'sc-observability-log-consumer-check'})
        self.save()
        with self.assertRaisesRegex(ValueError, 'six public'):
            verify_stage(self.root, VERSION)

    def test_rejects_path_escape(self):
        self.manifest['packages'][0]['archive'] = '../ambient.crate'
        self.save()
        with self.assertRaisesRegex(ValueError, 'unsafe'):
            verify_stage(self.root, VERSION)

    def test_rejects_ambient_manifest_dependency(self):
        path = self.root / 'ambient.crate'
        archive(path, PACKAGES[0], dependency='\n[dependencies.ambient]\npath = "/checkout"\n')
        with self.assertRaisesRegex(ValueError, 'ambient dependency'):
            inspect_archive(path, PACKAGES[0], VERSION, SOURCE)

    def test_rejects_archive_traversal(self):
        path = self.root / 'unsafe.crate'
        archive(path, PACKAGES[0], extra={'../../outside': b'bad'})
        with self.assertRaisesRegex(ValueError, 'unsafe'):
            inspect_archive(path, PACKAGES[0], VERSION, SOURCE)

    def test_rejects_ambient_checkout_resolution(self):
        paths = {name: self.root / name for name in PACKAGES}
        packages = [{'name': name, 'version': VERSION, 'source': None, 'manifest_path': str(paths[name] / 'Cargo.toml')} for name in PACKAGES]
        validate_resolution({'packages': packages}, paths, VERSION)
        packages[0]['manifest_path'] = '/checkout/Cargo.toml'
        with self.assertRaisesRegex(ValueError, 'ambient'):
            validate_resolution({'packages': packages}, paths, VERSION)

    def test_historical_staging_derives_current_workspace_version(self):
        import tomllib
        for baseline in ('1.2.0', '1.4.0', '2.7.9'):
            text = f'[workspace.package]\nversion = "{baseline}"\n[workspace.dependencies]\nsc-observability = {{ version = "{baseline}", path = "crates/sc-observability" }}\nsc-observability-log-macros = {{ version = "={baseline}", path = "crates/sc-observability-log-macros" }}\n'
            candidate = tomllib.loads(candidate_workspace_manifest(text, '1.3.0'))['workspace']
            self.assertEqual(candidate['package']['version'], '1.3.0')
            self.assertEqual(candidate['dependencies']['sc-observability']['version'], '1.3.0')
            self.assertEqual(candidate['dependencies']['sc-observability-log-macros']['version'], '=1.3.0')
            lock = self.root / 'Cargo.lock'
            lock.write_text(f'[[package]]\nname = "sc-observability"\nversion = "{baseline}"\n\n[[package]]\nname = "third-party"\nversion = "{baseline}"\n')
            normalized = tomllib.loads(normalized_lock(lock, '1.3.0').decode())['package']
            self.assertEqual(normalized[0]['version'], '1.3.0')
            self.assertEqual(normalized[1]['version'], baseline)

    def test_unrelated_or_pending_approval_is_not_a_waiver(self):
        directory = self.root / 'approvals'
        directory.mkdir()
        record = {'schema_version': 1, 'candidate_version': VERSION, 'crates': {PACKAGES[0]: {'status': 'approved', 'reviewer': 'reviewer', 'evidence': 'review-report', 'scope': ['public-api']}}}
        (directory / 'scoped.json').write_text(json.dumps(record))
        self.assertTrue(approval_for(PACKAGES[0], VERSION, directory))
        self.assertFalse(approval_for(PACKAGES[1], VERSION, directory))
        self.assertFalse(approval_for(PACKAGES[0], '1.3.0', directory))
        record['crates'][PACKAGES[0]]['status'] = 'pending'
        (directory / 'scoped.json').write_text(json.dumps(record))
        self.assertFalse(approval_for(PACKAGES[0], VERSION, directory))


if __name__ == '__main__':
    unittest.main()
