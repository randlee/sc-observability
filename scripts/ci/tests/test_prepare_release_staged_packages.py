import io
import json
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from prepare_release_staged_packages import (  # noqa: E402
    closure,
    inspect_archive,
    ordered_closure,
    package_command,
    path_graph,
)


class ReleaseStagePlanningTests(unittest.TestCase):
    def test_cargo_workspace_members_are_package_id_strings(self):
        from prepare_release_staged_packages import root_packages

        metadata = {
            "workspace_members": ["id:leaf"],
            "packages": [{"id": "id:leaf", "name": "leaf", "manifest_path": "/tmp/leaf/Cargo.toml", "publish": None}],
        }
        roster = [{"package": "leaf", "cargo_toml": "/tmp/leaf/Cargo.toml", "publish_order": 1}]
        self.assertEqual(root_packages(Path("/"), metadata, roster), {"leaf": metadata["packages"][0]})

    def test_transitive_path_closure_is_not_limited_to_locked_labels(self):
        graph = {
            "leaf": set(),
            "middle": {"leaf"},
            "top": {"middle"},
            "independent": set(),
        }
        self.assertEqual(closure(graph, {"top"}), {"top", "middle", "leaf"})

    def test_publish_order_controls_single_multi_package_command(self):
        roster = [
            {"package": "leaf", "publish_order": 1},
            {"package": "middle", "publish_order": 2},
            {"package": "top", "publish_order": 3},
        ]
        selected = ordered_closure(roster, {"top", "leaf", "middle"})
        self.assertEqual(selected, ["leaf", "middle", "top"])
        self.assertEqual(
            package_command(selected, Path("stage/target")),
            ["cargo", "package", "--locked", "--target-dir", "stage/target",
             "-p", "leaf", "-p", "middle", "-p", "top"],
        )

    def test_path_graph_uses_package_name_for_renamed_path_dependency(self):
        packages = {
            "top": {"dependencies": [{"name": "alias", "package": "leaf", "path": "../leaf"}]},
            "leaf": {"dependencies": []},
        }
        self.assertEqual(path_graph(packages), {"top": {"leaf"}, "leaf": set()})


class ArchiveInspectionTests(unittest.TestCase):
    def test_archive_requires_clean_source_provenance(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "leaf-1.4.0.crate"
            prefix = "leaf-1.4.0/"
            files = {
                "Cargo.toml": b'[package]\nname = "leaf"\nversion = "1.4.0"\nlicense = "MIT"\n',
                "LICENSE": b"MIT",
                ".cargo_vcs_info.json": json.dumps({"git": {"sha1": "a" * 40, "dirty": False}}).encode(),
            }
            with tarfile.open(archive, "w:gz") as output:
                for relative, content in files.items():
                    member = tarfile.TarInfo(prefix + relative)
                    member.size = len(content)
                    output.addfile(member, io.BytesIO(content))
            inspected = inspect_archive(archive, "leaf", "1.4.0", "a" * 40)
            self.assertIn('name = "leaf"', inspected["normalized_manifest"])
            with self.assertRaisesRegex(ValueError, "provenance"):
                inspect_archive(archive, "leaf", "1.4.0", "b" * 40)


if __name__ == "__main__":
    unittest.main()
