"""Regression for B.P2's inherited author metadata staging failure."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import tomllib
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location("runtime_stager", SCRIPTS / "prepare_runtime_level_staged_packages.py")
stager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stager)


class StagedAuthorsTest(unittest.TestCase):
    def test_inherited_authors_are_standalone_and_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace.package]\nauthors = ["First Author", "Second Author"]\n')
            manifest = root / "crates" / "example" / "Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text('[package]\nname = "example"\nversion.workspace = true\nauthors.workspace = true\n')
            package = tomllib.loads(stager.normalized_manifest(manifest, "1.4.0").decode())["package"]
            self.assertEqual(package["authors"], ["First Author", "Second Author"])
            self.assertEqual(package["version"], "1.4.0")


if __name__ == "__main__":
    unittest.main()
