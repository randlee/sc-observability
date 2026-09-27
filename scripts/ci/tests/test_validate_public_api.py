"""CLI exit-code contracts for the public API validator."""
import contextlib
import io
import unittest
from unittest.mock import patch

from validate_public_api import cli


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
