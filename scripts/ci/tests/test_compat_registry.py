"""Focused fixture coverage for compatibility registry signature validation."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compatibility_registry import (  # noqa: E402
    has_placeholder_baseline_signature,
    is_allowed_compat_reference_source,
    is_compat_source_path,


    validate_compatibility_source_boundary,
)


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

    def test_recorded_deprecated_owner_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/runtime.rs"
            source = self.source_file(root, relative, "#[deprecated]\npub fn old() {}\n")

            validate_compatibility_source_boundary(
                root, [source], {"deprecated_owner_exceptions": [relative]})


if __name__ == "__main__":
    unittest.main()
