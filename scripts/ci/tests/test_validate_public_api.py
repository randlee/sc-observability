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


if __name__ == '__main__':
    unittest.main()
