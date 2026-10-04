#!/usr/bin/env python3
"""Run the repository-local OTLP dev-install smoke."""
from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TESTS = (
    'public_sync_http_factory_exports_decoded_trace_counter_and_gauge',
    'public_sync_http_factory_recovers_a_partial_log_export_failure',
)


def main() -> None:
    for test in TESTS:
        subprocess.run(
            [
                'cargo', 'test', '--locked', '-p', 'sc-observability-otlp',
                '--test', 'full_stack_integration', '--features', 'sync-http',
                '--', test,
            ],
            cwd=ROOT,
            check=True,
        )
    print('OTLP_DEV_INSTALL_SMOKE_PASS: hermetic_collector public factory checks')


if __name__ == '__main__':
    main()
