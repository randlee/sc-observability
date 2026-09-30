"""Focused fixture coverage for compatibility registry signature validation."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compatibility_registry import has_placeholder_baseline_signature  # noqa: E402


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


if __name__ == "__main__":
    unittest.main()
