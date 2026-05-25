#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from pathlib import Path

root = Path(".")
requirements = (root / "docs/requirements.md").read_text(encoding="utf-8")
architecture = (root / "docs/architecture.md").read_text(encoding="utf-8")
api_design = (root / "docs/api-design.md").read_text(encoding="utf-8")

requirements_checks = [
    "LOG-041 Retained-log maintenance shall run on the writer thread during idle or post-batch windows",
    "LOG-046 `Logger::shutdown()` shall drain queued events, stop the writer thread within the configured bounded shutdown timeout",
]
for needle in requirements_checks:
    if needle not in requirements:
        raise SystemExit(f"requirements.md missing writer-thread lock marker: {needle!r}")

architecture_checks = [
    "maintenance runs on the writer thread during idle or post-batch windows",
    "### ADR-010: Queue-Backed Writer Thread Owns Logging And Maintenance",
    "rejected alternative of retaining a dedicated maintenance-only worker",
]
for needle in architecture_checks:
    if needle not in architecture:
        raise SystemExit(f"architecture.md missing writer-thread lock marker: {needle!r}")

api_checks = [
    "pub fn log(&self, event: LogEvent) -> Result<(), LogError>;",
    "pub fn try_log(&self, event: LogEvent) -> Result<(), TryLogError>;",
    "pub queue_high_water_mark: u64,",
    "pub queue_full_drops_total: u64,",
    "pub writer_state: WriterState,",
]
for needle in api_checks:
    if needle not in api_design:
        raise SystemExit(f"api-design.md missing writer-thread lock marker: {needle!r}")

print("writer-thread lock validation passed")
PY
