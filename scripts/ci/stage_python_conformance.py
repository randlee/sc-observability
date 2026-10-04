#!/usr/bin/env python3
"""Include the shared conformance corpus with a relocated Python test suite."""
from __future__ import annotations

import argparse
import shutil
import os
import subprocess
from pathlib import Path


def stage_conformance(source: Path, tests: Path) -> Path:
    corpus = source / 'bindings/conformance/v1/conversion-cases.json'
    destination = tests / 'conformance/v1/conversion-cases.json'
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(corpus, destination)
    base = os.environ.get('SC_API_ACCEPTED_BASE', 'HEAD')
    accepted = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', base, '--', 'schema/api'], cwd=source, text=True)
    for name in accepted.splitlines():
        if name.endswith('.json'):
            previous = subprocess.check_output(['git', 'show', base + ':' + name], cwd=source)
            if (source / name).read_bytes() != previous:
                raise ValueError('immutable accepted API history changed: ' + name)
    # The existing native pytest suite checks the installed package, not source.
    shutil.copyfile(source / 'scripts/api/python_surface.py', tests / 'api_python_surface.py')
    shutil.copyfile(source / 'scripts/api/python_history_test.py', tests / 'test_public_api_history.py')
    shutil.copytree(source / 'schema/api/python-sc-observability', tests / 'api-history', dirs_exist_ok=True)
    return destination


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--tests', type=Path, required=True)
    arguments = parser.parse_args()
    stage_conformance(arguments.source, arguments.tests)
