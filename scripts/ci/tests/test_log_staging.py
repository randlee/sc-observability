"""Negative cases at the immutable archive, resolution and approval boundaries."""
import io
import json
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_log_staged_packages import package_command
from _log_staging import PRIVATE_PACKAGE, PACKAGES, inspect_archive, sha256, verify_stage
from validate_log_staged_consumer import validate_resolution
from validate_public_api import approval_for
from _log_release_adaptations import apply_release_adaptations, blob
from wait_for_registry_version import wait
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


class PackageSelectionTests(unittest.TestCase):
    def metadata(self):
        names = [*PACKAGES, PRIVATE_PACKAGE, "sc-observability-dto", "unrelated-public-package"]
        return {"workspace_members": names, "packages": [
            {"id": name, "name": name, "publish": [] if name == PRIVATE_PACKAGE else None}
            for name in names]}

    def test_extra_public_packages_do_not_expand_candidate(self):
        command = package_command(self.metadata(), Path("build"))
        selected = [command[i + 1] for i, value in enumerate(command) if value == "-p"]
        self.assertEqual(selected, list(PACKAGES))
        self.assertNotIn("--workspace", command)
        self.assertNotIn(PRIVATE_PACKAGE, command)

    def test_selected_package_must_exist_and_be_public(self):
        metadata = self.metadata()
        metadata["packages"][0]["publish"] = []
        with self.assertRaisesRegex(ValueError, "missing or private"):
            package_command(metadata, Path("build"))
        metadata["packages"].pop(0)
        with self.assertRaisesRegex(ValueError, "missing or private"):
            package_command(metadata, Path("build"))

    def test_private_consumer_cannot_become_public(self):
        metadata = self.metadata()
        metadata["packages"][len(PACKAGES)]["publish"] = None
        with self.assertRaisesRegex(ValueError, "remain private"):
            package_command(metadata, Path("build"))


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

    def test_rejects_workspace_inheritance_in_nine_crate_validation(self):
        path = self.root / 'workspace.crate'
        archive(path, PACKAGES[0], extra={'Cargo.toml': b'[workspace]\n[package]\nname = "sc-observability-types"\nversion = "1.4.0"\nlicense = "MIT"\n'})
        with self.assertRaisesRegex(ValueError, 'inherits workspace'):
            inspect_archive(path, PACKAGES[0], VERSION, SOURCE, package_names=PACKAGES + ('sc-observability-dto', 'sc-observability-binding-runtime', 'sc-observability-tauri'))

    def test_rejects_inconsistent_first_party_version_in_nine_crate_validation(self):
        path = self.root / 'mismatch.crate'
        archive(path, PACKAGES[0], dependency=f'\n[dependencies.{PACKAGES[1]}]\nversion = "1.3.0"\n')
        with self.assertRaisesRegex(ValueError, 'first-party version mismatch'):
            inspect_archive(path, PACKAGES[0], VERSION, SOURCE, package_names=PACKAGES + ('sc-observability-dto', 'sc-observability-binding-runtime', 'sc-observability-tauri'))

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

    def test_future_publication_visibility_fails_after_bounded_retries(self):
        with patch('wait_for_registry_version.visible', return_value=False) as probe, patch('wait_for_registry_version.time.sleep') as sleep:
            with self.assertRaisesRegex(RuntimeError, 'exhausted after 3'):
                wait('sc-observability', VERSION, 3, 0)
            self.assertEqual(probe.call_count, 3)
            self.assertEqual(sleep.call_count, 2)
        with patch('wait_for_registry_version.visible', side_effect=[False, True]) as probe, patch('wait_for_registry_version.time.sleep'):
            wait('sc-observability', VERSION, 3, 0)
            self.assertEqual(probe.call_count, 2)

    def test_release_adaptation_rejects_unrelated_manifest_and_license_edits(self):
        import hashlib
        root = self.root / 'release'
        root.mkdir()
        (root / 'LICENSE').write_bytes(b'MIT license bytes')
        expected, flags, licenses = {}, {}, {}
        for name in PACKAGES:
            directory = root / 'crates' / name
            directory.mkdir(parents=True)
            (directory / 'LICENSE').write_bytes((root / 'LICENSE').read_bytes())
            licenses[f'crates/{name}/LICENSE'] = blob((root / 'LICENSE').read_bytes())
        for name in ('sc-observability-log', 'sc-observability-log-macros'):
            relative = f'crates/{name}/Cargo.toml'
            before = f'[package]\nname = "{name}"\npublish = false\n'.encode()
            after = before.replace(b'false', b'true')
            (root / relative).write_bytes(after)
            expected[relative] = blob(before)
            flags[relative] = {'before_blob': blob(before), 'after_blob': blob(after)}
        record = root / 'record.json'
        record.write_text(json.dumps({'schema_version': 1, 'candidate_version': VERSION, 'root_license_sha256': hashlib.sha256((root / 'LICENSE').read_bytes()).hexdigest(), 'license_copies': licenses, 'publish_flags': flags}))
        apply_release_adaptations(expected, root, record)
        changed = root / 'crates/sc-observability-log/Cargo.toml'
        original = changed.read_bytes()
        changed.write_bytes(original + b'description = "unrelated edit"\n')
        with self.assertRaisesRegex(ValueError, 'exceeds'):
            apply_release_adaptations(expected, root, record)
        changed.write_bytes(original)
        (root / 'crates/sc-observability-log/LICENSE').write_bytes(b'wrong license')
        with self.assertRaisesRegex(ValueError, 'license copy'):
            apply_release_adaptations(expected, root, record)

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
        record = {'schema_version': 1, 'candidate_version': VERSION, 'crates': {PACKAGES[0]: {'status': 'approved', 'reviewer': 'reviewer', 'evidence': 'review-report', 'scope': ['public-api'], 'api_sha256': 'a' * 64}}}
        (directory / 'scoped.json').write_text(json.dumps(record))
        self.assertTrue(approval_for(PACKAGES[0], VERSION, directory, 'a' * 64))
        self.assertFalse(approval_for(PACKAGES[1], VERSION, directory, 'a' * 64))
        self.assertFalse(approval_for(PACKAGES[0], '1.3.0', directory, 'a' * 64))
        self.assertFalse(approval_for(PACKAGES[0], VERSION, directory, 'b' * 64))
        record['crates'][PACKAGES[0]]['status'] = 'pending'
        (directory / 'scoped.json').write_text(json.dumps(record))
        self.assertFalse(approval_for(PACKAGES[0], VERSION, directory, 'a' * 64))


if __name__ == '__main__':
    unittest.main()
