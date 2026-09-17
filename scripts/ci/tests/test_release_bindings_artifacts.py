#!/usr/bin/env python3
"""Negative/positive tests for scripts/release_bindings_artifacts.py.

Uses synthetic fixture manifests written under a temp directory (never the
real release/bindings-artifacts.toml) to prove the malformed/missing/
duplicate/dependency-order/name/version negatives called for by B.7's
acceptance criteria without needing real registries.
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS.parent))
import release_bindings_artifacts as rba  # noqa: E402


WORKSPACE_TOML = """\
[workspace]
members = [
  "crates/dto",
  "crates/binding-runtime",
]

[workspace.package]
version = "9.9.9"
"""

DTO_CARGO_TOML = """\
[package]
name = "sc-observability-dto"
version.workspace = true
"""

RUNTIME_CARGO_TOML = """\
[package]
name = "sc-observability-binding-runtime"
version = "9.9.9"
"""

PYPROJECT_TOML = """\
[project]
name = "sc-observability"
version = "9.9.9"
"""

PACKAGE_JSON = json.dumps({"name": "@sc-observability/client", "version": "9.9.9"})


class FixtureRepo:
    """Builds a minimal synthetic repo tree under tmp for one test."""

    def __init__(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        (self.root / "Cargo.toml").write_text(WORKSPACE_TOML, encoding="utf-8")
        (self.root / "crates/dto").mkdir(parents=True)
        (self.root / "crates/dto/Cargo.toml").write_text(DTO_CARGO_TOML, encoding="utf-8")
        (self.root / "crates/binding-runtime").mkdir(parents=True)
        (self.root / "crates/binding-runtime/Cargo.toml").write_text(RUNTIME_CARGO_TOML, encoding="utf-8")
        (self.root / "pkg").mkdir(parents=True)
        (self.root / "pkg/pyproject.toml").write_text(PYPROJECT_TOML, encoding="utf-8")
        (self.root / "pkg/package.json").write_text(PACKAGE_JSON, encoding="utf-8")

    def cleanup(self) -> None:
        self._tmp.cleanup()

    def write_manifest(self, text: str) -> Path:
        path = self.root / "bindings-artifacts.toml"
        path.write_text(text, encoding="utf-8")
        return path

    @property
    def workspace_toml(self) -> Path:
        return self.root / "Cargo.toml"


VALID_MANIFEST = """\
schema_version = 1

[[crates]]
artifact = "sc-observability-dto"
package = "sc-observability-dto"
cargo_toml = "crates/dto/Cargo.toml"
workspace_member = true
publish_order = 1
depends_on = []
wait_after_publish_seconds = 30
status = "ready"

[[crates]]
artifact = "sc-observability-binding-runtime"
package = "sc-observability-binding-runtime"
cargo_toml = "crates/binding-runtime/Cargo.toml"
workspace_member = true
publish_order = 2
depends_on = ["sc-observability-dto"]
wait_after_publish_seconds = 30
status = "ready"

[[crates]]
artifact = "sc-observability-tauri"
package = "sc-observability-tauri"
cargo_toml = "bindings/tauri/Cargo.toml"
workspace_member = false
publish_order = 3
depends_on = ["sc-observability-binding-runtime"]
wait_after_publish_seconds = 30
status = "pending"
pending_reason = "not yet present in this branch's ancestry"

[[packages]]
artifact = "sc-observability-python-wheel"
kind = "pypi"
package = "sc-observability"
manifest_path = "pkg/pyproject.toml"
publish_order = 4
depends_on = ["sc-observability-binding-runtime"]
status = "ready"

[[packages]]
artifact = "sc-observability-typescript-client"
kind = "npm"
package = "@sc-observability/client"
manifest_path = "pkg/package.json"
publish_order = 5
depends_on = []
status = "pending"
pending_reason = "not yet present in this branch's ancestry"
"""


def _cd_and_run(repo: FixtureRepo, manifest_text: str, func, args_extra: dict[str, object] | None = None):
    manifest_path = repo.write_manifest(manifest_text)
    import argparse

    ns = argparse.Namespace(
        manifest=str(manifest_path),
        workspace_toml=str(repo.workspace_toml),
        require_ready=False,
    )
    if args_extra:
        for key, value in args_extra.items():
            setattr(ns, key, value)
    original_cwd = Path.cwd()
    try:
        import os

        os.chdir(repo.root)
        return func(ns)
    finally:
        os.chdir(original_cwd)


class ValidateManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.repo = FixtureRepo()

    def tearDown(self) -> None:
        self.repo.cleanup()

    def test_valid_manifest_passes(self) -> None:
        result = _cd_and_run(self.repo, VALID_MANIFEST, rba.cmd_validate_manifest)
        self.assertEqual(result, 0)

    def test_ready_entry_with_missing_file_fails(self) -> None:
        broken = VALID_MANIFEST.replace('cargo_toml = "crates/dto/Cargo.toml"', 'cargo_toml = "crates/dto/Missing.toml"')
        with self.assertRaisesRegex(SystemExit, "cargo_toml not found"):
            _cd_and_run(self.repo, broken, rba.cmd_validate_manifest)

    def test_pending_entry_missing_reason_fails(self) -> None:
        broken = VALID_MANIFEST.replace(
            'status = "pending"\npending_reason = "not yet present in this branch\'s ancestry"\n\n[[packages]]',
            'status = "pending"\n\n[[packages]]',
            1,
        )
        with self.assertRaisesRegex(SystemExit, "pending_reason"):
            _cd_and_run(self.repo, broken, rba.cmd_validate_manifest)

    def test_duplicate_artifact_across_tables_fails(self) -> None:
        broken = VALID_MANIFEST.replace(
            'artifact = "sc-observability-typescript-client"',
            'artifact = "sc-observability-dto"',
        )
        with self.assertRaisesRegex(SystemExit, "duplicate artifact"):
            _cd_and_run(self.repo, broken, rba.cmd_validate_manifest)

    def test_depends_on_undefined_artifact_fails(self) -> None:
        broken = VALID_MANIFEST.replace(
            'depends_on = ["sc-observability-dto"]',
            'depends_on = ["sc-observability-does-not-exist"]',
        )
        with self.assertRaisesRegex(SystemExit, "undefined artifact"):
            _cd_and_run(self.repo, broken, rba.cmd_validate_manifest)

    def test_publish_order_violating_dependency_fails(self) -> None:
        broken = VALID_MANIFEST.replace("publish_order = 2", "publish_order = 0", 1)
        with self.assertRaisesRegex(SystemExit, "publish_order"):
            _cd_and_run(self.repo, broken, rba.cmd_validate_manifest)

    def test_package_name_mismatch_fails(self) -> None:
        broken = VALID_MANIFEST.replace(
            'package = "sc-observability-dto"\ncargo_toml = "crates/dto/Cargo.toml"',
            'package = "totally-wrong-name"\ncargo_toml = "crates/dto/Cargo.toml"',
        )
        with self.assertRaisesRegex(SystemExit, "package name mismatch"):
            _cd_and_run(self.repo, broken, rba.cmd_validate_manifest)

    def test_duplicate_publish_order_fails(self) -> None:
        broken = VALID_MANIFEST.replace("publish_order = 2", "publish_order = 1", 1)
        with self.assertRaisesRegex(SystemExit, "duplicate publish_order"):
            _cd_and_run(self.repo, broken, rba.cmd_validate_manifest)


class VerifyVersionsTests(unittest.TestCase):
    def setUp(self) -> None:
        self.repo = FixtureRepo()

    def tearDown(self) -> None:
        self.repo.cleanup()

    def test_matching_versions_pass(self) -> None:
        result = _cd_and_run(self.repo, VALID_MANIFEST, rba.cmd_verify_versions)
        self.assertEqual(result, 0)

    def test_version_mismatch_fails(self) -> None:
        (self.repo.root / "crates/binding-runtime/Cargo.toml").write_text(
            '[package]\nname = "sc-observability-binding-runtime"\nversion = "0.0.1"\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(SystemExit, "version mismatch"):
            _cd_and_run(self.repo, VALID_MANIFEST, rba.cmd_verify_versions)

    def test_pypi_version_mismatch_fails(self) -> None:
        (self.repo.root / "pkg/pyproject.toml").write_text(
            '[project]\nname = "sc-observability"\nversion = "0.0.1"\n', encoding="utf-8"
        )
        with self.assertRaisesRegex(SystemExit, "version mismatch"):
            _cd_and_run(self.repo, VALID_MANIFEST, rba.cmd_verify_versions)


class ListPublishPlanTests(unittest.TestCase):
    def setUp(self) -> None:
        self.repo = FixtureRepo()

    def tearDown(self) -> None:
        self.repo.cleanup()

    def test_without_require_ready_includes_pending_rows(self) -> None:
        result = _cd_and_run(self.repo, VALID_MANIFEST, rba.cmd_list_publish_plan)
        self.assertEqual(result, 0)

    def test_require_ready_fails_and_lists_pending(self) -> None:
        with self.assertRaisesRegex(SystemExit, "sc-observability-tauri"):
            _cd_and_run(
                self.repo,
                VALID_MANIFEST,
                rba.cmd_list_publish_plan,
                args_extra={"require_ready": True},
            )

    def test_require_ready_passes_when_all_ready(self) -> None:
        # The two pending entries in VALID_MANIFEST point at files that do not
        # exist in the fixture repo (that is the point of "pending"), so
        # flipping their status to "ready" also requires materializing those
        # files here, matching the declared package names.
        (self.repo.root / "bindings/tauri").mkdir(parents=True)
        (self.repo.root / "bindings/tauri/Cargo.toml").write_text(
            '[package]\nname = "sc-observability-tauri"\nversion = "9.9.9"\n', encoding="utf-8"
        )
        all_ready = VALID_MANIFEST.replace(
            'status = "pending"\npending_reason = "not yet present in this branch\'s ancestry"',
            'status = "ready"',
        )
        result = _cd_and_run(
            self.repo, all_ready, rba.cmd_list_publish_plan, args_extra={"require_ready": True}
        )
        self.assertEqual(result, 0)


if __name__ == "__main__":
    unittest.main()
