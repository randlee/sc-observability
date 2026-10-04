"""Headless unit tests for the Rust-consumer integration suite runner."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


RUNNER = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("rust_consumers_run", RUNNER)
assert SPEC and SPEC.loader
run = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(run)


class RunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.output = Path(self.temporary.name)

    def evidence(self, **overrides):
        evidence = {
            "status": "passed",
            "source_commit": "a" * 40,
            "archives": {"sc-observability": "archive-sha"},
            "dependency_provenance": [{"name": "sc-observability", "source": None}],
        }
        evidence.update(overrides)
        path = self.output / "evidence.json"
        path.write_text(json.dumps(evidence))
        return path

    def test_four_named_independent_cases_are_recorded(self) -> None:
        calls = []

        def execute(command, **_kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "ok\n", "")

        with patch.object(run, "verify_source"), patch.object(run, "prepare_sandbox_prerequisites"), patch.object(run, "candidate_version", return_value="1.4.1"), patch.object(run, "checked_origin", return_value={"status": "passed"}), patch.object(run.subprocess, "run", side_effect=execute):
            self.assertEqual(0, run.run("a" * 40, self.output))
        self.assertEqual(6, len(calls))
        summary = json.loads((self.output / "summary.json").read_text())
        self.assertEqual({"core", "binding-bridge", "runtime-level", "log-bridge"}, set(summary["outcomes"]))
        self.assertTrue(all(summary["outcomes"].values()))

    def test_later_cases_run_when_earlier_case_fails(self) -> None:
        calls = []

        def execute(command, **_kwargs):
            calls.append(command)
            exit_code = 1 if len(calls) == 1 else 0
            return subprocess.CompletedProcess(command, exit_code, "", "failure\n" if exit_code else "")

        with patch.object(run, "verify_source"), patch.object(run, "prepare_sandbox_prerequisites"), patch.object(run, "candidate_version", return_value="1.4.1"), patch.object(run, "checked_origin", return_value={"status": "passed"}), patch.object(run.subprocess, "run", side_effect=execute):
            self.assertEqual(1, run.run("b" * 40, self.output))
        self.assertEqual(6, len(calls))
        summary = json.loads((self.output / "summary.json").read_text())
        self.assertFalse(summary["outcomes"]["core"])
        self.assertTrue(summary["outcomes"]["log-bridge"])

    def test_timeout_receipt_normalizes_bytes_and_marks_case_failed(self) -> None:
        command = ["cargo", "run"]
        timeout = subprocess.TimeoutExpired(command, 900, output=b"partial\xff stdout\n", stderr=None)
        evidence = self.evidence()
        with patch.object(run, "checked_origin", return_value={"status": "passed"}), patch.object(run.subprocess, "run", side_effect=timeout):
            passed, origin = run.record_case("timeout", [command], self.output, evidence, "a" * 40)

        self.assertFalse(passed)
        self.assertEqual({"status": "passed"}, origin)
        receipt = json.loads((self.output / "timeout.json").read_text())
        self.assertEqual(124, receipt["commands"][0]["exit_code"])
        log = (self.output / "timeout.log").read_text()
        self.assertIn("partial\ufffd stdout", log)
        self.assertIn("command exceeded 900 seconds", log)

    def test_case_command_vectors_include_all_consumer_proofs(self) -> None:
        calls = []

        def execute(command, **_kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "ok\n", "")

        with patch.object(run, "verify_source"), patch.object(run, "prepare_sandbox_prerequisites"), patch.object(run, "candidate_version", return_value="1.4.1"), patch.object(run, "checked_origin", return_value={"status": "passed"}), patch.object(run.subprocess, "run", side_effect=execute):
            self.assertEqual(0, run.run("e" * 40, self.output))

        self.assertEqual(6, len(calls))
        self.assertEqual("scripts/ci/build_binding_source_bundle.py", calls[0][1])
        self.assertEqual("scripts/ci/validate_binding_bundle.py", calls[1][1])
        self.assertEqual([run.sys.executable, "scripts/ci/validate_binding_runtime.py", "--consumer-only"], calls[2][:3])
        self.assertEqual("scripts/ci/build_binding_source_bundle.py", calls[3][1])
        self.assertEqual("scripts/ci/validate_binding_bundle.py", calls[4][1])
        self.assertEqual("scripts/ci/validate_log_staged_consumer.py", calls[5][1])

    def test_sandbox_prerequisites_install_pinned_toolchain(self) -> None:
        with patch.object(run.platform, "system", return_value="Darwin"), patch.object(run.subprocess, "run") as execute:
            run.prepare_sandbox_prerequisites()
        execute.assert_called_once_with(["rustup", "toolchain", "install", "1.94.1", "--profile", "minimal"], check=True)

    def test_linux_sandbox_prerequisites_require_bwrap(self) -> None:
        with patch.object(run.platform, "system", return_value="Linux"), patch.object(run.shutil, "which", return_value=None), patch.object(run.subprocess, "run"):
            with self.assertRaisesRegex(RuntimeError, "bwrap"):
                run.prepare_sandbox_prerequisites()

    def test_windows_sandbox_validators_use_existing_supervisor(self) -> None:
        calls = []

        def execute(command, **_kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "ok\n", "")

        with patch.object(run, "verify_source"), patch.object(run, "prepare_sandbox_prerequisites"), patch.object(run, "candidate_version", return_value="1.4.1"), patch.object(run, "checked_origin", return_value={"status": "passed"}), patch.object(run.platform, "system", return_value="Windows"), patch.object(run.subprocess, "run", side_effect=execute):
            self.assertEqual(0, run.run("c" * 40, self.output))

        supervised = [command for command in calls if run.WINDOWS_SUPERVISOR in command]
        self.assertEqual(3, len(supervised))
        self.assertTrue(all(command[:2] == [run.sys.executable, run.WINDOWS_SUPERVISOR] for command in supervised))
        self.assertTrue(all(command[4] == "--" for command in supervised))
        self.assertEqual(
            {"scripts/ci/validate_binding_bundle.py", "scripts/ci/validate_binding_runtime.py"},
            {command[6] for command in supervised},
        )

    def test_runtime_level_uses_origin_evidence_without_an_empty_marker(self) -> None:
        calls = []

        def execute(command, **_kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "ok\n", "")

        with patch.object(run, "verify_source"), patch.object(run, "prepare_sandbox_prerequisites"), patch.object(run, "candidate_version", return_value="1.4.1"), patch.object(run, "checked_origin", return_value={"status": "passed"}), patch.object(run.subprocess, "run", side_effect=execute):
            self.assertEqual(0, run.run("d" * 40, self.output))

        runtime_validator = next(
            command for command in calls
            if any("runtime-level-bundle" in item for item in command)
            and "scripts/ci/validate_binding_bundle.py" in command
        )
        self.assertNotIn("--expected-marker", runtime_validator)

    def test_origin_evidence_requires_passed_status(self) -> None:
        with self.assertRaisesRegex(ValueError, "status"):
            run.checked_origin(self.evidence(status="failed"), "a" * 40)

    def test_origin_evidence_requires_candidate_source(self) -> None:
        with self.assertRaisesRegex(ValueError, "source commit"):
            run.checked_origin(self.evidence(source_commit="b" * 40), "a" * 40)

    def test_origin_evidence_requires_archives(self) -> None:
        with self.assertRaisesRegex(ValueError, "archives"):
            run.checked_origin(self.evidence(archives={}), "a" * 40)

    def test_origin_evidence_rejects_outside_first_party_resolution(self) -> None:
        with self.assertRaisesRegex(ValueError, "outside artifacts"):
            run.checked_origin(
                self.evidence(
                    dependency_provenance=None,
                    dependency_resolution={"sc-observability": {"source": "registry+https://example.invalid"}},
                ),
                "a" * 40,
            )

    def test_source_mismatch_is_rejected_before_cases(self) -> None:
        with patch.object(run.subprocess, "check_output", return_value="b" * 40 + "\n"), self.assertRaisesRegex(ValueError, "does not match"):
            run.verify_source("a" * 40)

    def test_dirty_source_is_rejected_before_cases(self) -> None:
        with patch.object(
            run.subprocess,
            "check_output",
            side_effect=["a" * 40 + "\n", " M scripts/integrate/suites/rust-consumers/run.py\n"],
        ), self.assertRaisesRegex(ValueError, "uncommitted changes"):
            run.verify_source("a" * 40)


if __name__ == "__main__":
    unittest.main()
