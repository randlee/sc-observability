#!/usr/bin/env python3
"""Run the repository-local OTLP dev-install smoke."""
from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TESTS = (
    'canonical_ingress_exports_every_field_over_the_sdk_backend',
    'canonical_ingress_exports_every_field_over_the_sync_http_backend',
)


def main() -> None:
    for test in TESTS:
        subprocess.run(
            [
                'cargo', 'test', '--locked', '-p', 'sc-observability-otlp',
                '--test', 'canonical_ingress', '--features', 'otlp-sdk,sync-http',
                '--', test,
            ],
            cwd=ROOT,
            check=True,
        )
    print('OTLP_DEV_INSTALL_SMOKE_PASS: hermetic_collector canonical ingress checks')


if __name__ == '__main__':
    main()
