"""Copied into the existing isolated installed-wheel pytest suite at staging."""
import importlib.metadata
from pathlib import Path
import time

import sc_observability
from api_python_surface import compare, project


def test_installed_public_api_history():
    started = time.monotonic()
    version = importlib.metadata.version('sc-observability')
    compare(project(sc_observability), Path(__file__).parent / 'api-history' / (version + '.json'), version)
    elapsed = time.monotonic() - started
    print(f'Python API check: {elapsed:.3f}s; builds in check: 0; installed version: {version}')
    assert elapsed < 60, f'Python API check exceeded 60 seconds: {elapsed:.3f}s'
