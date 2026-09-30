"""Focused fixture coverage for compatibility registry signature validation."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compatibility_registry import (  # noqa: E402
    has_placeholder_baseline_signature,
    is_allowed_compat_reference_source,
    is_compat_source_path,
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


if __name__ == "__main__":
    unittest.main()
