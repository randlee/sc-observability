#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from pathlib import Path
import tomllib

root = Path(".")

def load_toml(path: Path):
    return tomllib.loads(path.read_text(encoding="utf-8"))

def package_deps(path: Path):
    data = load_toml(path)
    names = set()
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        names.update(data.get(section, {}).keys())
    return names

def section_deps(path: Path, section: str):
    data = load_toml(path)
    return set(data.get(section, {}).keys())

workspace = load_toml(root / "Cargo.toml")
members = set(workspace["workspace"]["members"])
required_members = {
    "crates/sc-observability-types",
    "crates/sc-observability",
    "crates/sc-observe",
    "crates/sc-observability-otlp",
}

missing = required_members - members
if missing:
    raise SystemExit(f"missing workspace members: {sorted(missing)}")

for path in root.rglob("Cargo.toml"):
    text = path.read_text(encoding="utf-8")
    if "agent-team-mail-" in text or "agent_team_mail" in text:
        raise SystemExit(f"ATM dependency reference found in {path}")

obs_deps = package_deps(root / "crates/sc-observability/Cargo.toml")
observe_deps = package_deps(root / "crates/sc-observe/Cargo.toml")
otlp_deps = package_deps(root / "crates/sc-observability-otlp/Cargo.toml")
otlp_runtime_deps = section_deps(root / "crates/sc-observability-otlp/Cargo.toml", "dependencies")
otlp_test_deps = section_deps(root / "crates/sc-observability-otlp/Cargo.toml", "dev-dependencies")

if obs_deps != {"serde_json", "sc-observability-types"}:
    raise SystemExit(
        "sc-observability dependency set drifted from allowed baseline: "
        f"{sorted(obs_deps)}"
    )

if observe_deps != {"sc-observability-types", "sc-observability"}:
    raise SystemExit(
        "sc-observe dependency set drifted from allowed baseline: "
        f"{sorted(observe_deps)}"
    )

required_otlp = {
    "serde_json",
    "thiserror",
    "sc-observability-types",
}
if otlp_runtime_deps != required_otlp:
    raise SystemExit(
        "sc-observability-otlp runtime dependency set drifted from allowed baseline: "
        f"{sorted(otlp_runtime_deps)}"
    )

allowed_otlp_test = {"sc-observability", "sc-observe"}
if otlp_test_deps != allowed_otlp_test:
    raise SystemExit(
        "sc-observability-otlp test dependency set drifted from allowed baseline: "
        f"{sorted(otlp_test_deps)}"
    )

for path in [
    root / "crates/sc-observability-types/Cargo.toml",
    root / "crates/sc-observability/Cargo.toml",
    root / "crates/sc-observe/Cargo.toml",
]:
    deps = package_deps(path)
    if any(name.startswith("opentelemetry") or "otlp" in name for name in deps):
        raise SystemExit(f"OTLP/OpenTelemetry dependency found outside sc-observability-otlp: {path}")

print("dependency ban validation passed")
PY
