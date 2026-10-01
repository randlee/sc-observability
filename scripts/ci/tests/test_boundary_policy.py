import unittest
from pathlib import Path

from scripts.ci.boundary_policy import validate_first_party_dependencies


ROOT = Path(__file__).resolve().parents[3]
CORE_MANIFESTS = {
    "sc-observability-types": "crates/sc-observability-types/Cargo.toml",
    "sc-observability": "crates/sc-observability/Cargo.toml",
    "sc-observe": "crates/sc-observe/Cargo.toml",
    "sc-observability-otlp": "crates/sc-observability-otlp/Cargo.toml",
    "sc-observability-dto": "crates/sc-observability-dto/Cargo.toml",
    "sc-observability-log": "crates/sc-observability-log/Cargo.toml",
    "sc-observability-log-macros": "crates/sc-observability-log-macros/Cargo.toml",
    "sc-observability-log-consumer-check": "crates/sc-observability-log-consumer-check/Cargo.toml",
}


class BoundaryPolicyTests(unittest.TestCase):
    def test_all_core_crates_match_their_boundary_manifest(self):
        import tomllib

        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        for package, relative_path in CORE_MANIFESTS.items():
            document = tomllib.loads((ROOT / relative_path).read_text(encoding="utf-8"))
            dependencies = set()
            for table in [document, *document.get("target", {}).values()]:
                for section in ("dependencies", "build-dependencies", "dev-dependencies"):
                    for alias, declaration in table.get(section, {}).items():
                        specification = declaration if isinstance(declaration, dict) else {}
                        if specification.get("workspace"):
                            specification = workspace["workspace"]["dependencies"].get(alias, {})
                            specification = specification if isinstance(specification, dict) else {}
                        dependencies.add(specification.get("package", alias))
            with self.subTest(package=package):
                validate_first_party_dependencies(ROOT, package, dependencies)

    def test_extra_first_party_dependency_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "first-party dependency drift"):
            validate_first_party_dependencies(
                ROOT,
                "sc-observability-types",
                {"sc-observability"},
            )
