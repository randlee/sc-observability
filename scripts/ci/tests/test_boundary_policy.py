import unittest
from pathlib import Path

from scripts.ci.boundary_policy import (
    discovers_home_paths,
    is_first_party_dependency,
    validate_allowed_dependents,
    validate_first_party_dependencies,
)


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
    "sc-otel-cli": "crates/sc-otel-cli/Cargo.toml",
}
SC_OTEL_CLI_EDGES = {"sc-observability-otlp"}


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

    def test_sc_otel_cli_is_first_party(self):
        self.assertTrue(is_first_party_dependency("sc-otel-cli"))

    def test_sc_otel_cli_forbidden_sc_observe_edge_is_rejected(self):
        # cli.toml forbidden_edges: { from = "sc-otel-cli", to = "sc-observe" }.
        with self.assertRaisesRegex(ValueError, r"forbidden edge.*sc-observe"):
            validate_first_party_dependencies(
                ROOT,
                "sc-otel-cli",
                SC_OTEL_CLI_EDGES | {"sc-observe"},
            )

    def test_sc_otel_cli_has_no_allowed_dependents(self):
        # cli.toml allowed_dependents = [].
        with self.assertRaisesRegex(
            ValueError, "sc-observe is not an allowed dependent of sc-otel-cli"
        ):
            validate_allowed_dependents(
                ROOT,
                "sc-observe",
                {"sc-observability", "sc-observability-types", "sc-otel-cli"},
            )

    def test_otlp_rejects_dependent_outside_allowed_dependents(self):
        # otlp.toml allowed_dependents = ["sc-observability-py", "sc-otel-cli"].
        with self.assertRaisesRegex(
            ValueError,
            "sc-observability is not an allowed dependent of sc-observability-otlp",
        ):
            validate_allowed_dependents(
                ROOT,
                "sc-observability",
                {"sc-observability-types", "sc-observability-otlp"},
            )

    def test_home_discovery_is_rejected_in_production_source(self):
        for path, text in [
            ("crates/sc-otel-cli/src/send.rs", 'std::env::var("HOME")'),
            ("crates/sc-observability/src/lib.rs", 'std::env::var_os("XDG_DATA_HOME")'),
            ("crates/sc-observability-log/src/tests.rs", '"XDG_CONFIG_HOME"'),
            ("crates/sc-observe/build.rs", "dirs::home_dir()"),
        ]:
            with self.subTest(path=path):
                self.assertTrue(discovers_home_paths(Path(path), text))

    def test_home_discovery_is_rejected_in_integration_tests(self):
        for token in (
            "dirs::home_dir",
            "dirs_next::home_dir",
            "home_dir()",
            'var("HOME")',
            'var_os("HOME")',
        ):
            with self.subTest(token=token):
                self.assertTrue(
                    discovers_home_paths(
                        Path("crates/sc-otel-cli/tests/send.rs"), token
                    )
                )

    def test_integration_test_environment_isolation_is_accepted(self):
        isolation = 'for key in ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"] { command.env(key, dir); }'
        self.assertFalse(
            discovers_home_paths(Path("crates/sc-otel-cli/tests/send.rs"), isolation)
        )
        self.assertFalse(
            discovers_home_paths(
                Path("crates/sc-otel-cli/tests/nested/send.rs"),
                'command.env("XDG_CONFIG_HOME", dir); command.env("XDG_DATA_HOME", dir);',
            )
        )
        self.assertFalse(
            discovers_home_paths(
                Path("crates/sc-otel-cli/tests"), 'command.env("XDG_CONFIG_HOME", dir);'
            )
        )
        self.assertTrue(
            discovers_home_paths(
                Path("crates/sc-otel-cli/tests"), 'std::env::var("HOME")'
            )
        )
        self.assertFalse(
            discovers_home_paths(Path("crates/sc-otel-cli/src/send.rs"), "let endpoint = 1;")
        )
