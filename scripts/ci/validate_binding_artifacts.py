#!/usr/bin/env python3
"""Check the existing input/output hashes for generated binding artifacts."""
import hashlib
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def main():
    evidence=json.loads((ROOT/'bindings/generation-manifest.json').read_text())
    for path,expected in evidence['inputs'].items():
        current=(ROOT/path).read_bytes()
        if hashlib.sha256(current).hexdigest()!=expected:raise SystemExit(f'generation source/toolchain drift: {path}')
    for path,expected in evidence['outputs'].items():
        if hashlib.sha256((ROOT/path).read_bytes()).hexdigest()!=expected:raise SystemExit(f'generated artifact hash drift: {path}')
    if evidence['toolchain']!={'python':'3.12.10','rust':'1.94.1','schemars':'1.2.2'}:raise SystemExit('generation toolchain pin drift')
    print('GENERATION_ARTIFACT_HASHES_PASSED: input/output content hashes, toolchains, locks, schema and outputs')
if __name__=='__main__':main()
