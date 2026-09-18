#!/usr/bin/env python3
"""Bounded index-visibility gate used only after B.7 performs real publication."""
import argparse
import json
import re
import time
import urllib.error
import urllib.request


def visible(package: str, version: str) -> bool:
    if not re.fullmatch(r'[a-z0-9][a-z0-9_-]{3,}', package):
        raise ValueError('expected a normalized crate name of at least four characters')
    url = f'https://index.crates.io/{package[:2]}/{package[2:4]}/{package}'
    request = urllib.request.Request(url, headers={'User-Agent': 'sc-observability-release-index-gate/1.0', 'Cache-Control': 'no-cache'})
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            rows = response.read().decode().splitlines()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise
    return any((row := json.loads(line)).get('vers') == version and not row.get('yanked', False) for line in rows)


def wait(package: str, version: str, attempts: int, interval: float) -> None:
    if not 1 <= attempts <= 30 or not 0 <= interval <= 60:
        raise ValueError('attempts must be 1..30; interval must be 0..60 seconds')
    for attempt in range(1, attempts + 1):
        try:
            if visible(package, version):
                print(f'index visible: {package} {version} (attempt {attempt}/{attempts})')
                return
        except (urllib.error.URLError, TimeoutError) as error:
            print(f'index request failed for {package}: {error}', flush=True)
        print(f'index not yet visible: {package} {version} (attempt {attempt}/{attempts})', flush=True)
        if attempt < attempts:
            time.sleep(interval)
    raise RuntimeError(f'index visibility exhausted after {attempts} attempts: {package} {version}; publication sequence stopped')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--attempts', type=int, default=12)
    parser.add_argument('--interval', type=float, default=10)
    args = parser.parse_args()
    wait(args.package, args.version, args.attempts, args.interval)


if __name__ == '__main__':
    main()
