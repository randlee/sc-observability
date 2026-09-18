"""Tests for release_bindings_artifacts.py's build-evidence/verify-evidence
subcommands (C05: wrong-source, changed-byte, missing-evidence-for-ready
negative paths).

These mock the actual build steps (build_crate_artifact / build_pypi_artifacts)
so the suite runs fast and does not require a working Rust/Python toolchain.
scripts/ci/validate_binding_registry_consumers.sh exercises the *real* cargo
package / maturin toolchain end to end separately (see its "candidate
evidence" section) -- that is where the machinery is proven against real
bytes, not here.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS.parent))
import release_bindings_artifacts as rba  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
from test_release_bindings_artifacts import FixtureRepo, VALID_MANIFEST  # noqa: E402


def _fake_crate_artifact(sha: str = "a" * 64):
    def _build(entry, dest_dir):
        dest_dir = Path(dest_dir)
        artifacts_dir = dest_dir / "artifacts"
        artifacts_dir.mkdir(parents=True, exist_ok=True)
        fake_path = artifacts_dir / f"{entry['package']}-fake.crate"
        fake_path.write_bytes(b"fake-crate-bytes")
        return {
            "table": "crates",
            "build_command": "fake-cargo-package",
            "files": {
                "crate": {"artifact_path": str(fake_path), "sha256": sha, "size_bytes": 17}
            },
            "lockfile_included": False,
            "note": "fixture",
        }

    return _build


def _fake_pypi_artifacts(sha_sdist: str = "b" * 64, sha_wheel: str = "c" * 64):
    def _build(entry, dest_dir):
        return {
            "table": "packages",
            "kind": "pypi",
            "build_command": "fake-maturin",
            "files": {
                "sdist": {"artifact_path": "fake-sdist.tar.gz", "sha256": sha_sdist, "size_bytes": 1},
                "wheel": {"artifact_path": "fake-wheel.whl", "sha256": sha_wheel, "size_bytes": 1},
            },
            "note": "fixture",
        }

    return _build


def _run_in_repo(repo: FixtureRepo, func, ns_kwargs: dict[str, object]):
    ns = argparse.Namespace(**ns_kwargs)
    original_cwd = Path.cwd()
    try:
        os.chdir(repo.root)
        return func(ns)
    finally:
        os.chdir(original_cwd)


class EvidenceTestsBase(unittest.TestCase):
    def setUp(self) -> None:
        self.repo = FixtureRepo()
        self.repo.write_manifest(VALID_MANIFEST)
        subprocess.run(["git", "init", "-q"], cwd=self.repo.root, check=True)
        subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=self.repo.root, check=True)
        subprocess.run(["git", "config", "user.name", "test"], cwd=self.repo.root, check=True)
        subprocess.run(["git", "add", "-A"], cwd=self.repo.root, check=True)
        subprocess.run(["git", "commit", "-q", "-m", "init"], cwd=self.repo.root, check=True)
        self.commit = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=self.repo.root, capture_output=True, text=True, check=True
        ).stdout.strip()
        self.manifest_path = str(self.repo.root / "bindings-artifacts.toml")
        self.workspace_toml = str(self.repo.workspace_toml)
        self.out_dir = self.repo.root / "evidence"

    def tearDown(self) -> None:
        self.repo.cleanup()

    def build_evidence(self) -> None:
        with patch.object(rba, "build_crate_artifact", side_effect=_fake_crate_artifact()), \
             patch.object(rba, "build_pypi_artifacts", side_effect=_fake_pypi_artifacts()):
            result = _run_in_repo(
                self.repo,
                rba.cmd_build_evidence,
                {"manifest": self.manifest_path, "workspace_toml": self.workspace_toml, "out": str(self.out_dir)},
            )
        self.assertEqual(result, 0)

    def verify_evidence(self, evidence_path: Path | None = None):
        with patch.object(rba, "build_crate_artifact", side_effect=_fake_crate_artifact()), \
             patch.object(rba, "build_pypi_artifacts", side_effect=_fake_pypi_artifacts()):
            return _run_in_repo(
                self.repo,
                rba.cmd_verify_evidence,
                {
                    "manifest": self.manifest_path,
                    "workspace_toml": self.workspace_toml,
                    "evidence": str(evidence_path or (self.out_dir / "bindings-candidate.json")),
                },
            )


class BuildEvidenceTests(EvidenceTestsBase):
    def test_build_evidence_writes_record_for_every_ready_artifact(self) -> None:
        self.build_evidence()
        evidence_path = self.out_dir / "bindings-candidate.json"
        self.assertTrue(evidence_path.exists())
        record = json.loads(evidence_path.read_text())
        self.assertEqual(record["source_commit"], self.commit)
        self.assertEqual(record["candidate_version"], "9.9.9")
        # VALID_MANIFEST's ready entries: 2 crates + 1 pypi package.
        self.assertEqual(
            set(record["artifacts"].keys()),
            {"sc-observability-dto", "sc-observability-binding-runtime", "sc-observability-python-wheel"},
        )


class VerifyEvidenceTests(EvidenceTestsBase):
    def test_verify_passes_immediately_after_build(self) -> None:
        self.build_evidence()
        result = self.verify_evidence()
        self.assertEqual(result, 0)

    def test_wrong_source_rejected(self) -> None:
        self.build_evidence()
        evidence_path = self.out_dir / "bindings-candidate.json"
        record = json.loads(evidence_path.read_text())
        record["source_commit"] = "0" * 40
        evidence_path.write_text(json.dumps(record))
        with self.assertRaisesRegex(SystemExit, "wrong-source rejection"):
            self.verify_evidence()

    def test_changed_byte_rejected(self) -> None:
        self.build_evidence()
        with patch.object(rba, "build_crate_artifact", side_effect=_fake_crate_artifact(sha="f" * 64)), \
             patch.object(rba, "build_pypi_artifacts", side_effect=_fake_pypi_artifacts()):
            with self.assertRaisesRegex(SystemExit, "changed-byte rejection"):
                _run_in_repo(
                    self.repo,
                    rba.cmd_verify_evidence,
                    {
                        "manifest": self.manifest_path,
                        "workspace_toml": self.workspace_toml,
                        "evidence": str(self.out_dir / "bindings-candidate.json"),
                    },
                )

    def test_missing_evidence_for_ready_artifact_rejected(self) -> None:
        self.build_evidence()
        evidence_path = self.out_dir / "bindings-candidate.json"
        record = json.loads(evidence_path.read_text())
        del record["artifacts"]["sc-observability-binding-runtime"]
        evidence_path.write_text(json.dumps(record))
        with self.assertRaisesRegex(SystemExit, "missing-evidence rejection"):
            self.verify_evidence()

    def test_missing_evidence_file_rejected(self) -> None:
        with self.assertRaisesRegex(SystemExit, "evidence file not found"):
            self.verify_evidence(evidence_path=self.out_dir / "does-not-exist.json")


if __name__ == "__main__":
    unittest.main()
