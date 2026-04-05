#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from pathlib import Path

root = Path(".")
requirements = (root / "docs/requirements.md").read_text(encoding="utf-8")
architecture = (root / "docs/architecture.md").read_text(encoding="utf-8")
api_design = (root / "docs/api-design.md").read_text(encoding="utf-8")
phase_plan = (root / "docs/phase-1-sprint-assignment.md").read_text(encoding="utf-8")

expected_stack = "sc-observability-types\n  <- sc-observability\n    <- sc-observe\n      <- sc-observability-otlp"
if expected_stack not in requirements:
    raise SystemExit("requirements.md missing canonical layered dependency order")

architecture_checks = [
    "sc-observability-types\n  shared neutral contracts only",
    "sc-observability\n  lightweight logging only",
    "sc-observe\n  observation routing / pub-sub / projection",
    "sc-observability-otlp\n  OpenTelemetry / OTLP integration",
    "`TelemetryConfig` is constructed and owned by the application layer",
    "`sc-observability-otlp` registers its `LogProjector`, `SpanProjector`, and",
    "atm-observability-adapter",
]
for needle in architecture_checks:
    if needle not in architecture:
        raise SystemExit(f"architecture.md missing consistency marker: {needle!r}")

api_checks = [
    "sc-observability-types <- sc-observability <- sc-observe <- sc-observability-otlp",
    "`sc-observe` does not derive or own `TelemetryConfig`",
    "`TelemetryConfig` is constructed independently by the application layer and",
    "`sc-observability-otlp` attaches to the routing layer by registering its",
]
for needle in api_checks:
    if needle not in api_design:
        raise SystemExit(f"api-design.md missing consistency marker: {needle!r}")

required_ids = ["LAY-004", "OTLP-017", "OTLP-018", "NFR-009", "NFR-010"]
for req_id in required_ids:
    if req_id not in requirements:
        raise SystemExit(f"requirements.md missing required rule: {req_id}")

phase_checks = [
    "Sprint 5: Working ATM Adapter Example",
    "Sprint 6: Hardening And Release Readiness",
    "docs consistency checks",
    "dependency-ban enforcement",
]
for needle in phase_checks:
    if needle not in phase_plan:
        raise SystemExit(f"phase-1-sprint-assignment.md missing consistency marker: {needle!r}")

print("docs consistency validation passed")
PY

for crate in sc-observability-types sc-observability sc-observe sc-observability-otlp; do
  cargo rustdoc -p "$crate" -- -Dmissing-docs >/dev/null
done

echo "rustdoc missing-docs validation passed"
