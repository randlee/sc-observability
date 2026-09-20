#!/usr/bin/env python3
"""Reject incomplete or mixed-revision B.2 platform qualification evidence."""
import argparse
import json
from pathlib import Path

from _log_staging import ASSERTIONS, PACKAGES, PLATFORMS, sha256, verify_stage


def validate(stage: Path, directory: Path, version: str, source: str) -> None:
    manifest = verify_stage(stage, version, source)
    archives = {item['name']: item['archive_sha256'] for item in manifest['packages']}
    for platform in PLATFORMS:
        result_path = directory / f'{platform}.json'
        result = json.loads(result_path.read_text())
        if (result.get('status') != 'passed' or result.get('platform') != platform
                or result.get('candidate_version') != version or result.get('source_commit') != source
                or result.get('stage_manifest_sha256') != sha256(stage / 'stage-manifest.json')
                or result.get('archives') != archives or result.get('assertions') != list(ASSERTIONS)
                or set(result.get('dependency_resolution', {})) != set(PACKAGES)
                or result.get('log_sha256') != sha256(result_path.with_suffix('.log'))):
            raise ValueError(f'incomplete or mismatched {platform} evidence')
    print('B.2 all three platforms passed against the same six immutable archives')


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument('--stage', type=Path, required=True)
    parser.add_argument('--evidence-dir', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit', required=True)
    args = parser.parse_args()
    validate(args.stage, args.evidence_dir, args.version, args.source_commit)


if __name__ == '__main__':
    main()
