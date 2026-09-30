"""Mutation regressions for the existing OTLP boundary/dependency gates."""
import shutil
import tempfile
import tomllib
import unittest
from pathlib import Path

from scripts.ci.otlp_dependencies import validate_composition_harness, validate_transport_dependencies

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = "crates/sc-observability-otlp/Cargo.toml"
HARNESS = "tests/sc-observability-composition/Cargo.toml"


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

    def test_tonic_rejected_in_legacy_runtime(self):
        self.replace(MANIFEST, 'legacy-http-json = ["dep:reqwest",', 'legacy-http-json = ["dep:tonic", "dep:reqwest",')
        self.rejects("tonic: incorrect binding to legacy-http-json")

    def test_nonoptional_transport(self):
        self.replace(MANIFEST, 'reqwest = { workspace = true, optional = true }', 'reqwest.workspace = true')
        self.rejects("reqwest: must be optional")

    def test_missing_binding(self):
        self.replace(MANIFEST, '"dep:opentelemetry", ', '')
        self.rejects("opentelemetry: incorrect binding")

    def test_legacy_pulls_sdk_through_feature_alias(self):
        self.replace(MANIFEST, 'legacy-http-json = [', 'bridge = ["otlp-sdk"]\nlegacy-http-json = ["bridge", ')
        self.rejects("incorrect binding to legacy-http-json")

    def test_dependency_feature_implicitly_enables_sdk(self):
        self.replace(MANIFEST, 'legacy-http-json = [', 'legacy-http-json = ["opentelemetry/trace", ')
        self.rejects("incorrect binding to legacy-http-json")

    def test_weak_feature_does_not_enable_dependency(self):
        self.replace(MANIFEST, 'legacy-http-json = [', 'legacy-http-json = ["opentelemetry?/trace", ')
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
        self.replace(HARNESS, 'features = ["otlp-sdk", "legacy-http-json"]', 'features = ["otlp-sdk"]')
        self.rejects("sc-observability-otlp effective features differ from policy")

    def test_unreviewed_dev_dependency(self):
        self.replace(HARNESS, "tonic.workspace = true", "tonic.workspace = true\nopentelemetry.workspace = true")
        self.rejects(r"unexpected \['opentelemetry'\]")


if __name__ == "__main__":
    unittest.main()
