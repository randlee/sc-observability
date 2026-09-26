#!/usr/bin/env python3
"""Compare committed generation provenance without rewriting expected hashes.

Source blobs pin bytes, not a commit that a stack rebase can discard. Each
pinned blob is retained by the committed input file, including in shallow
clones. Updating a source SHA-256 alone cannot replace its pinned Git bytes.
"""
import hashlib
import json
import re
import subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def main():
    evidence=json.loads((ROOT/'bindings/generation-manifest.json').read_text())
    blobs = evidence['source_blobs']
    if set(blobs) != set(evidence['inputs']):
        raise SystemExit('generation source blob inventory drift')
    for path,expected in evidence['inputs'].items():
        current=(ROOT/path).read_bytes()
        blob = blobs[path]
        if not isinstance(blob, str) or not re.fullmatch(r'[0-9a-f]{40}', blob):
            raise SystemExit(f'invalid generation source blob: {path}')
        try:
            recorded=subprocess.check_output(['git','cat-file','blob',blob],cwd=ROOT,stderr=subprocess.PIPE)
        except subprocess.CalledProcessError as error:
            raise SystemExit(f'generation source blob unavailable: {path}') from error
        if current!=recorded or hashlib.sha256(current).hexdigest()!=expected:raise SystemExit(f'generation source/toolchain drift: {path}')
    for path,expected in evidence['outputs'].items():
        if hashlib.sha256((ROOT/path).read_bytes()).hexdigest()!=expected:raise SystemExit(f'generated artifact hash drift: {path}')
    if evidence['toolchain']!={'python':'3.12.10','rust':'1.94.1','schemars':'1.2.2'}:raise SystemExit('generation toolchain pin drift')
    print('GENERATION_ARTIFACT_HASHES_PASSED: source blobs, toolchains, locks, schema and outputs')
if __name__=='__main__':main()
