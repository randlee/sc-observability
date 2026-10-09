"""ADR-022 public API parity: inventory, comparator and real target-conditioned extraction tests.

The extraction tests document a fixture crate for a Windows and a Linux target
with the pinned nightly rustdoc and the pinned renderer. They skip only when the
toolchain is absent locally; under CI a missing toolchain is a failure.
"""
import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import public_api_parity as parity  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / 'scripts/ci/fixtures/public-api-parity/conditioned'
# Hard bounds so a held cargo lock or a rustup download fails the test, not hangs it.
PROBE_TIMEOUT_SECONDS = 120
COMMAND_TIMEOUT_SECONDS = 900
FIXTURE_TARGETS = ['x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc']
MUTATIONS = {
    'mutate-method': 'windows_only_method',
    'mutate-variant': 'AccessDenied',
    'mutate-field': 'windows_only_field',
    'mutate-signature': 'open(',
    'mutate-bound': 'Sync',
    'mutate-blanket': 'PlatformExtension',
    'mutate-auto-trait': 'impl core::marker::Send for parity_conditioned_fixture::Handle',
    'mutate-hidden': '__support_windows_only',
    'mutate-reexport': 'WindowsAlias',
}

OTLP_LIKE_FEATURES = {
    'default': [],
    'feature-a': ['feature-b', 'dep:serde-saphyr', 'dep:uuid'],
    'feature-b': ['dep:reqwest', 'dep:tokio'],
    'feature-c': ['dep:opentelemetry', 'dep:tokio', 'dep:tonic'],
    'feature-d': ['feature-c'],
}


def make_cell(package='demo', selection_id='none', target='x86_64-unknown-linux-gnu', rows=None, **overrides):
    cell = {
        'schema': 1, 'package': package, 'lib_name': package.replace('-', '_'), 'crate_types': ['lib'],
        'target': target, 'selection': {'id': selection_id, 'resolved': [] if selection_id == 'none' else selection_id.split('+'),
                                        'flags': [] if selection_id == 'none' else ['--no-default-features', '--features', selection_id.replace('+', ',')]},
        'source_commit': 'a' * 40, 'toolchain': 'nightly-2026-06-29', 'rustdoc_args': list(parity.RUSTDOC_ARGS),
        'renderer': {'public_api': '0.52.2', 'rustdoc_types': '0.59.0', 'format_version': 59,
                     'impls': {'blanket': True, 'auto_trait': True, 'auto_derived': True}},
        'status': 'ok', 'rows': ['pub fn demo::one()', 'pub struct demo::Two'] if rows is None else rows,
        'row_count': 2 if rows is None else len(rows), 'unresolved_item_ids': [], 'external_item_ids': 0, 'log': '',
    }
    cell.update(overrides)
    return cell


def expected_for(packages, targets=('x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc')):
    return {'schema': 1, 'targets': list(targets), 'packages': {
        name: {'cargo_toml': f'crates/{name}/Cargo.toml', 'version': '1.5.0', 'lib_name': name.replace('-', '_'),
               'crate_types': ['lib'], 'selections': [
                   {'id': 'none', 'resolved': [], 'flags': []}] + [
                   {'id': sel, 'resolved': sel.split('+'), 'flags': ['--no-default-features', '--features', sel.replace('+', ',')]}
                   for sel in extra]}
        for name, extra in packages.items()}}


def full_cells(expected):
    return [make_cell(name, selection['id'], target)
            for name, package in expected['packages'].items()
            for selection in package['selections']
            for target in expected['targets']]


class FeatureSelectionTests(unittest.TestCase):
    def test_closure_resolves_crate_local_features_only(self):
        self.assertEqual(parity.feature_closure(OTLP_LIKE_FEATURES, ['feature-a']),
                         frozenset({'feature-a', 'feature-b'}))
        self.assertEqual(parity.feature_closure(OTLP_LIKE_FEATURES, ['feature-d']),
                         frozenset({'feature-d', 'feature-c'}))
        self.assertEqual(parity.feature_closure(OTLP_LIKE_FEATURES, []), frozenset())

    def test_dependency_feature_activates_same_named_crate_feature(self):
        features = {'default': ['tauri'], 'tauri': ['dep:tauri'], 'test': ['tauri/test'], 'weak': ['other?/x']}
        self.assertEqual(parity.feature_closure(features, ['test']), frozenset({'test', 'tauri'}))
        self.assertEqual(parity.feature_closure(features, ['weak']), frozenset({'weak'}))
        self.assertEqual(len(parity.feature_selections(features)), 10)

    def test_unknown_feature_reference_fails(self):
        with self.assertRaisesRegex(parity.ParityError, 'unknown feature'):
            parity.feature_closure({'a': ['missing']}, ['a'])

    def test_selections_dedupe_by_resolved_set_and_keep_default_invocation(self):
        selections = parity.feature_selections(OTLP_LIKE_FEATURES)
        self.assertEqual(len(selections), 18)
        ids = [item['id'] for item in selections]
        self.assertEqual(len(ids), len(set(ids)))
        default = next(item for item in selections if item['flags'] == [])
        self.assertEqual(default['resolved'], ['default'])
        self.assertEqual(default['id'], 'default')
        bare = next(item for item in selections if item['id'] == 'none')
        self.assertEqual(bare['flags'], ['--no-default-features'])
        feature_pair = next(item for item in selections if item['id'] == 'feature-a+feature-b')
        self.assertEqual(feature_pair['flags'], ['--no-default-features', '--features', 'feature-a'])
        self.assertNotIn(['--no-default-features', '--features', 'default'], [item['flags'] for item in selections])

    def test_crate_without_default_feature_has_single_bare_default(self):
        selections = parity.feature_selections({'test-double': ['dep:uuid']})
        self.assertEqual([item['id'] for item in selections], ['none', 'test-double'])
        self.assertEqual(selections[0]['flags'], [])

    def test_feature_free_crate_has_one_selection(self):
        self.assertEqual(parity.feature_selections({}), [{'id': 'none', 'resolved': [], 'flags': []}])


class ReleaseInventoryTests(unittest.TestCase):
    manifest = parity.load_manifest()

    def test_release_targets_union_six_triples(self):
        self.assertEqual(parity.release_targets(self.manifest), [
            'aarch64-apple-darwin', 'aarch64-pc-windows-msvc', 'aarch64-unknown-linux-gnu',
            'x86_64-apple-darwin', 'x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu',
        ])

    def test_published_packages_are_the_eleven_release_crates(self):
        names = [item['package'] for item in parity.published_packages(self.manifest)]
        self.assertEqual(len(names), 11)
        self.assertIn('sc-otel-cli', names)
        self.assertIn('sc-observability-tauri', names)
        self.assertIn('sc-observability-py', names)
        self.assertEqual(next(item for item in parity.published_packages(self.manifest)
                              if item['package'] == 'sc-observability-tauri')['cargo_toml'], 'bindings/tauri/Cargo.toml')

    def test_unpublished_crates_are_excluded(self):
        manifest = {'crates': [{'package': 'a', 'cargo_toml': 'a/Cargo.toml', 'publish': True},
                               {'package': 'b', 'cargo_toml': 'b/Cargo.toml', 'publish': False}]}
        self.assertEqual([item['package'] for item in parity.published_packages(manifest)], ['a'])
        with self.assertRaisesRegex(parity.ParityError, 'twice'):
            parity.published_packages({'crates': [{'package': 'a', 'cargo_toml': 'x', 'publish': True}] * 2})

    def test_platform_ids_map_to_wheel_targets(self):
        self.assertEqual(parity.platform_target(self.manifest, 'windows-arm64'), 'aarch64-pc-windows-msvc')
        self.assertEqual(parity.platform_target(self.manifest, 'macos-x86_64'), 'x86_64-apple-darwin')
        with self.assertRaisesRegex(parity.ParityError, 'platform id'):
            parity.platform_target(self.manifest, 'freebsd')

    def test_missing_targets_fail(self):
        with self.assertRaisesRegex(parity.ParityError, 'no release targets'):
            parity.release_targets({})

    def test_pinned_toolchain_is_an_exact_nightly(self):
        self.assertRegex(parity.pinned_toolchain(), r'^nightly-\d{4}-\d{2}-\d{2}$')
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'public-api-toolchain'
            path.write_text('stable\n')
            with self.assertRaisesRegex(parity.ParityError, 'exact nightly'):
                parity.pinned_toolchain(path)


class ComparatorTests(unittest.TestCase):
    def setUp(self):
        self.expected = expected_for({'demo': ['extra'], 'other': []})
        self.cells = full_cells(self.expected)

    def assert_fails(self, cells, pattern, expected=None):
        with self.assertRaisesRegex(parity.ParityError, pattern):
            parity.assert_public_api_equal(cells, expected or self.expected)

    def test_identical_surfaces_pass_and_report_counts(self):
        report = parity.assert_public_api_equal(self.cells, self.expected)
        self.assertEqual(report['status'], 'equal')
        self.assertEqual(report['cells'], 6)
        self.assertEqual(report['comparisons'], 3)
        self.assertEqual(report['reference_target'], 'x86_64-unknown-linux-gnu')

    def test_row_order_and_log_text_do_not_matter(self):
        self.cells[1]['rows'] = list(reversed(self.cells[1]['rows']))
        self.cells[1]['log'] = '/Users/someone/private/path'
        self.cells[1]['host'] = 'Windows-10'
        parity.assert_public_api_equal(self.cells, self.expected)

    def test_target_only_row_fails_naming_the_row(self):
        self.cells[1]['rows'].append('pub fn demo::windows_only()')
        self.cells[1]['row_count'] = 3
        self.assert_fails(self.cells, r'\+ pub fn demo::windows_only\(\)')

    def test_removed_row_fails(self):
        self.cells[1]['rows'] = self.cells[1]['rows'][:1]
        self.assert_fails(self.cells, r'- pub struct demo::Two')

    def test_duplicate_row_multiplicity_fails(self):
        self.cells[1]['rows'].append(self.cells[1]['rows'][0])
        self.assert_fails(self.cells, r'\+ pub fn demo::one\(\)')

    def test_missing_target_fails_even_when_present_targets_match(self):
        self.assert_fails(self.cells[:-1], r'missing cell for other \[none\] x86_64-pc-windows-msvc')

    def test_missing_selection_fails(self):
        cells = [cell for cell in self.cells if cell['selection']['id'] != 'extra']
        self.assert_fails(cells, r'missing cell for demo \[extra\]')

    def test_duplicate_cell_fails(self):
        self.assert_fails(self.cells + [copy.deepcopy(self.cells[0])], 'duplicate cell')

    def test_unexpected_cell_fails(self):
        self.assert_fails(self.cells + [make_cell('stranger')], 'unexpected cell for stranger')

    def test_source_commit_mismatch_fails(self):
        self.cells[2]['source_commit'] = 'b' * 40
        self.assert_fails(self.cells, 'disagree on source_commit')
        with self.assertRaisesRegex(parity.ParityError, 'expected ' + 'c' * 40):
            parity.assert_public_api_equal(full_cells(self.expected), self.expected, commit='c' * 40)

    def test_toolchain_and_renderer_mismatch_fail(self):
        self.cells[0]['toolchain'] = 'nightly-2026-01-01'
        self.assert_fails(self.cells, 'disagree on toolchain')
        cells = full_cells(self.expected)
        cells[0]['renderer'] = {**cells[0]['renderer'], 'format_version': 61}
        self.assert_fails(cells, 'disagree on renderer')
        cells = full_cells(self.expected)
        cells[0]['rustdoc_args'] = ['-Z', 'unstable-options', '--output-format', 'json']
        self.assert_fails(cells, 'disagree on rustdoc_args')

    def test_selection_flags_mismatch_fails(self):
        self.cells[0]['selection'] = {**self.cells[0]['selection'], 'flags': ['--all-features']}
        self.assert_fails(self.cells, 'selection .* differs from expected')

    def test_failed_empty_or_incomplete_extraction_fails(self):
        for status in ('extraction-error', 'render-error', 'missing-output', 'incomplete-extraction'):
            with self.subTest(status=status):
                cells = full_cells(self.expected)
                cells[3]['status'] = status
                cells[3]['log'] = 'rustdoc exploded'
                self.assert_fails(cells, f'status {status}: rustdoc exploded')
        cells = full_cells(self.expected)
        cells[3]['rows'] = []
        self.assert_fails(cells, 'no public API rows')
        cells = full_cells(self.expected)
        cells[3]['unresolved_item_ids'] = [7]
        self.assert_fails(cells, r'unresolved rustdoc item ids \[7\]')

    def test_difference_in_one_selection_is_not_masked_by_others(self):
        extra = next(cell for cell in self.cells if cell['selection']['id'] == 'extra'
                     and cell['target'] == 'x86_64-pc-windows-msvc')
        extra['rows'] = extra['rows'] + ['pub fn demo::feature_windows_only()']
        self.assert_fails(self.cells, r'demo \[extra\]: public API differs')

    def test_load_cells_rejects_malformed_and_foreign_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            with self.assertRaisesRegex(parity.ParityError, 'no public API cells'):
                parity.load_cells(evidence)
            cell_dir = evidence / 'b4a-public-api-linux' / parity.EVIDENCE_DIRNAME / 'x86_64-unknown-linux-gnu' / 'demo'
            cell_dir.mkdir(parents=True)
            (cell_dir / 'none.json').write_text('{"schema": 1, "package": ')
            with self.assertRaisesRegex(parity.ParityError, 'malformed cell'):
                parity.load_cells(evidence)
            (cell_dir / 'none.json').write_text(json.dumps({'schema': 1, 'package': 'demo'}))
            with self.assertRaisesRegex(parity.ParityError, 'lacks'):
                parity.load_cells(evidence)
            cell = make_cell()
            cell['rows_sha256'] = parity.rows_digest(cell['rows'])
            (cell_dir / 'none.json').write_text(json.dumps(cell))
            (cell_dir / 'producer.json').write_text('{}')
            (evidence / 'b4a-wheel-linux-x86_64' / 'build-result.json').parent.mkdir(parents=True)
            (evidence / 'b4a-wheel-linux-x86_64' / 'build-result.json').write_text('{"unrelated": true}')
            cells = parity.load_cells(evidence)
            self.assertEqual(len(cells), 1)
            self.assertEqual(cells[0]['package'], 'demo')

    def test_identically_truncated_surfaces_fail_integrity(self):
        for cell in self.cells:
            cell['rows_sha256'] = parity.rows_digest(cell['rows'])
            cell['rows'] = cell['rows'][:1]
        self.assert_fails(self.cells, 'row count or digest differs')

    def test_stale_toolchain_and_flags_fail_even_when_all_cells_agree(self):
        for cell in self.cells:
            cell['toolchain'] = 'nightly-2020-01-01'
            cell['rustdoc_args'] = ['--output-format', 'json']
        self.assert_fails(self.cells, 'pinned toolchain')
        self.assert_fails(self.cells, 'required rustdoc flags')

    def test_crate_type_mismatch_fails(self):
        self.cells[0]['crate_types'] = ['proc-macro']
        self.assert_fails(self.cells, 'crate types differ')

    def test_rows_digest_preserves_multiplicity_but_ignores_order(self):
        self.assertEqual(parity.rows_digest(['b', 'a']), parity.rows_digest(['a', 'b']))
        self.assertNotEqual(parity.rows_digest(['a']), parity.rows_digest(['a', 'a']))

    def test_row_differences_report_multiplicity(self):
        self.assertEqual(parity.row_differences(['a', 'a', 'b'], ['a', 'c']), ['- a', '- b', '+ c'])
        self.assertEqual(parity.row_differences(['a'], ['a', 'a', 'a']), ['+2x a'])
        self.assertEqual(parity.row_differences(['b', 'a'], ['a', 'b']), [])


def probe(command):
    """Runs a toolchain probe; a timeout becomes a result naming the command and its output."""
    try:
        return subprocess.run(command, capture_output=True, text=True, timeout=PROBE_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        return subprocess.CompletedProcess(command, -1, '', parity.timeout_message(error))


def toolchain_unavailable():
    """Reason the real extraction cannot run here, or None when it can."""
    try:
        toolchain = parity.pinned_toolchain()
    except (OSError, parity.ParityError) as error:
        return str(error)
    if shutil.which('cargo') is None or shutil.which('rustup') is None:
        return 'cargo/rustup not installed'
    rustdoc = probe(['rustup', 'run', toolchain, 'rustdoc', '--version'])
    if rustdoc.returncode != 0:
        return f'{toolchain} not installed: {rustdoc.stderr.strip()}'
    installed = probe(['rustup', 'target', 'list', '--installed', '--toolchain', toolchain])
    if installed.returncode != 0:
        return f'{toolchain} target list failed: {installed.stderr.strip()}'
    missing = sorted(set(FIXTURE_TARGETS) - set(installed.stdout.split()))
    if missing:
        return f'{toolchain} lacks target std for {missing}'
    return None


class CommandTimeoutTests(unittest.TestCase):
    """Every bounded subprocess fails with its command and output instead of hanging."""

    def test_run_timeout_names_command_and_output(self):
        command = [sys.executable, '-c', 'import time; print("started", flush=True); time.sleep(60)']
        with mock.patch.object(parity, 'COMMAND_TIMEOUT_SECONDS', 1):
            with self.assertRaises(parity.ParityError) as caught:
                parity.run(command, cwd=ROOT)
        message = str(caught.exception)
        self.assertIn('time.sleep(60)', message)
        self.assertIn('timed out after 1s', message)

    def test_probe_timeout_is_a_failed_result_naming_the_command(self):
        command = [sys.executable, '-c', 'import time; time.sleep(60)']
        with mock.patch.object(sys.modules[__name__], 'PROBE_TIMEOUT_SECONDS', 1):
            result = probe(command)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('time.sleep(60)', result.stderr)


class RealExtractionTests(unittest.TestCase):
    """Real rustdoc + renderer runs over the target-conditioned fixture crate."""

    @classmethod
    def setUpClass(cls):
        reason = toolchain_unavailable()
        if reason:
            if os.environ.get('CI'):
                raise AssertionError(f'CI must provide the parity toolchain: {reason}')
            raise unittest.SkipTest(reason)
        # Bound every runner command from the first one, class-level setup included, and
        # record the bound each command saw so a test can prove none ran unbounded.
        cls.setup_bounds = []
        real_run = parity.run

        def recording_run(*args, **kwargs):
            cls.setup_bounds.append(parity.COMMAND_TIMEOUT_SECONDS)
            return real_run(*args, **kwargs)

        bound = mock.patch.object(parity, 'COMMAND_TIMEOUT_SECONDS', COMMAND_TIMEOUT_SECONDS)
        bound.start()
        cls.addClassCleanup(bound.stop)
        recording = mock.patch.object(parity, 'run', recording_run)
        recording.start()
        try:
            cls.toolchain = parity.pinned_toolchain()
            # A private copy of the renderer crate, so its build uses its own target dir
            # instead of the shared checkout one (and its build lock).
            renderer_dir = Path(tempfile.mkdtemp(prefix='parity-renderer-')) / 'surface-renderer'
            cls.addClassCleanup(shutil.rmtree, renderer_dir.parent, ignore_errors=True)
            shutil.copytree(parity.RENDERER_DIR, renderer_dir, ignore=shutil.ignore_patterns('target'))
            cls.renderer = parity.ensure_renderer(renderer_dir)
            cls.commit = parity.source_commit()
            cls.library = parity.package_library(FIXTURE / 'Cargo.toml')
            cls.selections = {item['id']: item for item in parity.feature_selections(cls.library['features'])}
            cls.env = parity.extraction_environment()
        finally:
            recording.stop()

    def setUp(self):
        # A target dir per test, so no other cargo process or earlier run holds its build lock.
        self.target_dir = Path(tempfile.mkdtemp(prefix='parity-target-'))
        self.addCleanup(shutil.rmtree, self.target_dir, ignore_errors=True)

    def test_class_setup_commands_were_all_bounded(self):
        self.assertGreaterEqual(len(self.setup_bounds), 3)
        self.assertEqual(set(self.setup_bounds), {COMMAND_TIMEOUT_SECONDS})
        self.assertFalse(self.renderer.is_relative_to(parity.RENDERER_DIR))

    def extract(self, target, selection):
        return parity.extract_surface(
            package=self.library['name'], cargo_toml=FIXTURE / 'Cargo.toml', lib_name=self.library['lib_name'],
            crate_types=self.library['crate_types'], target=target, selection=selection, target_dir=self.target_dir,
            toolchain=self.toolchain, renderer=self.renderer, commit=self.commit, env=self.env)

    def inventory(self, selection_ids):
        return {'schema': 1, 'targets': FIXTURE_TARGETS, 'packages': {self.library['name']: {
            'cargo_toml': str(FIXTURE.relative_to(ROOT) / 'Cargo.toml'), 'version': self.library['version'],
            'lib_name': self.library['lib_name'], 'crate_types': self.library['crate_types'],
            'selections': [self.selections[item] for item in selection_ids]}}}

    def compare(self, selection_ids):
        cells = [self.extract(target, self.selections[item]) for item in selection_ids for target in FIXTURE_TARGETS]
        for cell in cells:
            self.assertEqual(cell['status'], 'ok', cell['log'])
        return cells, parity.assert_public_api_equal(cells, self.inventory(selection_ids), commit=self.commit)

    def test_fixture_declares_every_mutation_and_feature(self):
        self.assertEqual(set(self.library['features']), set(MUTATIONS) | {'transport', 'transport-extra'})
        self.assertEqual(self.library['lib_name'], 'parity_conditioned_fixture')

    def test_private_platform_differences_pass(self):
        cells, report = self.compare(['none'])
        self.assertEqual(report['status'], 'equal')
        rows = cells[0]['rows']
        self.assertIn('pub fn parity_conditioned_fixture::__support_everywhere()', rows)
        self.assertIn('impl core::marker::Send for parity_conditioned_fixture::Handle', rows)
        self.assertIn('impl core::marker::Sync for parity_conditioned_fixture::Handle', rows)
        self.assertIn('pub fn parity_conditioned_fixture::Handle::open(&str) -> Self', rows)
        self.assertNotIn('windows_only_method', '\n'.join(rows))
        self.assertTrue(any('#[non_exhaustive] pub enum parity_conditioned_fixture::OpenError' in row for row in rows))
        self.assertFalse(any('helper' in row for row in rows))
        # Blanket trait availability on exported types is part of their public surface.
        blanket = [row for row in rows if 'core::convert::From<T>' in row or 'core::any::Any' in row
                   or 'core::borrow::Borrow<T>' in row]
        self.assertTrue(blanket)

    def test_renderer_policy_mismatch_fails(self):
        cells = [self.extract(target, self.selections['none']) for target in FIXTURE_TARGETS]
        cells[1]['renderer'] = {**cells[1]['renderer'], 'impls': {**cells[1]['renderer']['impls'], 'blanket': False}}
        with self.assertRaisesRegex(parity.ParityError, 'disagree on renderer'):
            parity.assert_public_api_equal(cells, self.inventory(['none']), commit=self.commit)

    def test_each_windows_only_public_mutation_fails(self):
        for feature, marker in MUTATIONS.items():
            with self.subTest(feature=feature):
                cells = [self.extract(target, self.selections[feature]) for target in FIXTURE_TARGETS]
                for cell in cells:
                    self.assertEqual(cell['status'], 'ok', cell['log'])
                linux, windows = cells
                self.assertEqual(linux['target'], 'x86_64-unknown-linux-gnu')
                with self.assertRaises(parity.ParityError) as caught:
                    parity.assert_public_api_equal(cells, self.inventory([feature]), commit=self.commit)
                message = str(caught.exception)
                self.assertIn(f'parity-conditioned-fixture [{feature}]: public API differs', message)
                self.assertIn(marker, message)
                self.assertNotEqual(sorted(linux['rows']), sorted(windows['rows']))

    def test_auto_trait_mutation_removes_send_and_sync_on_windows_only(self):
        cells = [self.extract(target, self.selections['mutate-auto-trait']) for target in FIXTURE_TARGETS]
        linux, windows = cells
        self.assertIn('impl core::marker::Send for parity_conditioned_fixture::Handle', linux['rows'])
        self.assertNotIn('impl core::marker::Send for parity_conditioned_fixture::Handle', windows['rows'])
        self.assertIn('impl !core::marker::Sync for parity_conditioned_fixture::Handle', windows['rows'])

    def test_doc_hidden_mutation_is_visible_to_the_extractor(self):
        windows = self.extract('x86_64-pc-windows-msvc', self.selections['mutate-hidden'])
        self.assertIn('pub fn parity_conditioned_fixture::__support_windows_only()', windows['rows'])

    def test_feature_only_difference_requires_per_selection_comparison(self):
        _, report = self.compare(['none'])
        self.assertEqual(report['status'], 'equal')
        _, report = self.compare(['transport+transport-extra'])
        self.assertEqual(report['status'], 'equal')
        with self.assertRaisesRegex(parity.ParityError, r'transport_windows_only'):
            self.compare(['transport'])

    def test_collect_and_compare_end_to_end(self):
        inventory = self.inventory(['none', 'transport'])
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'b4a-public-api-fixture'
            cells = parity.collect(inventory, FIXTURE_TARGETS, output, target_dir=self.target_dir,
                                   toolchain=self.toolchain, renderer=self.renderer, commit=self.commit)
            self.assertEqual([cell['status'] for cell in cells], ['ok'] * 4)
            written = sorted(path.relative_to(output).as_posix() for path in output.rglob('*.json'))
            self.assertEqual(written, [
                'public-api-parity/x86_64-pc-windows-msvc/parity-conditioned-fixture/none.json',
                'public-api-parity/x86_64-pc-windows-msvc/parity-conditioned-fixture/transport.json',
                'public-api-parity/x86_64-pc-windows-msvc/producer.json',
                'public-api-parity/x86_64-unknown-linux-gnu/parity-conditioned-fixture/none.json',
                'public-api-parity/x86_64-unknown-linux-gnu/parity-conditioned-fixture/transport.json',
                'public-api-parity/x86_64-unknown-linux-gnu/producer.json',
            ])
            loaded = parity.load_cells(Path(directory))
            with self.assertRaisesRegex(parity.ParityError, 'transport_windows_only'):
                parity.assert_public_api_equal(loaded, inventory, commit=self.commit)
            partial = [cell for cell in loaded if cell['selection']['id'] == 'none']
            report = parity.assert_public_api_equal(partial, self.inventory(['none']), commit=self.commit)
            self.assertEqual(report['status'], 'equal')
            with self.assertRaisesRegex(parity.ParityError, 'missing cell'):
                parity.assert_public_api_equal(partial, inventory, commit=self.commit)

    def test_bin_only_published_crate_is_omitted_end_to_end(self):
        manifest = {
            'release_targets': [{'target': target} for target in FIXTURE_TARGETS],
            'crates': [
                {'package': 'sc-otel-cli', 'cargo_toml': 'crates/sc-otel-cli/Cargo.toml', 'publish': True},
                {'package': self.library['name'], 'cargo_toml': str(FIXTURE.relative_to(ROOT) / 'Cargo.toml'),
                 'publish': True},
            ],
        }
        self.assertIsNone(parity.package_library(ROOT / 'crates/sc-otel-cli/Cargo.toml'))
        inventory = parity.expected_inventory(ROOT, manifest)
        self.assertEqual(list(inventory['packages']), [self.library['name']])
        inventory['packages'][self.library['name']]['selections'] = [self.selections['none']]
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'evidence'
            cells = parity.collect(inventory, FIXTURE_TARGETS, output, target_dir=self.target_dir,
                                   toolchain=self.toolchain, renderer=self.renderer, commit=self.commit)
            self.assertEqual([cell['status'] for cell in cells], ['ok'] * 2)
            written = sorted(path.relative_to(output).as_posix() for path in output.rglob('*.json'))
            self.assertFalse([path for path in written if 'sc-otel-cli' in path], written)
            report = parity.assert_public_api_equal(parity.load_cells(Path(directory)), inventory,
                                                    commit=self.commit)
            self.assertEqual(report['status'], 'equal')
            self.assertEqual(report['packages'], [self.library['name']])

    def test_renderer_rejects_other_rustdoc_formats(self):
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / 'fake.json'
            fake.write_text(json.dumps({'format_version': 61, 'index': {}, 'paths': {}, 'root': 0,
                                        'crate_version': None, 'includes_private': False, 'external_crates': {},
                                        'target': {'triple': 'x', 'target_features': []}}))
            with self.assertRaisesRegex(parity.ParityError, 'format_version 61 differs'):
                parity.render_rows(self.renderer, fake)
            fake.write_text('not json')
            with self.assertRaisesRegex(parity.ParityError, 'format_version'):
                parity.render_rows(self.renderer, fake)

    def test_failed_extraction_is_recorded_not_skipped(self):
        broken = {'id': 'broken', 'resolved': ['broken'], 'flags': ['--no-default-features', '--features', 'broken']}
        cell = self.extract('x86_64-pc-windows-msvc', broken)
        self.assertEqual(cell['status'], 'extraction-error')
        self.assertIn('broken', cell['log'])
        self.assertEqual(cell['rows'], [])
        with self.assertRaisesRegex(parity.ParityError, 'status extraction-error'):
            parity.assert_public_api_equal(
                [cell, {**cell, 'target': 'x86_64-unknown-linux-gnu'}],
                {'schema': 1, 'targets': FIXTURE_TARGETS, 'packages': {self.library['name']: {
                    **self.inventory(['none'])['packages'][self.library['name']], 'selections': [broken]}}})

    def test_renderer_identity_is_pinned_to_the_toolchain_format(self):
        cell = self.extract('x86_64-unknown-linux-gnu', self.selections['none'])
        self.assertEqual(cell['status'], 'ok', cell['log'])
        self.assertEqual(cell['renderer']['public_api'], '0.52.2')
        self.assertEqual(cell['renderer']['format_version'], 59)
        self.assertEqual(cell['renderer']['impls'], {'blanket': True, 'auto_trait': True, 'auto_derived': True})
        self.assertEqual(cell['toolchain'], self.toolchain)
        self.assertEqual(cell['unresolved_item_ids'], [])
        self.assertGreater(cell['external_item_ids'], 0)


if __name__ == '__main__':
    unittest.main()
