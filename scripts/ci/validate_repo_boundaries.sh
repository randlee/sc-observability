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
    deps = data.get("dependencies", {})
    return set(deps.keys())

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

obs_deps = package_deps(root / "crates/sc-observability/Cargo.toml")
observe_deps = package_deps(root / "crates/sc-observe/Cargo.toml")
otlp_deps = package_deps(root / "crates/sc-observability-otlp/Cargo.toml")

if "sc-observability-otlp" in obs_deps or "sc-observe" in obs_deps:
    raise SystemExit("sc-observability must not depend on sc-observe or sc-observability-otlp")
if "sc-observability-otlp" in observe_deps:
    raise SystemExit("sc-observe must not depend on sc-observability-otlp")
if "sc-observe" not in otlp_deps:
    raise SystemExit("sc-observability-otlp must depend on sc-observe")

for path in root.rglob("Cargo.toml"):
    text = path.read_text(encoding="utf-8")
    if "agent-team-mail-" in text:
        raise SystemExit(f"agent-team-mail dependency reference found in {path}")

api = (root / "docs/api-design.md").read_text(encoding="utf-8")
arch = (root / "docs/architecture.md").read_text(encoding="utf-8")
req = (root / "docs/requirements.md").read_text(encoding="utf-8")

if "sc-observe -> sc-observability-otlp" in api:
    raise SystemExit("api-design.md still contains forbidden sc-observe -> sc-observability-otlp dependency")
if "pub otel:" in api or "ObservabilityConfig.otel" in api:
    raise SystemExit("api-design.md still embeds OTLP config in ObservabilityConfig")
if "atm-observability-adapter" not in arch:
    raise SystemExit("architecture.md missing explicit ATM adapter boundary")
if "OTLP-017" not in req or "OTLP-018" not in req:
    raise SystemExit("requirements.md missing OTLP attachment/TelemetryConfig requirements")

print("repo boundary validation passed")
PY
