"""Focused fixture coverage for compatibility registry signature validation."""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compatibility_registry import (  # noqa: E402
    BASELINE_COMMIT,
    DEPRECATED_OWNER_BASELINE,
    deprecated_owner_names,
    has_placeholder_baseline_signature,
    is_allowed_compat_reference_source,
    is_compat_source_path,
    validate_compatibility_source_boundary,
    validate_contract_signatures,
)

REPO_ROOT = Path(__file__).resolve().parents[3]


def baseline_source(relative: str) -> str | None:
    result = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "show", f"{BASELINE_COMMIT}:{relative}"],
        capture_output=True, text=True, check=False)
    return result.stdout if result.returncode == 0 else None


class BaselineSignatureTests(unittest.TestCase):
    def test_known_nominal_identity_placeholder_is_rejected(self):
        self.assertTrue(has_placeholder_baseline_signature(
            "released public nominal identity `sc_observability_types::IdentityError`"))

    def test_concrete_signature_is_accepted(self):
        self.assertFalse(has_placeholder_baseline_signature(
            "pub struct IdentityError { code: ErrorCode, message: String }"))

    def test_prefix_match_is_exact(self):
        self.assertFalse(has_placeholder_baseline_signature(
            "released public identity for sc_observability_types::IdentityError"))


class ContractSignatureTests(unittest.TestCase):
    SIGNATURE = "pub struct IdentityError(pub Box<ErrorContext>);"

    def row(self, **overrides):
        row = {"symbol": "sc_observability_types::IdentityError", "treatment": "unchanged_alias",
               "baseline_signature": self.SIGNATURE, "canonical_signature": self.SIGNATURE}
        row.update(overrides)
        return row

    def test_equal_unchanged_alias_is_accepted(self):
        validate_contract_signatures([self.row()])

    def test_unchanged_alias_with_different_signatures_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unchanged_alias baseline and canonical signatures differ"):
            validate_contract_signatures([self.row(canonical_signature="pub enum IdentityError { Process }")])

    def test_different_signatures_are_accepted_for_existing_pair(self):
        validate_contract_signatures([self.row(
            treatment="existing_pair", canonical_signature="pub enum IdentityError { Process }")])

    def test_placeholder_canonical_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder canonical contract"):
            validate_contract_signatures([self.row(
                treatment="existing_pair",
                canonical_signature="released public nominal identity `v2::IdentityError`")])

    def test_empty_canonical_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder canonical contract"):
            validate_contract_signatures([self.row(treatment="existing_pair", canonical_signature="")])

    def test_missing_canonical_signature_is_left_to_the_restoration_rule(self):
        validate_contract_signatures([self.row(treatment="restoration", canonical_signature=None)])

    def test_placeholder_baseline_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder baseline contract"):
            validate_contract_signatures([self.row(
                baseline_signature="released public nominal identity `IdentityError`")])


class CompatibilitySourcePathTests(unittest.TestCase):
    def test_compat_file_is_accepted(self):
        self.assertTrue(is_compat_source_path(
            "crates/sc-observability-otlp/src/compat.rs"))

    def test_compat_module_descendant_is_accepted(self):
        self.assertTrue(is_compat_source_path(
            "crates/sc-observability-otlp/src/compat/mod.rs"))

    def test_compatibility_file_is_rejected(self):
        self.assertFalse(is_compat_source_path(
            "crates/sc-observability-otlp/src/compatibility.rs"))

    def test_compat_extra_file_is_rejected(self):
        self.assertFalse(is_compat_source_path(
            "crates/sc-observability-otlp/src/compat_extra.rs"))

    def test_compatibility_module_descendant_is_rejected(self):
        self.assertFalse(is_compat_source_path(
            "crates/sc-observability-otlp/src/compatibility/mod.rs"))


class CompatibilityReferenceGuardTests(unittest.TestCase):
    def test_registered_facade_root_is_accepted(self):
        exceptions = {"crates/sc-observability-log/src/lib.rs"}
        self.assertTrue(is_allowed_compat_reference_source(
            "crates/sc-observability-log/src/lib.rs", exceptions))

    def test_unregistered_root_is_rejected(self):
        exceptions = {"crates/sc-observability-log/src/lib.rs"}
        self.assertFalse(is_allowed_compat_reference_source(
            "crates/sc-observability/src/lib.rs", exceptions))

    def test_non_root_file_is_rejected_even_when_its_root_is_registered(self):
        exceptions = {"crates/sc-observability-log/src/lib.rs"}
        self.assertFalse(is_allowed_compat_reference_source(
            "crates/sc-observability-log/src/handle.rs", exceptions))


class CompatibilitySourceBoundaryTests(unittest.TestCase):
    def source_file(self, root: Path, relative: str, contents: str) -> Path:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
        return path

    def test_compat_file_and_module_descendant_are_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sources = [self.source_file(root, relative, "#[deprecated]\nuse crate::compat::Legacy;\n")
                       for relative in ("crates/example/src/compat.rs", "crates/example/src/compat/mod.rs")]
            validate_compatibility_source_boundary(root, sources, {})

    def test_canonical_source_importing_compat_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.source_file(
                root, "crates/example/src/runtime.rs", "use crate::compat::Legacy;\n")

            with self.assertRaisesRegex(ValueError, "canonical source imports compatibility module"):
                validate_compatibility_source_boundary(root, [source], {})

    def test_unrecorded_deprecated_owner_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.source_file(
                root, "crates/example/src/runtime.rs", "#[deprecated]\npub fn old() {}\n")

            with self.assertRaisesRegex(ValueError, "deprecated owner is outside compat"):
                validate_compatibility_source_boundary(root, [source], {})

    def test_recorded_root_reexport_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/lib.rs"
            source = self.source_file(root, relative, "pub use crate::compat::Legacy;\n")

            validate_compatibility_source_boundary(
                root, [source], {"compat_root_reexport_exceptions": [relative]})

    def validate(self, relative: str, contents: str, registry=None, baseline=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.source_file(root, relative, contents)
            validate_compatibility_source_boundary(
                root, [source], registry or {}, baseline or {})

    def test_recorded_deprecated_owner_is_accepted(self):
        relative = "crates/example/src/runtime.rs"
        self.validate(relative, "#[deprecated]\npub fn old() {}\n",
                      {"deprecated_owner_exceptions": [relative]}, {relative: ("old",)})

    def test_deprecated_owner_beyond_baseline_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: new_owner"):
            self.validate(relative, "#[deprecated]\npub fn old() {}\n#[deprecated]\npub fn new_owner() {}\n",
                          {"deprecated_owner_exceptions": [relative]}, {relative: ("old",)})

    def test_renamed_deprecated_owner_within_baseline_count_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: emit_canonical"):
            self.validate(relative, "#[deprecated]\npub fn emit_canonical() {}\n",
                          {"deprecated_owner_exceptions": [relative]}, {relative: ("emit", "flush")})

    def test_repeated_deprecated_owner_beyond_baseline_count_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        contents = "impl A {\n#[deprecated]\npub fn new() {}\n}\nimpl B {\n#[deprecated]\npub fn new() {}\n}\n"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: new"):
            self.validate(relative, contents,
                          {"deprecated_owner_exceptions": [relative]}, {relative: ("new",)})

    def test_new_same_line_owner_before_baseline_named_item_is_rejected(self):
        relative = "crates/example/src/lib.rs"
        contents = "#[deprecated] pub fn brand_new() {}\npub fn max_age_days() {}\n"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: brand_new"):
            self.validate(
                relative, contents, {"deprecated_owner_exceptions": [relative]}, {relative: ("max_age_days",)})

    def test_exception_without_baseline_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        with self.assertRaisesRegex(ValueError, "exception has no v1.4.1 baseline"):
            self.validate(relative, "#[deprecated]\npub fn old() {}\n",
                          {"deprecated_owner_exceptions": [relative]}, {})

    def test_suffixed_compat_path_is_not_a_compat_reference(self):
        self.validate("crates/example/src/runtime.rs", "use crate::foo_compat::Legacy;\n")

    def test_relative_compat_path_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "canonical source imports compatibility module"):
            self.validate("crates/example/src/runtime.rs", "use super::compat::Legacy;\n")

    def test_aliased_compat_module_import_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "canonical source imports compatibility module"):
            self.validate("crates/example/src/runtime.rs", "use crate::compat as legacy;\n")

    def test_compat_module_declared_outside_crate_root_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "compatibility module declared outside a crate root"):
            self.validate("crates/example/src/runtime.rs", "mod compat;\n")

    def test_compat_module_declared_at_crate_root_is_accepted(self):
        self.validate("crates/example/src/lib.rs", "mod compat;\n")

    def test_compatibility_module_declaration_is_not_compat(self):
        self.validate("crates/example/src/runtime.rs", "mod compatibility;\n")


class DeprecatedOwnerNameTests(unittest.TestCase):
    def test_multiline_attribute_and_preamble_resolve_to_the_owner(self):
        contents = (
            "#[deprecated(\n    since = \"1.4.0\",\n    note = \"use v2\"\n)]\n"
            "/// Legacy error.\n#[derive(Debug)]\npub struct IdentityError(u8);\n"
            "pub struct Config {\n    #[deprecated]\n    pub max_age_days: u32,\n}\n")
        self.assertEqual(deprecated_owner_names(contents), ["IdentityError", "max_age_days"])

    def test_same_line_owner_mid_file_is_taken_from_the_attribute_line(self):
        contents = "#[deprecated] pub fn brand_new() {}\npub fn max_age_days() {}\n"
        self.assertEqual(deprecated_owner_names(contents), ["brand_new"])

    def test_same_line_owner_at_end_of_file_is_taken_from_the_attribute_line(self):
        contents = "pub fn current() {}\n#[deprecated(note = \"use v2\")] pub struct Legacy;"
        self.assertEqual(deprecated_owner_names(contents), ["Legacy"])

    def test_recorded_baseline_matches_pinned_release_source(self):
        for relative, names in DEPRECATED_OWNER_BASELINE.items():
            with self.subTest(relative=relative):
                source = baseline_source(relative)
                if source is None:
                    if os.environ.get("CI"):
                        self.fail(f"pinned commit {BASELINE_COMMIT} is not in this CI clone: {relative}")
                    self.skipTest(f"pinned commit {BASELINE_COMMIT} is not in this local clone")
                self.assertEqual(deprecated_owner_names(source), list(names))


if __name__ == "__main__":
    unittest.main()
