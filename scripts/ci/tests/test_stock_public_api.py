"""Unit tests for the stock rustdoc-JSON snapshot wrapper."""
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.ci import stock_public_api as snapshots


class StockPublicApiTests(unittest.TestCase):
    def test_release_roster_matches_publish_artifacts(self):
        packages = snapshots.published_packages()
        manifest = tomllib.loads(
            (snapshots.ROOT / "release/publish-artifacts.toml").read_text(encoding="utf-8")
        )
        expected = {
            item["package"] for item in manifest["crates"] if item.get("publish") is True
        } - {"sc-otel-cli"}
        self.assertEqual({package.name for package in packages}, expected)
        self.assertEqual(len(packages), len(expected))

    def test_published_packages_skip_binary_only_crates(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "1.0.0"\n', encoding="utf-8")
            for name, source in (("lib-crate", "lib.rs"), ("bin-crate", "main.rs")):
                (root / name / "src").mkdir(parents=True)
                (root / name / "src" / source).write_text("", encoding="utf-8")
                (root / name / "Cargo.toml").write_text(
                    f'[package]\nname = "{name}"\nversion.workspace = true\n', encoding="utf-8"
                )
            (root / "release").mkdir()
            (root / "release/publish-artifacts.toml").write_text(
                "".join(
                    f'[[crates]]\npackage = "{name}"\ncargo_toml = "{name}/Cargo.toml"\npublish = true\n'
                    for name in ("lib-crate", "bin-crate")
                ),
                encoding="utf-8",
            )
            self.assertEqual([package.name for package in snapshots.published_packages(root)], ["lib-crate"])
            (root / "bin-crate/src/lib.rs").write_text("", encoding="utf-8")
            self.assertEqual(
                [package.name for package in snapshots.published_packages(root)], ["lib-crate", "bin-crate"]
            )

    def test_check_requires_generated_json_before_invoking_stock_tool(self):
        package = snapshots.Package("demo", Path("demo/Cargo.toml"), "demo", "1.5.0")
        with tempfile.TemporaryDirectory() as directory, patch.object(snapshots, "published_packages", return_value=[package]), patch.object(snapshots, "BASELINES", Path(directory) / "baselines"):
            with self.assertRaisesRegex(snapshots.SnapshotError, "missing current rustdoc JSON"):
                snapshots.check(Path(directory) / "target")

    def test_check_compares_verbatim_stock_text_without_build_arguments(self):
        package = snapshots.Package("demo", Path("demo/Cargo.toml"), "demo", "1.5.0")
        with tempfile.TemporaryDirectory() as directory, patch.object(snapshots, "published_packages", return_value=[package]), patch.object(snapshots, "BASELINES", Path(directory) / "baselines"):
            target = Path(directory) / "target"
            current = target / "doc/demo.json"
            current.parent.mkdir(parents=True)
            current.write_text("{}")
            baseline = Path(directory) / "baselines/demo/1.5.0.txt"
            baseline.parent.mkdir(parents=True)
            baseline.write_text("pub fn demo()\n")
            seen = []
            def fake_run(command):
                seen.append(command)
                return subprocess.CompletedProcess(command, 0, "pub fn demo()\n", "")
            with patch.object(snapshots, "run", side_effect=fake_run):
                snapshots.check(target)
            self.assertEqual(seen, [snapshots.command("--rustdoc-json", str(current))])
            self.assertNotIn("--manifest-path", seen[0])

    def test_setup_then_check_rejects_changed_signature_without_mutating_baseline(self):
        package = snapshots.Package("demo", Path("demo/Cargo.toml"), "demo", "1.5.0")
        with tempfile.TemporaryDirectory() as directory, patch.object(snapshots, "published_packages", return_value=[package]), patch.object(snapshots, "BASELINES", Path(directory) / "baselines"):
            target = Path(directory) / "target"
            current = target / "doc/demo.json"
            current.parent.mkdir(parents=True)
            current.write_text("{}")
            baseline = Path(directory) / "baselines/demo/1.5.0.txt"
            baseline.parent.mkdir(parents=True)
            baseline.write_text("pub fn old(value: u8)\n")
            before = baseline.read_bytes()
            def fake_run(command):
                return subprocess.CompletedProcess(command, 0, "pub fn old(value: u16)\n", "")
            with patch.object(snapshots, "run", side_effect=fake_run):
                snapshots.setup(target)
                with self.assertRaisesRegex(snapshots.SnapshotError, "removed or changed"):
                    snapshots.check(target)
            self.assertEqual(baseline.read_bytes(), before)
            self.assertEqual((target / "native-text/demo.txt").read_text(), "pub fn old(value: u16)\n")

    def test_setup_then_check_rejects_removed_accepted_item_without_mutating_baseline(self):
        package = snapshots.Package("demo", Path("demo/Cargo.toml"), "demo", "1.5.0")
        with tempfile.TemporaryDirectory() as directory, patch.object(snapshots, "published_packages", return_value=[package]), patch.object(snapshots, "BASELINES", Path(directory) / "baselines"):
            target = Path(directory) / "target"
            current = target / "doc/demo.json"
            current.parent.mkdir(parents=True)
            current.write_text("{}")
            baseline = Path(directory) / "baselines/demo/1.5.0.txt"
            baseline.parent.mkdir(parents=True)
            baseline.write_text("pub fn accepted()\npub struct StillHere;\n")
            before = baseline.read_bytes()
            def fake_run(command):
                return subprocess.CompletedProcess(command, 0, "pub struct StillHere;\n", "")
            with patch.object(snapshots, "run", side_effect=fake_run):
                snapshots.setup(target)
                with self.assertRaisesRegex(snapshots.SnapshotError, "removed or changed"):
                    snapshots.check(target)
            self.assertEqual(baseline.read_bytes(), before)
            self.assertEqual((target / "native-text/demo.txt").read_text(), "pub struct StillHere;\n")

    def test_setup_then_check_allows_added_item_without_mutating_baseline(self):
        package = snapshots.Package("demo", Path("demo/Cargo.toml"), "demo", "1.5.0")
        with tempfile.TemporaryDirectory() as directory, patch.object(snapshots, "published_packages", return_value=[package]), patch.object(snapshots, "BASELINES", Path(directory) / "baselines"):
            target = Path(directory) / "target"
            current = target / "doc/demo.json"
            current.parent.mkdir(parents=True)
            current.write_text("{}")
            baseline = Path(directory) / "baselines/demo/1.5.0.txt"
            baseline.parent.mkdir(parents=True)
            baseline.write_text("pub fn accepted()\n")
            before = baseline.read_bytes()
            def fake_run(command):
                return subprocess.CompletedProcess(command, 0, "pub fn accepted()\npub fn added()\n", "")
            with patch.object(snapshots, "run", side_effect=fake_run):
                snapshots.setup(target)
                snapshots.check(target)
            self.assertEqual(baseline.read_bytes(), before)
            self.assertEqual((target / "native-text/demo.txt").read_text(), "pub fn accepted()\npub fn added()\n")


if __name__ == "__main__":
    unittest.main()
