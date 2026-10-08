"""Regression coverage for the OTLP dependency boundary."""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.ci.otlp_dependencies import validate_transport_dependencies

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = "crates/sc-observability-otlp/Cargo.toml"
CORE_BOUNDARY_MANIFEST = "boundaries/sc-observability/observability.toml"
SHELL_GATES = ("scripts/ci/validate_dependency_bans.sh", "scripts/ci/validate_repo_boundaries.sh")
SHELL_CHILD_ENV = "SC_OBS_OTLP_SHELL_GATE_CHILD"


class TransportPolicyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for relative in (MANIFEST, "Cargo.toml", "Cargo.lock", "policy/otlp-transport.toml"):
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, target)

    def replace(self, before, after):
        path = self.root / MANIFEST
        text = path.read_text(encoding="utf-8")
        self.assertIn(before, text)
        path.write_text(text.replace(before, after), encoding="utf-8")

    def rejects(self, message):
        with self.assertRaisesRegex(SystemExit, message):
            validate_transport_dependencies(self.root)

    def test_reviewed_transport_policy_passes(self):
        self.assertIn("opentelemetry-otlp", validate_transport_dependencies(self.root))

    def test_sc_observe_regular_dependency_is_rejected(self):
        self.replace(
            """[dependencies]
serde_json""",
            """[dependencies]
sc-observe.workspace = true
sc-lint-attributes""",
        )
        self.rejects("sc-observe: dev-only")

    def test_renamed_sc_observe_regular_dependency_is_rejected(self):
        self.replace(
            """[dependencies]
serde_json""",
            """[dependencies]
observe_alias = { package = "sc-observe", workspace = true }
sc-lint-attributes""",
        )
        self.rejects(r"sc-observe: dev-only; it must not appear in \[dependencies\]")

    def test_extra_feature_is_rejected_in_production_transport(self):
        self.replace(
            'otel-reqwest = { workspace = true, optional = true }',
            'otel-reqwest = { workspace = true, optional = true, features = ["json"] }',
        )
        self.rejects("otel-reqwest: effective dependency features differ")

    def test_log_sink_cannot_enable_otlp_exporter(self):
        self.replace(
            'log-sink = ["dep:sc-observability",',
            'log-sink = ["dep:opentelemetry-otlp", "dep:sc-observability",',
        )
        self.rejects("opentelemetry-otlp: incorrect binding to log-sink")

    def test_transport_dependency_must_be_optional(self):
        self.replace('futures-executor = { workspace = true, optional = true }', 'futures-executor.workspace = true')
        self.rejects("futures-executor: must be optional")

    def test_transport_dependency_must_be_feature_bound(self):
        self.replace('"dep:opentelemetry", ', '')
        self.rejects("opentelemetry: incorrect binding")

    def test_transport_workspace_pin_must_match_policy(self):
        cargo = self.root / "Cargo.toml"
        text = cargo.read_text(encoding="utf-8")
        self.assertIn('opentelemetry = "=0.33.0"', text)
        cargo.write_text(text.replace('opentelemetry = "=0.33.0"', 'opentelemetry = "0.33"'), encoding="utf-8")
        self.rejects("opentelemetry: workspace/lock pin differs")

    def test_transitive_lock_pin_is_required(self):
        lock = self.root / "Cargo.lock"
        text = lock.read_text(encoding="utf-8")
        self.assertIn('name = "prost"\nversion = "0.14.4"', text)
        lock.write_text(text.replace('name = "prost"\nversion = "0.14.4"', 'name = "prost"\nversion = "0.14.3"'), encoding="utf-8")
        self.rejects("reviewed lock pin .* missing")


@unittest.skipIf(os.environ.get(SHELL_CHILD_ENV) == "1", "nested run inside a shell gate")
@unittest.skipIf(shutil.which("bash") is None or shutil.which("git") is None, "bash and git are required")
class ShellGateIntegrationTests(unittest.TestCase):
    """Run retained dependency-boundary regressions against an isolated checkout."""

    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name)
        tracked = subprocess.run(
            ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True, timeout=60
        ).stdout.decode().split("\0")
        for relative in filter(None, tracked):
            source = ROOT / relative
            if source.is_file():
                target = cls.root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
        objects = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-path", "objects"],
            cwd=ROOT, check=True, capture_output=True, text=True, timeout=60,
        ).stdout.strip()
        subprocess.run(["git", "init", "-q", str(cls.root)], check=True, timeout=60)
        (cls.root / ".git/objects/info/alternates").write_text(objects + "\n")
        cls.core_boundary_manifest = (cls.root / CORE_BOUNDARY_MANIFEST).read_text()

    def setUp(self):
        self.addCleanup(
            (self.root / CORE_BOUNDARY_MANIFEST).write_text,
            self.core_boundary_manifest,
        )

    def run_gate(self, script):
        process = subprocess.run(
            ["bash", script],
            cwd=self.root,
            env={**os.environ, SHELL_CHILD_ENV: "1"},
            capture_output=True,
            text=True,
            timeout=900,
        )
        return process.returncode, process.stdout, process.stderr

    def test_reviewed_checkout_passes_both_gates(self):
        for script in SHELL_GATES:
            with self.subTest(script=script):
                code, stdout, stderr = self.run_gate(script)
                self.assertEqual(code, 0, (stdout + stderr)[-4000:])

    def test_core_dependency_policy_is_loaded_from_its_boundary_manifest(self):
        path = self.root / CORE_BOUNDARY_MANIFEST
        self.assertIn('allowed_dependencies = ["sc-observability-types"]', self.core_boundary_manifest)
        path.write_text(
            self.core_boundary_manifest.replace(
                'allowed_dependencies = ["sc-observability-types"]',
                "allowed_dependencies = []",
            )
        )
        for script in SHELL_GATES:
            with self.subTest(script=script):
                code, stdout, stderr = self.run_gate(script)
                self.assertEqual(code, 1, (stdout + stderr)[-4000:])
                self.assertEqual(
                    stderr.rstrip().splitlines()[-1],
                    "sc-observability first-party dependency drift: expected [], found ['sc-observability-types']",
                )


if __name__ == "__main__":
    unittest.main()
