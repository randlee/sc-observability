"""Retained 1.x facade warning checks against isolated source mutations."""
import re
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.ci import validate_error_migration as validator

ROOT = Path(__file__).resolve().parents[3]
OTLP = Path('crates/sc-observability-otlp/src/v1.rs')
LOGGER = Path('crates/sc-observability/src/compat.rs')


class SourceContractTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        paths = [
            OTLP,
            LOGGER,
            Path('crates/sc-observability-types/src/errors.rs'),
            Path('crates/sc-observe/src/compat.rs'),
            Path('.claude/skills/sc-observability-adopting/references/migrate-error-api.md'),
        ]
        paths.extend(Path('crates/sc-observability/src') / name
                     for name in ('runtime.rs', 'builder.rs', 'sinks.rs'))
        for path in paths:
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, target)
        fixtures = Path('scripts/ci/fixtures/error-migration')
        for path in (ROOT / fixtures).rglob('*.rs'):
            target = self.root / path.relative_to(ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
        root_patch = patch.object(validator, 'ROOT', self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        fixture_patch = patch.object(validator, 'FIXTURE_ROOT', self.root / fixtures)
        fixture_patch.start()
        self.addCleanup(fixture_patch.stop)

    def test_retained_typed_helpers_and_warning_contract_pass(self):
        source = (self.root / OTLP).read_text()
        for method in ('new_typed', 'build_typed', 'flush_typed', 'shutdown_typed'):
            self.assertIn(f'pub fn {method}(', source)
        validator.check_source_contract()

    def test_each_relocated_wrapper_rejects_missing_deprecation(self):
        for key, owner, replacement in validator.METHODS + validator.RETAINED_FACADE_METHODS:
            with self.subTest(owner=owner):
                self.remove_attribute_and_reject(
                    OTLP if key.startswith('otlp_') else (
                        Path('crates/sc-observe/src/compat.rs') if key == 'observe' else LOGGER
                    ),
                    f'Use {replacement}(); see migrate-error-api.md.', owner)

    def test_missing_all_otlp_markers_names_the_marker_check(self):
        path = self.root / OTLP
        original = path.read_text()
        self.assertIn('since = "1.4.0"', original)
        try:
            mutated = original.replace('since = "1.4.0"', '')
            path.write_text(mutated)
            for name in ('otlp_config', 'otlp_runtime', 'otlp_assembly'):
                with self.subTest(source=name):
                    validator.check_b1e_marker(original, name)
                    with self.assertRaisesRegex(
                        AssertionError, f'^{name} has no B.1e deprecation marker$'
                    ):
                        validator.check_b1e_marker(mutated, name)
            with self.assertRaisesRegex(
                AssertionError, '^otlp_config has no B.1e deprecation marker$'
            ):
                validator.check_source_contract()
        finally:
            path.write_text(original)
        validator.check_source_contract()

    def test_supported_methods_reject_deprecation(self):
        path = self.root / LOGGER
        original = path.read_text()
        for method in ('pub fn build(self)', 'pub fn build_with_level_owner(',
                       'pub fn new_with_level_owner('):
            with self.subTest(method=method):
                self.assertEqual(original.count(method), 1)
                try:
                    path.write_text(original.replace(
                        method,
                        '#[deprecated(since = "1.4.0", note = "planted negative")]\n    ' + method,
                        1,
                    ))
                    with self.assertRaisesRegex(
                        AssertionError,
                        '^' + re.escape(f'supported method {method} was deprecated') + '$',
                    ) as failure:
                        validator.check_source_contract()
                    print(f'planted exemption negative: {failure.exception}')
                finally:
                    path.write_text(original)
                validator.check_source_contract()

    def test_emit_rejects_missing_original_warning(self):
        path = self.root / LOGGER
        source = path.read_text()
        match = re.search(r'#\[deprecated\(\s*since = "1\.2\.0",.*?\)\]', source, re.S)
        self.assertIsNotNone(match)
        try:
            path.write_text(source[:match.start()] + source[match.end():])
            with self.assertRaisesRegex(AssertionError, 'Logger::emit existing 1.2.0 warning changed'):
                validator.check_source_contract()
        finally:
            path.write_text(source)
        validator.check_source_contract()

    def remove_attribute_and_reject(self, relative, note, owner):
        path = self.root / relative
        source = path.read_text()
        pattern = r'#\[deprecated\(\s*since = "1\.4\.0",\s*note = "' + re.escape(note) + r'"\s*\)\]'
        match = re.search(pattern, source)
        self.assertIsNotNone(match, owner)
        try:
            path.write_text(source[:match.start()] + source[match.end():])
            with self.assertRaisesRegex(AssertionError, re.escape(owner)) as failure:
                validator.check_source_contract()
            print(f'planted negative {owner}: {failure.exception}')
        finally:
            path.write_text(source)
        validator.check_source_contract()


if __name__ == '__main__':
    unittest.main()
