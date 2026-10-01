#!/usr/bin/env python3
"""Run the repository-local replacement for the historical dev-install smoke.

source: scripts/otel-dev-install-smoke.py
git_blob: 6524662aa9d37f21451759accef52c2cfea51c78
sha256: f38a13677e11a5eaafef711e16d28e125810094a8b813f2af3c29954a883f58d
disposition: transplant-and-adapt
allowed_delta: workspace-public-factory
allowed_delta: hermetic-collector
allowed_delta: three-signal-contract
"""
from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TESTS = (
    'public_legacy_http_json_factory_exports_decoded_trace_counter_and_gauge',
    'public_legacy_factory_recovers_a_partial_log_export_failure',
)


def main() -> None:
    for test in TESTS:
        subprocess.run(
            [
                'cargo', 'test', '--locked', '-p', 'sc-observability-otlp',
                '--test', 'full_stack_integration', '--features', 'legacy-http-json',
                '--', test,
            ],
            cwd=ROOT,
            check=True,
        )
    print('OTLP_DEV_INSTALL_SMOKE_PASS: hermetic_collector public factory checks')


if __name__ == '__main__':
    main()
