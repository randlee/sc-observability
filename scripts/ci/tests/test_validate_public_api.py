"""Fail-closed fixtures for the compatible 1.x public API gate."""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from subprocess import CompletedProcess
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from validate_public_api import cli, compatible_diff_problems, main, validate_compatible_policy  # noqa: E402
from validate_version_literals import (  # noqa: E402
    validate_cargo_lock, validate_inventory_candidate, validate_package_lock,
)


class CompatiblePolicyTests(unittest.TestCase):
    def policy(self):
        return {
            'schema_version': 1,
            'candidate_version': '1.5.0',
            'crates': {'sc-observability-log-macros': {
                'baseline_version': '1.4.1', 'kind': 'proc-macro',
            }},
        }

    def diff(self, *, removal='', changed='', addition=''):
        return ('Removed items from the public API\n' + (removal or '(none)') + '\n'
                'Changed items in the public API\n' + (changed or '(none)') + '\n'
                'Added items to the public API\n' + (addition or '(none)') + '\n')

    def test_identical_and_additive_api_pass(self):
        self.assertEqual(compatible_diff_problems(self.diff()), [])
        self.assertEqual(compatible_diff_problems(
            self.diff(addition='+pub fn sc_observability_log_macros::new_api()')), [])

    def test_breaking_and_signature_change_fail(self):
        self.assertTrue(compatible_diff_problems(
            self.diff(removal='-pub fn sc_observability_log_macros::old_api()')))
        self.assertTrue(compatible_diff_problems(self.diff(
            changed='-pub fn sc_observability_log_macros::api() -> Old\n'
                    '+pub fn sc_observability_log_macros::api() -> New')))

    def test_missing_diff_sections_fail_closed(self):
        with self.assertRaisesRegex(ValueError, 'diff sections'):
            compatible_diff_problems('cargo-public-api failed')

    def test_manifest_requires_exact_candidate_baseline_and_no_exceptions(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            manifest_path = root / 'release/public-api-major-breaks.toml'
            cases = [
                ('schema_version = 1\nbaseline_version = "1.4.1"\ncandidate_version = "1.5.0"\nbreaks = []\n', True),
                ('schema_version = 1\nbaseline_version = "1.4.0"\ncandidate_version = "1.5.0"\nbreaks = []\n', False),
                ('schema_version = 1\nbaseline_version = "1.4.1"\ncandidate_version = "2.0.0"\nbreaks = []\n', False),
                ('schema_version = 1\nbaseline_version = "1.4.1"\ncandidate_version = "1.5.0"\nbreaks = [{ id = "waiver" }]\n', False),
            ]
            for contents, accepted in cases:
                with self.subTest(contents=contents):
                    manifest_path.write_text(contents)
                    with patch('validate_public_api.ROOT', root):
                        if accepted:
                            self.assertIsNone(validate_compatible_policy(self.policy()))
                        else:
                            with self.assertRaises(ValueError):
                                validate_compatible_policy(self.policy())

    def test_missing_or_wrong_package_baseline_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            (root / 'release/public-api-major-breaks.toml').write_text(
                'schema_version = 1\nbaseline_version = "1.4.1"\n'
                'candidate_version = "1.5.0"\nbreaks = []\n')
            crate = 'sc-observability-log-macros'
            package = {'name': crate, 'version': '1.5.0', 'manifest_path': 'macros/Cargo.toml',
                       'targets': [{'kind': ['proc-macro']}]}
            for baseline in (None, '1.4.0'):
                with self.subTest(baseline=baseline):
                    policy = self.policy()
                    policy['crates'][crate]['baseline_version'] = baseline
                    (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
                    with patch('validate_public_api.ROOT', root), \
                            patch('validate_public_api.CACHE', root / 'cache'), \
                            patch('validate_public_api.run', return_value=CompletedProcess(
                                [], 0, json.dumps({'packages': [package]}), '')), \
                            patch('sys.argv', ['validate_public_api.py', 'semver']), \
                            contextlib.redirect_stdout(io.StringIO()), \
                            contextlib.redirect_stderr(io.StringIO()):
                        self.assertEqual(cli(), 3)

    def test_blocking_semver_mode_accepts_additions_and_rejects_break_fixture(self):
        crate = 'sc-observability-log-macros'
        package = {'name': crate, 'version': '1.5.0', 'manifest_path': 'macros/Cargo.toml',
                   'targets': [{'kind': ['proc-macro']}]}
        policy = {'schema_version': 1, 'candidate_version': '1.5.0', 'crates': {
            crate: {'baseline_version': '1.4.1', 'kind': 'proc-macro'}}}
        valid_manifest = ('schema_version = 1\nbaseline_version = "1.4.1"\n'
                          'candidate_version = "1.5.0"\nbreaks = []\n')
        diffs = {
            'additive': self.diff(addition='+pub fn sc_observability_log_macros::new_api()'),
            'breaking': self.diff(removal='-pub fn sc_observability_log_macros::old_api()'),
        }
        for kind, diff in diffs.items():
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / 'release').mkdir()
                (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
                (root / 'release/public-api-major-breaks.toml').write_text(valid_manifest)
                commands = [
                    CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                    CompletedProcess([], 0, 'fixture-head', ''),
                    CompletedProcess([], 0, diff, ''),
                ]
                with patch('validate_public_api.ROOT', root), \
                        patch('validate_public_api.CACHE', root / 'cache'), \
                        patch('validate_public_api.run', side_effect=commands), \
                        patch('sys.argv', ['validate_public_api.py', 'semver']), \
                        contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(main(), 0 if kind == 'additive' else 2)
                report = json.loads((root / 'cache/public-api-semver.json').read_text())
                self.assertEqual(report['crates'][crate]['status'],
                                 'compatible-proc-macro-api' if kind == 'additive'
                                 else 'proc-macro-api-incompatible')


class PublicApiCliTests(unittest.TestCase):
    def test_unhandled_validator_failure_uses_distinct_crash_exit_code(self):
        stderr = io.StringIO()
        with patch('validate_public_api.main', side_effect=ValueError('version mismatch')):
            with contextlib.redirect_stderr(stderr):
                status = cli()

        self.assertEqual(status, 3)
        self.assertIn('ValueError: version mismatch', stderr.getvalue())


class CandidateVersionLockTests(unittest.TestCase):
    def test_cargo_lock_rejects_a_stale_sc_observability_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            lock.write_text('[[package]]\nname = "sc-observability-types"\nversion = "2.0.0"\n')
            with self.assertRaisesRegex(ValueError, "candidate version 1.5.0"):
                validate_cargo_lock(lock, "1.5.0")

    def test_cargo_lock_accepts_candidate_package_versions(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            lock.write_text('[[package]]\nname = "sc-observability-types"\nversion = "1.5.0"\n')
            validate_cargo_lock(lock, "1.5.0")

    def test_package_lock_rejects_stale_candidate(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "package-lock.json"
            lock.write_text(json.dumps({"name": "@synaptic-canvas/sc-observability", "version": "2.0.0",
                                       "packages": {"": {"name": "@synaptic-canvas/sc-observability",
                                                           "version": "2.0.0"}}}))
            with self.assertRaisesRegex(ValueError, "candidate version 1.5.0"):
                validate_package_lock(lock, "1.5.0")

    def test_inventory_candidate_must_match_workspace_candidate(self):
        with tempfile.TemporaryDirectory() as temporary:
            inventory = Path(temporary) / "release-inventory.json"
            inventory.write_text(json.dumps({"releaseVersion": "1.1.0",
                                             "qualificationCandidate": {"version": "1.5.0"}}))
            validate_inventory_candidate(inventory, "1.5.0")
            inventory.write_text(json.dumps({"releaseVersion": "1.1.0",
                                             "qualificationCandidate": {"version": "2.0.0"}}))
            with self.assertRaisesRegex(ValueError, "qualificationCandidate.version must be 1.5.0"):
                validate_inventory_candidate(inventory, "1.5.0")


if __name__ == '__main__':
    unittest.main()
