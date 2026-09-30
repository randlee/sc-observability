"""Focused fixture coverage for compatibility registry signature validation."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compatibility_registry import (  # noqa: E402
    has_placeholder_baseline_signature,
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


if __name__ == "__main__":
    unittest.main()
