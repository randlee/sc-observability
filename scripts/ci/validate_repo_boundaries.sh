#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from pathlib import Path
import re
import subprocess
import tomllib

root = Path(".")

def load_toml(path: Path):
    return tomllib.loads(path.read_text(encoding="utf-8"))

def package_deps(path: Path):
    data = load_toml(path)
    names = set()
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        deps = data.get(section, {})
        names.update(deps.keys())
    return names

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

shared_crate_roots = [
    root / "crates/sc-observability-types",
    root / "crates/sc-observability",
    root / "crates/sc-observe",
    root / "crates/sc-observability-otlp",
]

source_files = []
for crate_root in shared_crate_roots:
    if crate_root.exists():
        source_files.extend(
            p for p in crate_root.rglob("*")
            if p.suffix in {".rs", ".toml"} and p.is_file()
        )

for path in source_files:
    text = path.read_text(encoding="utf-8")
    if "agent-team-mail-" in text or "agent_team_mail" in text:
        raise SystemExit(f"ATM coupling reference found in shared crate source: {path}")
    if re.search(r"\bATM_[A-Z0-9_]+\b", text):
        raise SystemExit(f"ATM-prefixed env/config reference found in shared crate source: {path}")
    if any(
        token in text
        for token in [
            "dirs::home_dir",
            "dirs_next::home_dir",
            "home_dir()",
            'var(\"HOME\")',
            "var_os(\"HOME\")",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
        ]
    ):
        raise SystemExit(f"home/path discovery reference found in shared crate source: {path}")

for path in [
    root / "crates/sc-observability-types/Cargo.toml",
    root / "crates/sc-observability/Cargo.toml",
    root / "crates/sc-observe/Cargo.toml",
]:
    deps = package_deps(path)
    if any(name.startswith("opentelemetry") or "otlp" in name for name in deps):
        raise SystemExit(f"OTLP/OpenTelemetry dependency found outside sc-observability-otlp: {path}")

api = (root / "docs/api-design.md").read_text(encoding="utf-8")
arch = (root / "docs/architecture.md").read_text(encoding="utf-8")
req = (root / "docs/requirements.md").read_text(encoding="utf-8")
atm_example = (root / "docs/atm-adapter-example.md").read_text(encoding="utf-8")

if "sc-observe -> sc-observability-otlp" in api:
    raise SystemExit("api-design.md still contains forbidden sc-observe -> sc-observability-otlp dependency")
if "pub otel:" in api or "ObservabilityConfig.otel" in api:
    raise SystemExit("api-design.md still embeds OTLP config in ObservabilityConfig")
if "ObservabilityConfig.otel" in arch or "ObservabilityConfig owns OTLP config" in arch:
    raise SystemExit("architecture.md still implies ObservabilityConfig owns OTLP config")
if "ObservabilityConfig.otel" in req:
    raise SystemExit("requirements.md still implies ObservabilityConfig owns OTLP config")
if "ObservabilityConfig.otel" in atm_example:
    raise SystemExit("atm-adapter-example.md still implies ObservabilityConfig owns OTLP config")
if "atm-observability-adapter" not in arch:
    raise SystemExit("architecture.md missing explicit ATM adapter boundary")
if "OTLP-017" not in req or "OTLP-018" not in req:
    raise SystemExit("requirements.md missing OTLP attachment/TelemetryConfig requirements")

subprocess.run(
    ["cargo", "check", "--manifest-path", "examples/atm-adapter-example/Cargo.toml"],
    cwd=root,
    check=True,
)

print("repo boundary validation passed")
PY
