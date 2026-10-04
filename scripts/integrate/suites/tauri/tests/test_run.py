"""Unit tests for the CI-only Tauri integration runner."""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


RUNNER = Path(__file__).resolve().parents[1] / "run.py"
SPEC = importlib.util.spec_from_file_location("tauri_runner", RUNNER)
assert SPEC and SPEC.loader
tauri_runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tauri_runner)


class TauriRunnerTests(unittest.TestCase):
    def test_local_execution_is_rejected_before_any_desktop_work(self):
        with patch.dict("os.environ", {}, clear=True), patch.object(tauri_runner, "prepare_platform") as prepare:
            with self.assertRaisesRegex(RuntimeError, "only in CI"):
                tauri_runner.run("a" * 40, Path(tempfile.mkdtemp()))
        prepare.assert_not_called()

    def test_selected_source_must_be_the_checked_out_commit(self):
        with patch.object(tauri_runner.subprocess, "check_output", return_value="a" * 40 + "\n"):
            tauri_runner.verify_source("a" * 40)
        with self.assertRaisesRegex(RuntimeError, "lowercase 40-hex"):
            tauri_runner.verify_source("HEAD")
        with patch.object(tauri_runner.subprocess, "check_output", return_value="a" * 40 + "\n"):
            with self.assertRaisesRegex(RuntimeError, "does not match"):
                tauri_runner.verify_source("b" * 40)

    def test_linux_uses_headless_existing_qualification_helper(self):
        self.assertEqual(
            ["xvfb-run", "-a", "bash", "scripts/ci/validate_typescript_bindings.sh", "--platform"],
            tauri_runner.qualification_command("Linux"),
        )

    def test_windows_uses_process_supervisor(self):
        self.assertEqual(
            [tauri_runner.sys.executable, "scripts/ci/supervise_windows_proof.py", "--", "bash",
             "scripts/ci/validate_typescript_bindings.sh", "--platform"],
            tauri_runner.qualification_command("Windows"),
        )

    def test_failed_qualification_retains_helper_evidence_before_reraising(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "target" / "tauri-qualification"
            evidence.mkdir(parents=True)
            (evidence / "windows-supervisor.json").write_text('{"exit": 1}\n')
            output = root / "output"
            with patch.object(tauri_runner, "command", side_effect=subprocess.CalledProcessError(1, ["proof"])):
                with self.assertRaises(subprocess.CalledProcessError):
                    tauri_runner.run_qualification("Windows", {}, evidence, output)
            self.assertEqual('{"exit": 1}\n', (output / "qualification" / "windows-supervisor.json").read_text())

    def test_artifacts_are_immutable_and_tied_to_selected_source(self):
        sha = "b" * 40
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            typescript = root / "bindings/typescript"
            typescript.mkdir(parents=True)
            archive = root / "artifacts/npm/sc-observability-1.0.0.tgz"

            def fake_command(arguments, *, cwd, env=None):
                if arguments[:2] == ["npm", "pack"]:
                    archive.parent.mkdir(parents=True, exist_ok=True)
                    archive.write_bytes(b"immutable archive")
                elif arguments[1:] == ["scripts/ci/build_binding_source_bundle.py", "--root-manifest", "bindings/tauri/Cargo.toml", "--output", str(root / "artifacts/rust-bundle")]:
                    (root / "artifacts/rust-bundle").mkdir(parents=True, exist_ok=True)

            with patch.object(tauri_runner, "ROOT", root), patch.object(tauri_runner, "command", side_effect=fake_command):
                found_archive, manifest, bundle = tauri_runner.prepare_artifacts(sha, root / "artifacts")
            producer = json.loads(manifest.read_text())
            self.assertEqual(sha, producer["source_commit"])
            self.assertEqual(found_archive.name, producer["filename"])
            self.assertEqual(hashlib.sha256(found_archive.read_bytes()).hexdigest(), producer["sha256"])
            self.assertTrue(bundle.is_dir())


if __name__ == "__main__":
    unittest.main()
