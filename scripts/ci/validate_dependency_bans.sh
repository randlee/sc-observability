#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from pathlib import Path
import tempfile
import tomllib

root = Path(".")

def load_toml(path: Path):
    return tomllib.loads(path.read_text(encoding="utf-8"))

def section_deps(path: Path, section: str):
    data = load_toml(path)
    return set(data.get(section, {}).keys())

def target_section_deps(path: Path, section: str):
    data = load_toml(path)
    target_tables = data.get("target", {})
    return {
        target_name: set(target_data.get(section, {}).keys())
        for target_name, target_data in target_tables.items()
        if section in target_data
    }

def dependency_names(path: Path, workspace_document=None):
    data = load_toml(path)
    workspace_document = workspace_document or workspace
    names = set()
    for table in [data, *data.get("target", {}).values()]:
        for section in ("dependencies", "build-dependencies", "dev-dependencies"):
            for alias, declaration in table.get(section, {}).items():
                spec = declaration if isinstance(declaration, dict) else {}
                if spec.get("workspace"):
                    spec = workspace_document["workspace"]["dependencies"].get(alias, {})
                    spec = spec if isinstance(spec, dict) else {}
                names.add(spec.get("package", alias))
    return names

BANNED_PREFIXES = ("tauri", "pyo3", "specta")

def assert_no_banned_dependencies(path: Path, workspace_document=None):
    banned = sorted(name for name in dependency_names(path, workspace_document)
                    if name.startswith(BANNED_PREFIXES))
    if banned:
        raise SystemExit(f"forbidden boundary dependencies in {path}: {banned}")

CORE_BOUNDARY_FORBIDDEN = {"schemars", "sc-observability-dto"}

def assert_no_core_boundary_dependencies(path: Path):
    forbidden = sorted(section_deps(path, "dependencies") & CORE_BOUNDARY_FORBIDDEN)
    if forbidden:
        raise SystemExit(f"forbidden core boundary dependencies in {path}: {forbidden}")

workspace = load_toml(root / "Cargo.toml")
members = set(workspace["workspace"]["members"])
artifacts = load_toml(root / "release/publish-artifacts.toml")
required_manifests = {crate["cargo_toml"] for crate in artifacts["crates"]}
if len(required_manifests) != len(artifacts["crates"]):
    raise SystemExit("publish artifact roster contains duplicate package names")

def is_workspace_member(cargo_toml: Path) -> bool:
    try:
        relative = cargo_toml.parent.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        relative = ""
    if relative in members:
        return True
    return load_toml(cargo_toml).get("workspace") == {}

missing = sorted(path for path in required_manifests
                 if not (root / path).exists() or not is_workspace_member(root / path))
if missing:
    raise SystemExit(f"missing workspace members or standalone manifests: {missing}")

for path in root.rglob("Cargo.toml"):
    text = path.read_text(encoding="utf-8")
    if "agent-team-mail-" in text or "agent_team_mail" in text:
        raise SystemExit(f"ATM dependency reference found in {path}")

obs_runtime_deps = section_deps(root / "crates/sc-observability/Cargo.toml", "dependencies")
obs_target_runtime_deps = target_section_deps(
    root / "crates/sc-observability/Cargo.toml",
    "dependencies",
)
obs_test_deps = section_deps(root / "crates/sc-observability/Cargo.toml", "dev-dependencies")
observe_runtime_deps = section_deps(root / "crates/sc-observe/Cargo.toml", "dependencies")
observe_test_deps = section_deps(root / "crates/sc-observe/Cargo.toml", "dev-dependencies")
otlp_runtime_deps = section_deps(root / "crates/sc-observability-otlp/Cargo.toml", "dependencies")
otlp_test_deps = section_deps(root / "crates/sc-observability-otlp/Cargo.toml", "dev-dependencies")

if obs_runtime_deps != {"serde", "serde_json", "sc-observability-types", "thiserror"}:
    raise SystemExit(
        "sc-observability runtime dependency set drifted from allowed baseline: "
        f"{sorted(obs_runtime_deps)}"
    )
if obs_target_runtime_deps != {"cfg(windows)": {"windows-sys"}}:
    raise SystemExit(
        "sc-observability target-specific runtime dependency set drifted from allowed baseline: "
        f"{obs_target_runtime_deps}"
    )
if obs_test_deps - {"temp-env", "tempfile"}:
    raise SystemExit(
        "sc-observability test dependency set drifted from allowed baseline: "
        f"{sorted(obs_test_deps - {'temp-env', 'tempfile'})}"
    )

if observe_runtime_deps != {"sc-observability-types", "sc-observability"}:
    raise SystemExit(
        "sc-observe runtime dependency set drifted from allowed baseline: "
        f"{sorted(observe_runtime_deps)}"
    )

if observe_test_deps - {"serde_json"}:
    raise SystemExit(
        "sc-observe test dependency set drifted from allowed baseline: "
        f"{sorted(observe_test_deps - {'serde_json'})}"
    )

required_otlp = {
    "serde_json",
    "thiserror",
    "sc-observability-types",
}
allowed_otlp = required_otlp | {"sc-observability"}
if not required_otlp.issubset(otlp_runtime_deps) or not otlp_runtime_deps.issubset(allowed_otlp):
    raise SystemExit(
        "sc-observability-otlp runtime dependency set drifted from allowed baseline: "
        f"{sorted(otlp_runtime_deps)}"
    )

if otlp_test_deps - {"sc-observe"}:
    raise SystemExit(
        "sc-observability-otlp test dependency set drifted from allowed baseline: "
        f"{sorted(otlp_test_deps - {'sc-observe'})}"
    )

for path in [
    root / "crates/sc-observability-types/Cargo.toml",
    root / "crates/sc-observability/Cargo.toml",
    root / "crates/sc-observe/Cargo.toml",
]:
    deps = section_deps(path, "dependencies")
    if any(name.startswith("opentelemetry") or "otlp" in name for name in deps):
        raise SystemExit(f"OTLP/OpenTelemetry dependency found outside sc-observability-otlp: {path}")

dto = load_toml(root / "crates/sc-observability-dto/Cargo.toml")
if set(dto["dependencies"]) != {"sc-observability-types", "serde", "serde_json", "schemars"}:
    raise SystemExit("DTO dependency closure drifted")
if dto["dependencies"]["schemars"] != {"version": "=1.2.2", "optional": True} or dto["features"].get("schema-gen") != ["dep:schemars"]:
    raise SystemExit("DTO schema tooling must remain optional and exactly pinned")

boundary_manifests = [
    root / "crates/sc-observability-dto/Cargo.toml",
    root / "crates/sc-observability-types/Cargo.toml",
    root / "crates/sc-observability/Cargo.toml",
    root / "crates/sc-observe/Cargo.toml",
    root / "crates/sc-observability-otlp/Cargo.toml",
    root / "crates/sc-observability-log/Cargo.toml",
    root / "crates/sc-observability-log-macros/Cargo.toml",
    root / "crates/sc-observability-log-consumer-check/Cargo.toml",
]
for path in boundary_manifests:
    assert_no_banned_dependencies(path)

core_manifests = {
    "sc-observability-types": root / "crates/sc-observability-types/Cargo.toml",
    "sc-observability": root / "crates/sc-observability/Cargo.toml",
    "sc-observe": root / "crates/sc-observe/Cargo.toml",
    "sc-observability-otlp": root / "crates/sc-observability-otlp/Cargo.toml",
}
for path in core_manifests.values():
    assert_no_core_boundary_dependencies(path)

# Exercise each banned prefix through the same manifest reader used above.
# Keep every declaration form in an independent fixture so a partial scanner
# cannot pass merely because another section contains the same banned package.
with tempfile.TemporaryDirectory(prefix="dependency-ban-fixtures-") as directory:
    fixture_root = Path(directory)
    for banned in BANNED_PREFIXES:
        alias = f"blocked_{banned.replace('-', '_')}"
        cases = {
            "direct": (
                f"[dependencies]\n{banned} = \"0.0.0\"\n",
                None,
            ),
            "renamed-package": (
                f"[dependencies]\n{alias} = {{ package = \"{banned}\", version = \"0.0.0\" }}\n",
                None,
            ),
            "workspace-inherited": (
                f"[dependencies]\n{alias} = {{ workspace = true }}\n",
                {
                    "workspace": {
                        "dependencies": {
                            alias: {"package": banned, "version": "0.0.0"}
                        }
                    }
                },
            ),
            "build": (
                f"[build-dependencies]\n{alias} = {{ package = \"{banned}\", version = \"0.0.0\" }}\n",
                None,
            ),
            "dev": (
                f"[dev-dependencies]\n{alias} = {{ package = \"{banned}\", version = \"0.0.0\" }}\n",
                None,
            ),
            "target": (
                f"[target.'cfg(unix)'.dependencies]\n{alias} = {{ package = \"{banned}\", version = \"0.0.0\" }}\n",
                None,
            ),
        }
        for case, (manifest, workspace_document) in cases.items():
            fixture = fixture_root / f"{banned}-{case}.toml"
            fixture.write_text(manifest, encoding="utf-8")
            try:
                assert_no_banned_dependencies(fixture, workspace_document)
            except SystemExit as error:
                if banned not in str(error):
                    raise SystemExit(
                        f"negative dependency-ban fixture lost {banned} in {case}: {error}"
                    )
            else:
                raise SystemExit(
                    f"negative dependency-ban fixture was accepted: {banned} in {case}"
                )

# Exercise the distinct core-runtime prohibition independently for every core
# crate and every forbidden package. These cases intentionally contain only
# one dependency declaration so each failure identifies its own rule.
with tempfile.TemporaryDirectory(prefix="core-boundary-fixtures-") as directory:
    fixture_root = Path(directory)
    for crate, _ in core_manifests.items():
        for forbidden in sorted(CORE_BOUNDARY_FORBIDDEN):
            fixture = fixture_root / f"{crate}-{forbidden}.toml"
            fixture.write_text(
                f"[dependencies]\n{forbidden} = \"0.0.0\"\n",
                encoding="utf-8",
            )
            try:
                assert_no_core_boundary_dependencies(fixture)
            except SystemExit as error:
                if forbidden not in str(error):
                    raise SystemExit(
                        f"negative core-boundary fixture lost {forbidden} for {crate}: {error}"
                    )
            else:
                raise SystemExit(
                    f"negative core-boundary fixture was accepted: {forbidden} for {crate}"
                )
print("dependency ban validation passed")
PY

python3 scripts/ci/validate_binding_runtime_dependencies.py
