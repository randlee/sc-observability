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

class StructuralDiagnosticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import tomllib
        cls.fixtures = Path(__file__).parent / 'fixtures/semver'
        cls.entries = tomllib.loads((Path(__file__).resolve().parents[3] /
            'release/public-api-major-breaks.toml').read_text())['breaks']

    def output(self, crate):
        return (self.fixtures / (crate + '.stdout')).read_text()

    def check(self, crate, output=None, status=1, stderr=''):
        from validate_public_api import structural_diagnostics_are_enumerated
        return structural_diagnostics_are_enumerated('sc-observability-' + crate,
            self.output(crate) if output is None else output, self.entries, status, stderr)

    def test_real_approved_diagnostics(self):
        for crate in ('otlp', 'log'):
            with self.subTest(crate=crate):
                self.assertTrue(self.check(crate, stderr=(self.fixtures / (crate + '.stderr')).read_text()))

    def test_every_extra_or_wrong_failed_in_item_is_rejected(self):
        otlp, log = self.output('otlp'), self.output('log')
        cases = [
            ('otlp', otlp.replace('sc_observability_otlp::constants,', 'sc_observability_otlp::unrelated,')),
            ('otlp', otlp.replace('Failed in:', 'Failed in:\n  mod sc_observability_otlp::unrelated, previously in file /src/lib.rs:1', 1)),
            ('otlp', otlp.replace('DEFAULT_LOG_BATCH_SIZE', 'DEFAULT_UNAPPROVED')),
            ('otlp', otlp.replace('TELEMETRY_SHUTDOWN', 'TELEMETRY_UNAPPROVED')),
            ('otlp', otlp.replace('  ALL in file', '  UNAPPROVED in file')),
            ('otlp', otlp.replace('/src/error_codes.rs:', '/src/unrelated.rs:')),
            ('otlp', otlp.replace('struct OtelConfig in', 'struct OtherConfig in')),
            ('log', log + '  type LogControl is no longer Send, in /repo/crates/sc-observability-log/src/control.rs:43\n'),
            ('log', log + '  type Unrelated is no longer Send, in /repo/crates/sc-observability-log/src/control.rs:43\n'),
            ('log', log.replace('type LogControl', 'type WrongSymbol')),
        ]
        for crate, output in cases:
            with self.subTest(crate=crate, output=output):
                self.assertFalse(self.check(crate, output))

    def test_missing_duplicate_and_unparsed_rows_fail(self):
        output = self.output('log')
        row = next(line for line in output.splitlines() if '  type LogControl' in line)
        for changed in (output.replace(row + '\n', ''), output + row + '\n',
                        output + '  unparsed finding\n', output.replace('Failed in:', 'Items:'),
                        output.replace('Description:', 'Details:'), ''):
            with self.subTest(output=changed):
                self.assertFalse(self.check('log', changed))

    def test_mixed_unknown_or_indented_failure_headers_fail(self):
        for prefix in ('', '  '):
            for crate in ('log', 'otlp'):
                mixed = self.output(crate) + prefix + '--- failure trait_method_added: added ---\n\nDescription:\nnew method\n\nFailed in:\n  trait Other\n'
                with self.subTest(prefix=prefix, crate=crate):
                    self.assertFalse(self.check(crate, mixed))
                    self.assertFalse(self.check(crate, stderr=mixed))

    def test_abnormal_exit_cannot_be_waived(self):
        for status in (0, 2, 101, -9):
            with self.subTest(status=status):
                self.assertFalse(self.check('log', status=status))

    def test_mixed_execution_errors_and_panics_fail(self):
        for error in ('error: rustc failed', 'error[E0308]: mismatch',
                      "thread 'main' panicked at checker.rs:1", 'could not compile crate',
                      "process didn't exit successfully", 'command failed'):
            with self.subTest(error=error):
                self.assertFalse(self.check('log', stderr=error))
                self.assertFalse(self.check('log', self.output('log') + error))

    def test_main_rejects_abnormal_exit_even_with_complete_approved_stdout(self):
        import json
        import tempfile
        from subprocess import CompletedProcess
        from validate_public_api import main
        crate = 'sc-observability-log'
        entries = [entry for entry in self.entries if entry['crate'] == crate]
        package = {'name': crate, 'version': '2.0.0', 'manifest_path': 'log/Cargo.toml',
                   'targets': [{'kind': ['lib']}]}
        policy = {'candidate_version': '2.0.0',
                  'crates': {crate: {'baseline_version': '1.4.1', 'kind': 'lib'}}}
        diff = ('Removed items from the public API\nChanged items in the public API\n'
                + ''.join('-' + entry['old'] + '\n+' + entry['new'] + '\n' for entry in entries)
                + 'Added items to the public API\n')
        for status, expected in ((1, 0), (101, 2), (-9, 2), (2, 2)):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / 'release').mkdir()
                (root / 'release/public-api-policy.json').write_text(json.dumps(policy))
                outputs = [CompletedProcess([], 0, json.dumps({'packages': [package]}), ''),
                           CompletedProcess([], 0, 'head', ''),
                           CompletedProcess([], 0, diff, ''),
                           CompletedProcess([], status, self.output('log'), '')]
                with patch('validate_public_api.ROOT', root), \
                        patch('validate_public_api.CACHE', root / 'cache'), \
                        patch('validate_public_api.major_breaks', return_value=entries), \
                        patch('validate_public_api.run', side_effect=outputs), \
                        patch('sys.argv', ['validate_public_api.py', 'semver']), \
                        contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(main(), expected)

    def test_exact_manifest_authority_required(self):
        from validate_public_api import structural_diagnostics_are_enumerated
        self.assertFalse(structural_diagnostics_are_enumerated(
            'sc-observability-log', self.output('log'), [], 1))
        self.assertFalse(structural_diagnostics_are_enumerated(
            'other-crate', self.output('log'), self.entries, 1))


if __name__ == '__main__':
    unittest.main()
