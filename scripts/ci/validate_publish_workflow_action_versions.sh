#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from pathlib import Path
import re

root = Path('.github/workflows')
paths = sorted(root.glob('release*.yml')) + sorted(root.glob('*-publish.yml'))
minimum = {'checkout': 5, 'setup-python': 6}
failures = []
for path in paths:
    for line_no, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        match = re.search(r'uses:\s*actions/([A-Za-z0-9_-]+)@v(\d+)', line)
        if match and match.group(1) in minimum and int(match.group(2)) < minimum[match.group(1)]:
            failures.append(f'{path}:{line_no}: {match.group(1)}@v{match.group(2)} < v{minimum[match.group(1)]}')
if failures:
    raise SystemExit('publish workflow action version floor failed:\n' + '\n'.join(failures))
print(f'publish workflow action version floor passed ({len(paths)} workflows)')
PY
