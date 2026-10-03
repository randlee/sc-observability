"""Regression coverage for the B.4a workflow policy helper."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from b4a_policy import POLICY_PATH, matrix, write_policy


class B4aPolicyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_matrix_writes_six_platforms_and_twenty_nine_cells(self):
        output = self.root / "github-output"
        matrix(output)
        values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
        self.assertEqual(len(json.loads(values["platforms"])), 6)
        self.assertEqual(len(json.loads(values["cells"])), 29)

    def test_write_policy_contains_windows_arm64(self):
        output = self.root / "b4a-policy.json"
        write_policy(output)
        policy = json.loads(output.read_text(encoding="utf-8"))
        self.assertIn("windows-arm64", {platform["id"] for platform in policy["platforms"]})

    def test_duplicate_platform_id_is_rejected(self):
        policy = json.loads(POLICY_PATH.read_text(encoding="utf-8"))
        policy["platforms"].append(policy["platforms"][0].copy())
        duplicate = self.root / "duplicate-policy.json"
        duplicate.write_text(json.dumps(policy), encoding="utf-8")
        with self.assertRaises(AssertionError):
            matrix(self.root / "github-output", duplicate)


if __name__ == "__main__":
    unittest.main()
