"""CLI exit-code contracts for the public API validator."""
import contextlib
import io
import sys
import unittest
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from validate_public_api import cli  # noqa: E402


class PublicApiCliTests(unittest.TestCase):
    def test_unhandled_validator_failure_uses_distinct_crash_exit_code(self):
        stderr = io.StringIO()
        with patch('validate_public_api.main', side_effect=ValueError('version mismatch')):
            with contextlib.redirect_stderr(stderr):
                status = cli()

        self.assertEqual(status, 3)
        self.assertIn('ValueError: version mismatch', stderr.getvalue())


class MajorBreakTests(unittest.TestCase):
    OLD = 'pub fn sc_observability_dto::from_core_health(Health, Level) -> Result<Dto, Failure>'
    NEW = 'pub fn sc_observability_dto::from_core_health(Health, Level) -> Dto'

    def report(self, old=None, new=None):
        return ('Removed items from the public API\n(none)\n'
                'Changed items in the public API\n-' + (old or self.OLD) + '\n+'
                + (new or self.NEW) + '\nAdded items to the public API\n(none)\n')

    def entry(self):
        return {'id': 'health', 'crate': 'sc-observability-dto', 'old': self.OLD,
                'new': self.NEW}

    def test_exact_observed_break_passes_but_omitted_entry_fails(self):
        from validate_public_api import check_major_diff
        self.assertEqual(check_major_diff('sc-observability-dto', self.report(), [self.entry()]), [])
        missing = check_major_diff('sc-observability-dto', self.report(), [])
        self.assertEqual(missing, ['unlisted API break: ' + self.OLD])

    def test_changed_replacement_and_unrelated_removal_are_rejected(self):
        from validate_public_api import check_major_diff
        self.assertIn('replacement differs', check_major_diff(
            'sc-observability-dto', self.report(new='pub fn changed()'), [self.entry()])[0])
        self.assertTrue(any('unlisted API break' in error for error in check_major_diff(
            'sc-observability-dto', self.report() + '-pub fn unrelated()\n', [self.entry()])))

    def test_other_crate_or_stale_entry_cannot_authorize_break(self):
        from validate_public_api import check_major_diff
        self.assertTrue(check_major_diff('other-crate', self.report(), [self.entry()]))
        unchanged = self.report().replace('-' + self.OLD + '\n', '').replace('+' + self.NEW + '\n', '')
        self.assertEqual(check_major_diff('sc-observability-dto', unchanged, [self.entry()]),
                         ['stale break entry: health'])

    def test_tool_output_must_contain_diff_sections(self):
        from validate_public_api import check_major_diff
        with self.assertRaisesRegex(ValueError, 'diff sections'):
            check_major_diff('sc-observability-dto', 'error: rustdoc failed', [self.entry()])

    def test_additions_do_not_need_break_waivers(self):
        from validate_public_api import check_major_diff
        output = self.report().replace('-' + self.OLD + '\n', '')
        self.assertEqual(check_major_diff('sc-observability-dto', output, []), [])

    def test_invalid_manifest_baseline_and_duplicate_entries_fail_closed(self):
        import tempfile
        from validate_public_api import major_breaks
        policy = {'candidate_version': '2.0.0', 'crates': {'sc-observability-dto': {}}}
        entry = {'id': 'health', 'crate': 'sc-observability-dto', 'old': self.OLD,
                 'new': self.NEW, 'authority': 'ADR-017', 'reason': 'infallible projection',
                 'migration': 'docs/migration.md'}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            (root / 'release/public-api-major-breaks.toml').write_text('fixture')
            for baseline, entries in [('1.4.0', [entry]), ('1.4.1', [entry, entry])]:
                manifest = {'schema_version': 1, 'candidate_version': '2.0.0',
                            'baseline_version': baseline, 'breaks': entries}
                with self.subTest(baseline=baseline), patch('validate_public_api.ROOT', root), \
                        patch('validate_public_api.tomllib.loads', return_value=manifest), \
                        self.assertRaises(ValueError):
                    major_breaks(policy)


    def test_manifest_cannot_hide_structural_semver_failure(self):
        import json
        import tempfile
        from subprocess import CompletedProcess
        from validate_public_api import main
        crate = 'sc-observability-dto'
        package = {'name': crate, 'version': '2.0.0', 'manifest_path': 'dto/Cargo.toml',
                   'targets': [{'kind': ['lib']}]}
        policy = {'candidate_version': '2.0.0',
                  'crates': {crate: {'baseline_version': '1.4.1', 'kind': 'lib'}}}
        outputs = [CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                   CompletedProcess([], 0, 'head', ''),
                   CompletedProcess([], 0, self.report(), ''),
                   CompletedProcess([], 1, '', 'required trait item added')]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'release').mkdir()
            (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
            with patch('validate_public_api.ROOT', root), \
                    patch('validate_public_api.CACHE', root / 'cache'), \
                    patch('validate_public_api.major_breaks', return_value=[self.entry()]), \
                    patch('validate_public_api.run', side_effect=outputs), \
                    patch('sys.argv', ['validate_public_api.py', 'semver']), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(main(), 2)
            report = json.loads((root / 'cache/public-api-semver.json').read_text())
            self.assertEqual(report['crates'][crate]['status'], 'structural-semver-failed')



if __name__ == '__main__':
    unittest.main()
