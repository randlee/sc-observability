#!/usr/bin/env python3
"""Release-cut capture from an already-installed native wheel; never builds."""
import argparse
import importlib.metadata
import json
import os
from pathlib import Path
import subprocess
import time

from python_surface import project


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--accepted-base', default=os.environ.get('SC_API_ACCEPTED_BASE', 'HEAD'))
    args = parser.parse_args()
    import sc_observability
    import sc_observability._native  # require real installed native wheel
    version = importlib.metadata.version('sc-observability')
    relative = 'schema/api/python-sc-observability/' + version + '.json'
    if subprocess.run(['git', 'cat-file', '-e', args.accepted_base + ':' + relative], cwd=args.root, capture_output=True).returncode == 0:
        raise SystemExit('accepted Python history is immutable; increment the package version')
    started = time.monotonic()
    rows = project(sc_observability)
    path = args.root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({'format': 'python-runtime/v1', 'version': version, 'rows': rows}, indent=2, sort_keys=True) + '\n')
    print(f'Python release-cut inspection: {time.monotonic() - started:.3f}s; {len(rows)} rows')


if __name__ == '__main__':
    main()
