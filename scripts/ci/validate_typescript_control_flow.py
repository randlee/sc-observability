#!/usr/bin/env python3
"""Reject throwing/panicking operational paths in authored binding sources."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main():
    paths = sorted((ROOT / 'bindings/typescript/src').glob('*.ts'))
    paths += [ROOT / 'bindings/tauri/src/lib.rs', ROOT / 'examples/tauri-logging/src-tauri/src/main.rs']
    violations = []
    for path in paths:
        if path.name == 'test.ts':
            continue
        source = path.read_text()
        # The adapter's cfg(test) module is a trailing unit-test module. Public
        # operational code is above it; test assertions intentionally may panic.
        if path.suffix == '.rs':
            source = source.split('#[cfg(test)]\nmod tests {', 1)[0]
            pattern = r'\b(?:panic|todo|unimplemented)!\s*\(|\.(?:unwrap|expect)\s*\('
        else:
            pattern = r'\bthrow\s+(?:new\s+)?\w|\bPromise\.reject\s*\('
        for number, line in enumerate(source.splitlines(), 1):
            if line.lstrip().startswith(('//', '*')):
                continue
            if re.search(pattern, line):
                violations.append(f'{path.relative_to(ROOT)}:{number}: {line.strip()}')
    if violations:
        raise SystemExit('\n'.join(violations))
    print('TYPESCRIPT_TAURI_AUTHORED_CONTROL_FLOW_PASSED')


if __name__ == '__main__':
    main()
