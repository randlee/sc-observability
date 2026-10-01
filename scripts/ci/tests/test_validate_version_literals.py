"""Regression coverage for shared compatible-release policy validation."""
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import validate_version_literals  # noqa: E402
from validate_public_api import validate_compatible_policy  # noqa: E402


ROOT = Path(__file__).resolve().parents[3]


class VersionLiteralPolicyTests(unittest.TestCase):
    def test_version_validation_delegates_compatible_policy(self):
        policy = json.loads((ROOT / "release/public-api-policy.json").read_text(encoding="utf-8"))
        with patch.object(validate_version_literals, "validate_compatible_policy",
                          wraps=validate_compatible_policy) as validate_policy:
            validate_version_literals.validate(ROOT)

        validate_policy.assert_called_once_with(policy, ROOT)


if __name__ == "__main__":
    unittest.main()
