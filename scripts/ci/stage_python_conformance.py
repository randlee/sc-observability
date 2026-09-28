#!/usr/bin/env python3
"""Include the shared conformance corpus with a relocated Python test suite."""
from __future__ import annotations

import argparse
import shutil
from pathlib import Path


def stage_conformance(source: Path, tests: Path) -> Path:
    corpus = source / 'bindings/conformance/v1/conversion-cases.json'
    destination = tests / 'conformance/v1/conversion-cases.json'
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(corpus, destination)
    return destination


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--tests', type=Path, required=True)
    arguments = parser.parse_args()
    stage_conformance(arguments.source, arguments.tests)
