import argparse
import tempfile
import unittest
from pathlib import Path

from scripts import release_artifacts


class ReleaseManifestValidationTests(unittest.TestCase):
    def run_validate(self, root: Path, manifest: Path, workspace: Path):
        return release_artifacts.cmd_validate_manifest(
            argparse.Namespace(manifest=str(manifest), workspace_toml=str(workspace))
        )

    def test_accepts_standalone_package_and_workspace_member(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(
                "[workspace]\nmembers=['crates/member']\n[workspace.package]\nversion='1.0.0'\n"
            )
            (root / "crates/member").mkdir(parents=True)
            (root / "crates/member/Cargo.toml").write_text(
                "[package]\nname='member'\nversion='1.0.0'\n"
            )
            (root / "standalone").mkdir()
            (root / "standalone/Cargo.toml").write_text(
                "[package]\nname='standalone'\nversion='1.0.0'\n[workspace]\n"
            )
            manifest = root / "manifest.toml"
            manifest.write_text(
                "schema_version=1\n"
                "[[crates]]\nartifact='member'\npackage='member'\n"
                f"cargo_toml='{root / 'crates/member/Cargo.toml'}'\n"
                "publish_order=1\nwait_after_publish_seconds=0\n"
                "[[crates]]\nartifact='standalone'\npackage='standalone'\n"
                f"cargo_toml='{root / 'standalone/Cargo.toml'}'\n"
                "publish_order=2\nwait_after_publish_seconds=0\n"
            )
            self.assertEqual(self.run_validate(root, manifest, root / "Cargo.toml"), 0)

    def test_rejects_nonmember_and_publish_false_packages(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(
                "[workspace]\nmembers=[]\n[workspace.package]\nversion='1.0.0'\n"
            )
            (root / "private").mkdir()
            (root / "private/Cargo.toml").write_text(
                "[package]\nname='private'\nversion='1.0.0'\npublish=false\n"
            )
            manifest = root / "manifest.toml"
            manifest.write_text(
                "[[crates]]\nartifact='private'\npackage='private'\n"
                f"cargo_toml='{root / 'private/Cargo.toml'}'\n"
                "publish_order=1\nwait_after_publish_seconds=0\n"
            )
            with self.assertRaises(SystemExit):
                self.run_validate(root, manifest, root / "Cargo.toml")


if __name__ == "__main__":
    unittest.main()
