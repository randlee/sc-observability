"""Mutation regressions for the existing OTLP boundary/dependency gates."""
import os
import shutil
import signal
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

from scripts.ci.tests._shell import BASH

from scripts.ci.otlp_dependencies import validate_composition_harness, validate_transport_dependencies

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = "crates/sc-observability-otlp/Cargo.toml"
CORE_BOUNDARY_MANIFEST = "boundaries/sc-observability/observability.toml"
HARNESS = "tests/sc-observability-composition/Cargo.toml"
SHELL_GATES = ("scripts/ci/validate_dependency_bans.sh", "scripts/ci/validate_repo_boundaries.sh")
# validate_dependency_bans.sh runs this module; its child run skips the shell
# integration cases so they never recurse, while the helper tests still run.
SHELL_CHILD_ENV = "SC_OBS_OTLP_SHELL_GATE_CHILD"
SHELL_TIMEOUT_SECONDS = 900


class TransportPolicyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for relative in (MANIFEST, "Cargo.toml", "Cargo.lock", "policy/otlp-transport.toml"):
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, target)

    def replace(self, relative, before, after):
        path = self.root / relative
        text = path.read_text()
        self.assertIn(before, text)
        path.write_text(text.replace(before, after))

    def rejects(self, text):
        with self.assertRaisesRegex(SystemExit, text):
            validate_transport_dependencies(self.root)

    def test_reviewed_manifest(self):
        self.assertIn("opentelemetry-otlp", validate_transport_dependencies(self.root))

    def test_sc_observe_dev_dependency_is_allowed(self):
        validate_transport_dependencies(self.root)

    def test_sc_observe_regular_dependency_is_rejected(self):
        self.replace(
            MANIFEST,
            "[dependencies]\n",
            "[dependencies]\nsc-observe.workspace = true\n",
        )
        with self.assertRaises(SystemExit) as raised:
            validate_transport_dependencies(self.root)
        self.assertEqual(
            str(raised.exception),
            "OTLP dependency sc-observe: dev-only; it must not appear in [dependencies]",
        )

    def test_sc_observe_regular_dependency_without_dev_declaration_is_rejected(self):
        self.replace(
            MANIFEST,
            "sc-observe.workspace = true\n",
            "",
        )
        self.replace(
            MANIFEST,
            "[dependencies]\n",
            "[dependencies]\nsc-observe.workspace = true\n",
        )
        with self.assertRaises(SystemExit) as raised:
            validate_transport_dependencies(self.root)
        self.assertEqual(
            str(raised.exception),
            "OTLP dependency sc-observe: dev-only; it must not appear in [dependencies]",
        )

    def test_test_collector_router_must_remain_dev_only(self):
        self.replace(
            MANIFEST,
            'tonic = { workspace = true, features = ["router"] }',
            'tonic = { workspace = true }',
        )
        self.rejects("dev-dependency tonic: effective features differ from policy")

    def test_test_collector_feature_expansion(self):
        self.replace(MANIFEST, 'features = ["router"] }', 'features = ["router", "tls-ring"] }')
        self.rejects("dev-dependency tonic: effective features differ from policy")

    def test_renamed_test_collector(self):
        self.replace(MANIFEST, 'tonic = { workspace = true, features = ["router"] }', 'tonic = { workspace = true, package = "tonic", features = ["router"] }')
        self.rejects("dev-dependency tonic: must inherit the reviewed workspace pin")

    def test_unreviewed_dev_dependency(self):
        self.replace(MANIFEST, 'sc-observe.workspace = true', 'sc-observe.workspace = true\nserde.workspace = true')
        self.rejects(r"dev-dependencies differ from policy: unexpected \['serde'\]")

    def test_target_specific_dev_dependency(self):
        self.replace(MANIFEST, '[lints]', "[target.'cfg(unix)'.dev-dependencies]\nserde.workspace = true\n\n[lints]")
        self.rejects(r"dev-dependencies differ from policy: unexpected \['serde'\]")

    def test_router_in_production_transport(self):
        self.replace(MANIFEST, 'tonic = { workspace = true, optional = true }', 'tonic = { workspace = true, optional = true, features = ["router"] }')
        self.rejects("tonic: effective dependency features differ")

    def test_tonic_rejected_in_sync_http_runtime(self):
        self.replace(MANIFEST, 'sync-http = ["dep:reqwest",', 'sync-http = ["dep:tonic", "dep:reqwest",')
        self.rejects("tonic: incorrect binding to sync-http")

    def test_nonoptional_transport(self):
        self.replace(MANIFEST, 'reqwest = { workspace = true, optional = true }', 'reqwest.workspace = true')
        self.rejects("reqwest: must be optional")

    def test_missing_binding(self):
        self.replace(MANIFEST, '"dep:opentelemetry", ', '')
        self.rejects("opentelemetry: incorrect binding")

    def test_sync_http_pulls_sdk_through_feature_alias(self):
        self.replace(MANIFEST, 'sync-http = [', 'bridge = ["otlp-sdk"]\nsync-http = ["bridge", ')
        self.rejects("incorrect binding to sync-http")

    def test_dependency_feature_implicitly_enables_sdk(self):
        self.replace(MANIFEST, 'sync-http = [', 'sync-http = ["opentelemetry/trace", ')
        self.rejects("incorrect binding to sync-http")

    def test_weak_feature_does_not_enable_dependency(self):
        self.replace(MANIFEST, 'sync-http = [', 'sync-http = ["opentelemetry?/trace", ')
        validate_transport_dependencies(self.root)

    def test_default_backend(self):
        self.replace(MANIFEST, 'default = []', 'default = ["otlp-sdk"]')
        self.rejects("must not be enabled by default")

    def test_pin_drift(self):
        self.replace("Cargo.toml", 'opentelemetry = "=0.33.0"', 'opentelemetry = "0.33"')
        self.rejects("workspace/lock pin differs")

    def test_missing_locked_pin(self):
        self.replace("Cargo.lock", 'name = "opentelemetry"\nversion = "0.33.0"', 'name = "opentelemetry"\nversion = "0.32.0"')
        self.rejects("workspace/lock pin differs")

    def test_sdk_transitive_pin_drift(self):
        self.replace("Cargo.lock", 'name = "tonic"\nversion = "0.14.6"', 'name = "tonic"\nversion = "0.14.5"')
        self.rejects("reviewed lock pin .* missing")

    def test_missing_transport_feature(self):
        self.replace("Cargo.toml", '"grpc-tonic", ', '')
        self.rejects("effective dependency features differ")

    def test_missing_runtime_feature(self):
        self.replace("Cargo.toml", 'features = ["rt-tokio"]', 'features = []')
        self.rejects("effective dependency features differ")

    def test_implicit_default_transport(self):
        self.replace("Cargo.toml", 'version = "=0.33.0", default-features = false', 'version = "=0.33.0", default-features = true')
        self.rejects("effective dependency features differ")



class CompositionHarnessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        members = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["members"]
        for relative in ["Cargo.toml", "policy/otlp-transport.toml"] + [f"{m}/Cargo.toml" for m in members]:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, target)

    def replace(self, relative, before, after):
        path = self.root / relative
        text = path.read_text()
        self.assertIn(before, text)
        path.write_text(text.replace(before, after))

    def rejects(self, text):
        with self.assertRaisesRegex(SystemExit, text):
            validate_composition_harness(self.root)

    def test_reviewed_harness(self):
        validate_composition_harness(self.root)

    def test_published_harness(self):
        self.replace(HARNESS, "publish = false", "publish = true")
        self.rejects("must set publish = false")

    def test_production_reverse_edge(self):
        self.replace(
            MANIFEST,
            "[dev-dependencies]\n",
            '[dev-dependencies]\nharness = { package = "sc-observability-composition", path = "../../tests/sc-observability-composition" }\n',
        )
        self.rejects("crates/sc-observability-otlp must not depend on the harness")

    def test_proto_moved_to_normal_dependencies(self):
        self.replace(HARNESS, "opentelemetry-proto.workspace = true\n", "")
        self.replace(HARNESS, "[dev-dependencies]", "[dependencies]\nopentelemetry-proto.workspace = true\n\n[dev-dependencies]")
        self.rejects("must not declare dependencies")

    def test_target_specific_build_dependency(self):
        self.replace(HARNESS, "[lints]", '[target.\'cfg(unix)\'.build-dependencies]\ntonic.workspace = true\n\n[lints]')
        self.rejects("must not declare build-dependencies")

    def test_unpinned_collector_dependency(self):
        self.replace(HARNESS, "tonic.workspace = true", 'tonic = "0.14"')
        self.rejects("tonic must inherit the reviewed workspace pin")

    def test_collector_feature_expansion(self):
        self.replace(HARNESS, "tonic.workspace = true", 'tonic = { workspace = true, features = ["router"] }')
        self.rejects("tonic effective features differ from policy")

    def test_collector_default_features_enabled(self):
        self.replace(HARNESS, "opentelemetry-proto.workspace = true", "opentelemetry-proto = { workspace = true, default-features = true }")
        self.replace("Cargo.toml", 'opentelemetry-proto = { version = "=0.33.0", default-features = false', 'opentelemetry-proto = { version = "=0.33.0", default-features = true')
        self.rejects("opentelemetry-proto effective features differ from policy")

    def test_workspace_feature_drift(self):
        self.replace("Cargo.toml", 'features = ["rt", "rt-multi-thread", "macros", "time"]', 'features = ["rt", "rt-multi-thread", "macros", "time", "net"]')
        self.rejects("tokio effective features differ from policy")

    def test_target_section_feature_expansion(self):
        self.replace(HARNESS, "tonic.workspace = true\n", "")
        self.replace(HARNESS, "[lints]", '[target.\'cfg(unix)\'.dev-dependencies]\ntonic = { workspace = true, features = ["router"] }\n\n[lints]')
        self.rejects("tonic effective features differ from policy")

    def test_otlp_backend_feature_removed(self):
        self.replace(HARNESS, 'features = ["otlp-sdk", "sync-http"]', 'features = ["otlp-sdk"]')
        self.rejects("sc-observability-otlp effective features differ from policy")

    def test_unreviewed_dev_dependency(self):
        self.replace(HARNESS, "tonic.workspace = true", "tonic.workspace = true\nopentelemetry.workspace = true")
        self.rejects(r"unexpected \['opentelemetry'\]")


@unittest.skipIf(os.environ.get(SHELL_CHILD_ENV) == "1", "nested run inside a shell gate")
@unittest.skipIf(shutil.which("bash") is None or shutil.which("git") is None, "bash and git are required")
class ShellGateIntegrationTests(unittest.TestCase):
    """Runs both real boundary shells against a temporary copy of the checkout."""

    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name) / "checkout"
        # Let Git create the shared-object linkage. Writing an alternates file
        # ourselves from a platform-formatted objects path worked on Unix but
        # did not give Git for Windows a usable alternate object database.
        # A shared local clone is network-free and still fails the pinned
        # baseline check if the source clone does not contain that commit.
        subprocess.run(
            ["git", "clone", "--shared", "--quiet", str(ROOT), str(cls.root)],
            check=True,
            timeout=60,
        )
        tracked = subprocess.run(
            ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True, timeout=60
        ).stdout.decode().split("\0")
        for relative in filter(None, tracked):
            source = ROOT / relative
            if source.is_file():
                target = cls.root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
        cls.manifest = (cls.root / MANIFEST).read_text()
        cls.core_boundary_manifest = (cls.root / CORE_BOUNDARY_MANIFEST).read_text()

    def setUp(self):
        self.addCleanup((self.root / MANIFEST).write_text, self.manifest)
        self.addCleanup(
            (self.root / CORE_BOUNDARY_MANIFEST).write_text,
            self.core_boundary_manifest,
        )

    def replace_manifest(self, before, after):
        self.assertIn(before, self.manifest)
        (self.root / MANIFEST).write_text(self.manifest.replace(before, after))

    def replace_core_boundary_manifest(self, before, after):
        self.assertIn(before, self.core_boundary_manifest)
        (self.root / CORE_BOUNDARY_MANIFEST).write_text(
            self.core_boundary_manifest.replace(before, after)
        )

    def run_gate(self, script):
        env = {
            **os.environ,
            SHELL_CHILD_ENV: "1",
            "CARGO_TARGET_DIR": os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target")),
        }
        process = subprocess.Popen(
            [BASH, script],
            cwd=self.root,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        try:
            stdout, stderr = process.communicate(timeout=SHELL_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            self.fail(f"{script} exceeded {SHELL_TIMEOUT_SECONDS}s")
        return process.returncode, stdout, stderr

    def rejects(self, message):
        # The shell's own gate must stop with the helper's diagnostic as its
        # final line, not a later nested unittest reporting the same text.
        for script in SHELL_GATES:
            with self.subTest(script=script):
                code, stdout, stderr = self.run_gate(script)
                self.assertEqual(code, 1, (stdout + stderr)[-4000:])
                self.assertEqual(stderr.rstrip().splitlines()[-1], message, stderr[-4000:])

    def test_reviewed_checkout_passes_both_gates(self):
        for script in SHELL_GATES:
            with self.subTest(script=script):
                code, stdout, stderr = self.run_gate(script)
                self.assertEqual(code, 0, (stdout + stderr)[-4000:])

    def test_unreviewed_dev_dependency_fails_both_gates(self):
        self.replace_manifest("sc-observe.workspace = true", "sc-observe.workspace = true\nserde_yaml.workspace = true")
        self.rejects("OTLP dev-dependencies differ from policy: unexpected ['serde_yaml'], missing []")

    def test_collector_server_feature_fails_both_gates(self):
        self.replace_manifest('features = ["router"] }', 'features = ["router", "server"] }')
        self.rejects("OTLP dev-dependency tonic: effective features differ from policy")

    def test_core_dependency_policy_is_loaded_from_its_boundary_manifest(self):
        self.replace_core_boundary_manifest(
            'allowed_dependencies = ["sc-observability-types"]',
            "allowed_dependencies = []",
        )
        self.rejects(
            "sc-observability first-party dependency drift: expected [], "
            "found ['sc-observability-types']"
        )


if __name__ == "__main__":
    unittest.main()
