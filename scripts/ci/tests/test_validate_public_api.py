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

from validate_public_api import (  # noqa: E402
    cli, compatible_diff_problems, main, validate_api_package_roster,
    validate_compatible_policy,
)
from validate_version_literals import (  # noqa: E402
    validate_cargo_lock, validate_inventory_candidate, validate_package_lock,
)


class CompatiblePolicyTests(unittest.TestCase):
    deferred_tauri = {
        'package': 'sc-observability-tauri',
        'baselineVersion': '1.4.1',
        'reason': (
            'The Tauri adapter is a separate workspace and remains pending its '
            'standalone API/publication qualification in '
            'release/bindings-artifacts.toml; it is not one of this candidate\'s '
            'nine workspace API packages.'
        ),
    }

    def policy(self):
        return {
            'schema_version': 1,
            'candidate_version': '1.5.0',
            'crates': {'sc-observability-log-macros': {
                'baseline_version': '1.4.1', 'kind': 'proc-macro',
            }},
        }

    @classmethod
    def write_package_roster(cls, root, *, candidate_packages=None, deferred=None,
                             published_packages=None):
        candidate_packages = candidate_packages or ['sc-observability-log-macros']
        deferred = [cls.deferred_tauri.copy()] if deferred is None else deferred
        published_packages = published_packages or [
            'sc-observability-log-macros', 'sc-observability-tauri',
        ]
        (root / 'release/release-inventory.json').write_text(json.dumps({
            'qualificationCandidate': {
                'packages': candidate_packages,
                'deferredStandalonePackages': deferred,
            },
        }))
        (root / 'release/publish-artifacts.toml').write_text(
            ''.join(f'[[crates]]\npackage = "{package}"\n' for package in published_packages)
        )

    def test_package_roster_requires_exact_candidate_and_deferred_coverage(self):
        valid_inventory = {
            'qualificationCandidate': {
                'packages': ['sc-observability-log-macros'],
                'deferredStandalonePackages': [self.deferred_tauri.copy()],
            },
        }
        valid_artifacts = {'crates': [
            {'package': 'sc-observability-log-macros'},
            {'package': 'sc-observability-tauri'},
        ]}
        validate_api_package_roster(self.policy(), valid_inventory, valid_artifacts)

        cases = [
            ('duplicate candidate', {
                'qualificationCandidate': {
                    'packages': ['sc-observability-log-macros', 'sc-observability-log-macros'],
                    'deferredStandalonePackages': [self.deferred_tauri.copy()],
                },
            }, valid_artifacts, 'duplicate packages'),
            ('duplicate deferred package', {
                'qualificationCandidate': {
                    'packages': ['sc-observability-log-macros'],
                    'deferredStandalonePackages': [
                        self.deferred_tauri.copy(), self.deferred_tauri.copy(),
                    ],
                },
            }, valid_artifacts, 'contains duplicate packages'),
            ('omitted artifact', valid_inventory, {
                'crates': valid_artifacts['crates'] + [{'package': 'sc-observability-dto'}],
            }, r"omitted=\['sc-observability-dto'\]"),
            ('duplicate artifact', valid_inventory, {
                'crates': valid_artifacts['crates'] + [{'package': 'sc-observability-tauri'}],
            }, 'duplicate crate packages'),
            ('changed exemption metadata', {
                'qualificationCandidate': {
                    'packages': ['sc-observability-log-macros'],
                    'deferredStandalonePackages': [{
                        **self.deferred_tauri, 'baselineVersion': '1.4.0',
                    }],
                },
            }, valid_artifacts, 'exact approved exemption'),
            ('missing approved deferral', {
                'qualificationCandidate': {
                    'packages': ['sc-observability-log-macros'],
                    'deferredStandalonePackages': [],
                },
            }, valid_artifacts, 'exact approved exemption'),
        ]
        for name, inventory, artifacts, error in cases:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, error):
                validate_api_package_roster(self.policy(), inventory, artifacts)

        unknown_inventory = {
            'qualificationCandidate': {
                'packages': ['sc-observability-log-macros', 'sc-observability-unknown'],
                'deferredStandalonePackages': [self.deferred_tauri.copy()],
            },
        }
        policy_with_unknown = self.policy()
        policy_with_unknown['crates']['sc-observability-unknown'] = {
            'baseline_version': '1.4.1', 'kind': 'lib',
        }
        with self.assertRaisesRegex(ValueError, r"unknown=\['sc-observability-unknown'\]"):
            validate_api_package_roster(policy_with_unknown, unknown_inventory, valid_artifacts)

    def test_policy_crates_must_match_candidate_packages_independently(self):
        cases = [
            ('policy omits candidate',
             ['sc-observability-log-macros', 'sc-observability'],
             {'sc-observability-log-macros'}),
            ('policy adds non-candidate',
             ['sc-observability-log-macros'],
             {'sc-observability-log-macros', 'sc-observability'}),
        ]
        for name, candidate_packages, policy_packages in cases:
            inventory = {
                'qualificationCandidate': {
                    'packages': candidate_packages,
                    'deferredStandalonePackages': [self.deferred_tauri.copy()],
                },
            }
            artifacts = {'crates': [
                {'package': 'sc-observability-log-macros'},
                *([{'package': 'sc-observability'}]
                  if 'sc-observability' in candidate_packages else []),
                {'package': 'sc-observability-tauri'},
            ]}
            policy = {'crates': {package: {} for package in policy_packages}}
            with self.subTest(name=name), self.assertRaisesRegex(
                    ValueError,
                    'public API policy must name exactly the qualification candidate packages'):
                validate_api_package_roster(policy, inventory, artifacts)

    def test_candidate_and_exact_deferred_package_overlap_is_rejected(self):
        inventory = {
            'qualificationCandidate': {
                'packages': ['sc-observability-log-macros', 'sc-observability-tauri'],
                'deferredStandalonePackages': [self.deferred_tauri.copy()],
            },
        }
        policy = {'crates': {
            'sc-observability-log-macros': {},
            'sc-observability-tauri': {},
        }}
        artifacts = {'crates': [
            {'package': 'sc-observability-log-macros'},
            {'package': 'sc-observability-tauri'},
        ]}
        with self.assertRaisesRegex(
                ValueError, 'candidate and deferred API package sets overlap'):
            validate_api_package_roster(policy, inventory, artifacts)

    def test_package_roster_shape_guards_report_specific_value_errors(self):
        valid_inventory = {
            'qualificationCandidate': {
                'packages': ['sc-observability-log-macros'],
                'deferredStandalonePackages': [self.deferred_tauri.copy()],
            },
        }
        valid_artifacts = {'crates': [
            {'package': 'sc-observability-log-macros'},
            {'package': 'sc-observability-tauri'},
        ]}
        cases = [
            ('candidate is not an object',
             {'qualificationCandidate': None}, valid_artifacts,
             'release inventory must define qualificationCandidate'),
            ('candidate packages is not a list',
             {'qualificationCandidate': {
                 'packages': None,
                 'deferredStandalonePackages': [self.deferred_tauri.copy()],
             }}, valid_artifacts,
             'qualificationCandidate.packages must be a list of package names'),
            ('candidate package is not a string',
             {'qualificationCandidate': {
                 'packages': ['sc-observability-log-macros', None],
                 'deferredStandalonePackages': [self.deferred_tauri.copy()],
             }}, valid_artifacts,
             'qualificationCandidate.packages must be a list of package names'),
            ('candidate package is empty',
             {'qualificationCandidate': {
                 'packages': [''],
                 'deferredStandalonePackages': [self.deferred_tauri.copy()],
             }}, valid_artifacts,
             'qualificationCandidate.packages must be a list of package names'),
            ('deferred packages is not a list',
             {'qualificationCandidate': {
                 'packages': ['sc-observability-log-macros'],
                 'deferredStandalonePackages': None,
             }}, valid_artifacts,
             'qualificationCandidate.deferredStandalonePackages must be a list of package records'),
            ('deferred record is not an object',
             {'qualificationCandidate': {
                 'packages': ['sc-observability-log-macros'],
                 'deferredStandalonePackages': [None],
             }}, valid_artifacts,
             'qualificationCandidate.deferredStandalonePackages must be a list of package records'),
            ('deferred record package is missing',
             {'qualificationCandidate': {
                 'packages': ['sc-observability-log-macros'],
                 'deferredStandalonePackages': [{'reason': 'fixture'}],
             }}, valid_artifacts,
             'deferred standalone API package records must name a package'),
            ('deferred record package is empty',
             {'qualificationCandidate': {
                 'packages': ['sc-observability-log-macros'],
                 'deferredStandalonePackages': [{'package': ''}],
             }}, valid_artifacts,
             'deferred standalone API package records must name a package'),
            ('artifact crates is not a list',
             valid_inventory, {'crates': None},
             'publish-artifacts manifest must define a crates list'),
            ('artifact crate is not an object',
             valid_inventory, {'crates': [None]},
             'publish-artifacts manifest must define a crates list'),
            ('artifact package is missing',
             valid_inventory, {'crates': [{}]},
             'every publish-artifacts crate must name a package'),
            ('artifact package is empty',
             valid_inventory, {'crates': [{'package': ''}]},
             'every publish-artifacts crate must name a package'),
        ]
        for name, inventory, artifacts, error in cases:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, error):
                validate_api_package_roster(self.policy(), inventory, artifacts)

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
            self.write_package_roster(root)
            cases = [
                ('explicit empty break list',
                 'schema_version = 1\nbaseline_version = "1.4.1"\n'
                 'candidate_version = "1.5.0"\nbreaks = []\n', True),
                ('wrong baseline',
                 'schema_version = 1\nbaseline_version = "1.4.0"\n'
                 'candidate_version = "1.5.0"\nbreaks = []\n', False),
                ('major candidate',
                 'schema_version = 1\nbaseline_version = "1.4.1"\n'
                 'candidate_version = "2.0.0"\nbreaks = []\n', False),
                ('nonempty break list',
                 'schema_version = 1\nbaseline_version = "1.4.1"\n'
                 'candidate_version = "1.5.0"\nbreaks = [{ id = "waiver" }]\n', False),
                ('non-list breaks',
                 'schema_version = 1\nbaseline_version = "1.4.1"\n'
                 'candidate_version = "1.5.0"\nbreaks = "none"\n', False),
                ('missing breaks key',
                 'schema_version = 1\nbaseline_version = "1.4.1"\n'
                 'candidate_version = "1.5.0"\n', False),
                ('misspelled breaks key',
                 'schema_version = 1\nbaseline_version = "1.4.1"\n'
                 'candidate_version = "1.5.0"\nbreak = [{ id = "waiver" }]\n', False),
            ]
            for name, contents, accepted in cases:
                with self.subTest(name=name):
                    manifest_path.write_text(contents)
                    with patch('validate_public_api.ROOT', root):
                        if accepted:
                            self.assertIsNone(validate_compatible_policy(self.policy()))
                        else:
                            with self.assertRaises(ValueError):
                                validate_compatible_policy(self.policy())

    def test_cli_rejects_missing_or_misspelled_break_key_before_api_tools(self):
        crate = 'sc-observability-log-macros'
        package = {'name': crate, 'version': '1.5.0', 'manifest_path': 'macros/Cargo.toml',
                   'targets': [{'kind': ['proc-macro']}]}
        policy = {'schema_version': 1, 'candidate_version': '1.5.0', 'crates': {
            crate: {'baseline_version': '1.4.1', 'kind': 'proc-macro'}}}
        invalid_manifests = {
            'missing': ('schema_version = 1\nbaseline_version = "1.4.1"\n'
                        'candidate_version = "1.5.0"\n'),
            'misspelled': ('schema_version = 1\nbaseline_version = "1.4.1"\n'
                           'candidate_version = "1.5.0"\n'
                           'break = [{ id = "waiver" }]\n'),
        }

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
            self.write_package_roster(root, candidate_packages=[crate])
            for name, manifest in invalid_manifests.items():
                with self.subTest(name=name):
                    (root / 'release/public-api-major-breaks.toml').write_text(manifest)
                    stderr = io.StringIO()
                    with patch('validate_public_api.ROOT', root), \
                            patch('validate_public_api.CACHE', root / 'cache'), \
                            patch('validate_public_api.run', return_value=CompletedProcess(
                                ['cargo', 'metadata'], 0,
                                json.dumps({'packages': [package]}), '')) as mocked_run, \
                            patch('sys.argv', ['validate_public_api.py', 'semver']), \
                            contextlib.redirect_stdout(io.StringIO()), \
                            contextlib.redirect_stderr(stderr):
                        self.assertEqual(cli(), 3)
                    self.assertEqual(mocked_run.call_count, 1)
                    self.assertIn('compatible 1.x release cannot accept enumerated breaking API exceptions',
                                  stderr.getvalue())

    def test_missing_or_wrong_package_baseline_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            (root / 'release/public-api-major-breaks.toml').write_text(
                'schema_version = 1\nbaseline_version = "1.4.1"\n'
                'candidate_version = "1.5.0"\nbreaks = []\n')
            self.write_package_roster(root)
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
                self.write_package_roster(root)
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

    def test_lib_semver_command_uses_baseline_release_type_all_features_and_fails_closed(self):
        crate = 'sc-observability'
        package = {'name': crate, 'version': '1.5.0', 'manifest_path': 'crates/sc-observability/Cargo.toml',
                   'targets': [{'kind': ['lib']}]}
        policy = {'schema_version': 1, 'candidate_version': '1.5.0', 'crates': {
            crate: {'baseline_version': '1.4.1', 'kind': 'lib'}}}
        valid_manifest = ('schema_version = 1\nbaseline_version = "1.4.1"\n'
                          'candidate_version = "1.5.0"\nbreaks = []\n')

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            CompatiblePolicyTests.write_package_roster(
                root, candidate_packages=[crate],
                published_packages=[crate, 'sc-observability-tauri'],
            )
            (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
            (root / 'release/public-api-major-breaks.toml').write_text(valid_manifest)
            commands = [
                CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                CompletedProcess([], 0, 'fixture-head', ''),
                CompletedProcess(['cargo', 'semver-checks'], 1, '', 'fixture semver failure'),
            ]
            with patch('validate_public_api.ROOT', root), \
                    patch('validate_public_api.CACHE', root / 'cache'), \
                    patch('validate_public_api.run', side_effect=commands) as mocked_run, \
                    patch('sys.argv', ['validate_public_api.py', 'semver']), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(main(), 2)

            semver_command = mocked_run.call_args_list[-1].args[0]
            self.assertEqual(semver_command[:2], ['cargo', 'semver-checks'])
            self.assertIn('--baseline-version', semver_command)
            self.assertEqual(semver_command[semver_command.index('--baseline-version') + 1], '1.4.1')
            self.assertEqual(semver_command[semver_command.index('--release-type') + 1], 'minor')
            self.assertIn('--all-features', semver_command)
            report = json.loads((root / 'cache/public-api-semver.json').read_text())
            self.assertEqual(report['crates'][crate]['status'], 'tool-error')
            self.assertEqual(report['crates'][crate]['exit_code'], 1)
            self.assertEqual(report['crates'][crate]['command'], semver_command)

    def test_blocking_semver_mode_rejects_an_unaccounted_published_crate(self):
        crate = 'sc-observability-log-macros'
        package = {'name': crate, 'version': '1.5.0', 'manifest_path': 'macros/Cargo.toml',
                   'targets': [{'kind': ['proc-macro']}]}
        policy = self.policy()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
            (root / 'release/public-api-major-breaks.toml').write_text(
                'schema_version = 1\nbaseline_version = "1.4.1"\n'
                'candidate_version = "1.5.0"\nbreaks = []\n')
            self.write_package_roster(root, published_packages=[
                'sc-observability-log-macros', 'sc-observability-tauri', 'sc-observability-dto',
            ])
            stderr = io.StringIO()
            with patch('validate_public_api.ROOT', root), \
                    patch('validate_public_api.CACHE', root / 'cache'), \
                    patch('validate_public_api.run', return_value=CompletedProcess(
                        [], 0, json.dumps({'packages': [package]}), '')), \
                    patch('sys.argv', ['validate_public_api.py', 'semver']), \
                    contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(stderr):
                self.assertEqual(cli(), 3)
        self.assertIn('omitted=', stderr.getvalue())

class DocsApprovalEvidenceTests(unittest.TestCase):
    def test_changed_api_without_approval_evidence_fails_closed(self):
        crate = 'sc-observability-log-macros'
        api_sha256 = 'a' * 64
        package = {'name': crate, 'version': '1.5.0', 'manifest_path': 'macros/Cargo.toml',
                   'targets': [{'kind': ['proc-macro']}]}
        policy = {'schema_version': 1, 'candidate_version': '1.5.0', 'crates': {
            crate: {'baseline_version': '1.4.1', 'kind': 'proc-macro'}}}
        report = {'source_commit': 'fixture-head', 'candidate_version': '1.5.0', 'crates': {
            crate: {'status': 'changed', 'api_sha256': api_sha256}}}
        # This otherwise matching approval is invalid because it has no evidence.
        approval = {'schema_version': 1, 'candidate_version': '1.5.0', 'crates': {
            crate: {'status': 'approved', 'reviewer': 'quality-mgr',
                    'scope': ['public-api'], 'api_sha256': api_sha256}}}

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            CompatiblePolicyTests.write_package_roster(
                root, candidate_packages=[crate],
                published_packages=[crate, 'sc-observability-tauri'],
            )
            (root / 'target/public-api').mkdir(parents=True)
            (root / 'docs/api-approvals').mkdir(parents=True)
            (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
            (root / 'release/public-api-major-breaks.toml').write_text(
                'schema_version = 1\nbaseline_version = "1.4.1"\n'
                'candidate_version = "1.5.0"\nbreaks = []\n')
            (root / 'target/public-api/public-api-diff.json').write_text(json.dumps(report))
            (root / 'docs/api-approvals/fixture.json').write_text(json.dumps(approval))
            commands = [
                CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                CompletedProcess([], 0, 'fixture-head', ''),
                CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                CompletedProcess([], 0, 'fixture-head', ''),
            ]
            stderr = io.StringIO()
            with patch('validate_public_api.ROOT', root), \
                    patch('validate_public_api.CACHE', root / 'target/public-api'), \
                    patch('validate_public_api.run', side_effect=commands), \
                    patch('sys.argv', ['validate_public_api.py', 'docs']), \
                    contextlib.redirect_stderr(stderr):
                self.assertEqual(main(), 1)
                self.assertIn(f'missing_scoped_approvals=[\'{crate}\']', stderr.getvalue())

                # A non-empty evidence value is the positive control: the same
                # otherwise matching approval should satisfy the docs gate.
                approval['crates'][crate]['evidence'] = 'positive-control evidence'
                (root / 'docs/api-approvals/fixture.json').write_text(json.dumps(approval))
                self.assertEqual(main(), 0)

    def test_docs_mode_rejects_an_approved_removed_api_under_compatible_policy(self):
        crate = 'sc-observability-log-macros'
        api_sha256 = 'b' * 64
        package = {'name': crate, 'version': '1.5.0', 'manifest_path': 'macros/Cargo.toml',
                   'targets': [{'kind': ['proc-macro']}]}
        policy = {'schema_version': 1, 'candidate_version': '1.5.0', 'crates': {
            crate: {'baseline_version': '1.4.1', 'kind': 'proc-macro'}}}
        report = {'source_commit': 'fixture-head', 'candidate_version': '1.5.0', 'crates': {
            crate: {'status': 'changed', 'api_sha256': api_sha256}}}
        approval = {'schema_version': 1, 'candidate_version': '1.5.0', 'crates': {
            crate: {'status': 'approved', 'reviewer': 'quality-mgr',
                    'scope': ['public-api'], 'api_sha256': api_sha256,
                    'evidence': 'reviewed removed API fixture'}}}

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            CompatiblePolicyTests.write_package_roster(
                root, candidate_packages=[crate],
                published_packages=[crate, 'sc-observability-tauri'],
            )
            (root / 'target/public-api').mkdir(parents=True)
            (root / 'docs/api-approvals').mkdir(parents=True)
            (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
            manifest_path = root / 'release/public-api-major-breaks.toml'
            manifest_path.write_text(
                'schema_version = 1\nbaseline_version = "1.4.1"\n'
                'candidate_version = "1.5.0"\nbreaks = []\n')
            (root / 'target/public-api/public-api-diff.json').write_text(json.dumps(report))
            (root / 'docs/api-approvals/fixture.json').write_text(json.dumps(approval))
            commands = [
                CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                CompletedProcess([], 0, 'fixture-head', ''),
                CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                CompletedProcess([], 0, 'fixture-head', ''),
            ]
            stderr = io.StringIO()
            with patch('validate_public_api.ROOT', root), \
                    patch('validate_public_api.CACHE', root / 'target/public-api'), \
                    patch('validate_public_api.run', side_effect=commands), \
                    patch('sys.argv', ['validate_public_api.py', 'docs']), \
                    contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(stderr):
                self.assertEqual(cli(), 0)

                # A matching scoped approval cannot waive a declared API removal.
                manifest_path.write_text(
                    'schema_version = 1\nbaseline_version = "1.4.1"\n'
                    'candidate_version = "1.5.0"\n'
                    'breaks = [{ id = "removed-public-api" }]\n')
                self.assertEqual(cli(), 3)
                self.assertIn('cannot accept enumerated breaking API exceptions',
                              stderr.getvalue())



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
