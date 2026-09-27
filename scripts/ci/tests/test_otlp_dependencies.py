"""Mutation regressions for the existing OTLP boundary/dependency gates."""
import shutil
import tempfile
import unittest
from pathlib import Path

from scripts.ci.otlp_dependencies import validate_transport_dependencies

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = "crates/sc-observability-otlp/Cargo.toml"


class TransportPolicyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for relative in (MANIFEST, "Cargo.toml", "Cargo.lock", "boundaries/sc-observability-otlp/otlp.toml"):
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


if __name__ == "__main__":
    unittest.main()
