#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from pathlib import Path
import tomllib

root = Path(".")

def load_toml(path: Path):
    return tomllib.loads(path.read_text(encoding="utf-8"))

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

obs_runtime_deps = section_deps(root / "crates/sc-observability/Cargo.toml", "dependencies")
obs_test_deps = section_deps(root / "crates/sc-observability/Cargo.toml", "dev-dependencies")
observe_runtime_deps = section_deps(root / "crates/sc-observe/Cargo.toml", "dependencies")
observe_test_deps = section_deps(root / "crates/sc-observe/Cargo.toml", "dev-dependencies")
otlp_runtime_deps = section_deps(root / "crates/sc-observability-otlp/Cargo.toml", "dependencies")
otlp_test_deps = section_deps(root / "crates/sc-observability-otlp/Cargo.toml", "dev-dependencies")

if obs_runtime_deps != {"serde_json", "sc-observability-types"}:
    raise SystemExit(
        "sc-observability runtime dependency set drifted from allowed baseline: "
        f"{sorted(obs_runtime_deps)}"
    )
if obs_test_deps - {"temp-env"}:
    raise SystemExit(
        "sc-observability test dependency set drifted from allowed baseline: "
        f"{sorted(obs_test_deps - {'temp-env'})}"
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

print("dependency ban validation passed")
PY
