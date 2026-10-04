"""Unit tests for the CI-only Tauri integration runner."""
from __future__ import annotations

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch


RUNNER = Path(__file__).resolve().parents[1] / "run.py"
SPEC = importlib.util.spec_from_file_location("tauri_runner", RUNNER)
assert SPEC and SPEC.loader
tauri_runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tauri_runner)


class TauriRunnerTests(unittest.TestCase):
    def test_command_uses_npm_cmd_on_windows(self):
        cwd = Path("/tmp")
        with patch.object(tauri_runner.os, "name", "nt"), \
                patch.object(tauri_runner.subprocess, "Popen") as popen:
            popen.return_value.wait.return_value = 0
            tauri_runner.command(["npm", "ci", "--ignore-scripts"], cwd=cwd, timeout=1, step="npm ci")
        popen.assert_called_once_with(
            ["npm.cmd", "ci", "--ignore-scripts"], cwd=cwd, env=None, start_new_session=False
        )

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
        with patch.dict(tauri_runner.os.environ, {"ProgramFiles": r"D:\Git Tools"}, clear=True):
            self.assertEqual(
                [tauri_runner.sys.executable, "scripts/ci/supervise_windows_proof.py", "--",
                 r"D:\Git Tools\Git\bin\bash.exe", "scripts/ci/validate_typescript_bindings.sh", "--platform"],
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

    def test_retention_copy_error_does_not_mask_the_qualification_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "target" / "tauri-qualification"
            evidence.mkdir(parents=True)
            output = root / "output"
            stderr = io.StringIO()
            primary = subprocess.CalledProcessError(17, ["proof"])
            with patch.object(tauri_runner, "command", side_effect=primary), \
                    patch.object(tauri_runner.shutil, "copytree", side_effect=OSError("locked")), \
                    patch.object(tauri_runner.sys, "stderr", stderr):
                with self.assertRaises(subprocess.CalledProcessError) as raised:
                    tauri_runner.run_qualification("Windows", {}, evidence, output)
            self.assertIs(primary, raised.exception)
            self.assertIn("could not retain qualification evidence: locked", stderr.getvalue())

    def test_qualification_timeout_kills_the_process_group_and_retains_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "target" / "tauri-qualification"
            evidence.mkdir(parents=True)
            (evidence / "viewer.log").write_text("timed out\n")
            output = root / "output"
            process = Mock(pid=42)
            process.wait.side_effect = [subprocess.TimeoutExpired(["proof"], 1), 0]
            with patch.object(tauri_runner.subprocess, "Popen", return_value=process), \
                    patch.object(tauri_runner.os, "name", "posix"), \
                    patch.object(tauri_runner.os, "killpg") as killpg:
                with self.assertRaisesRegex(RuntimeError, "Tauri qualification timed out after 1800s"):
                    tauri_runner.run_qualification("Linux", {}, evidence, output)
            killpg.assert_called_once_with(42, tauri_runner.signal.SIGKILL)
            self.assertEqual("timed out\n", (output / "qualification" / "viewer.log").read_text())

    def test_artifacts_are_immutable_and_tied_to_selected_source(self):
        sha = "b" * 40
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            typescript = root / "bindings/typescript"
            typescript.mkdir(parents=True)
            archive = root / "artifacts/npm/sc-observability-1.0.0.tgz"

            def fake_command(arguments, *, cwd, timeout, step, env=None):
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
