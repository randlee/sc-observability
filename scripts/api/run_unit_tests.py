#!/usr/bin/env python3
"""Run the existing unit command once; compare API only after it has completed."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from history import ApiError, ROOT, check_current, collect_artifacts, compiler_environment, source_fingerprint


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--accepted-base', default=os.environ.get('SC_API_ACCEPTED_BASE', 'HEAD'))
    parser.add_argument('--capture', action='store_true', help='release-cut only: create new version snapshots')
    parser.add_argument('--record', type=Path, default=ROOT / 'target/api/current-build.json')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if command[:2] != ['cargo', 'test']:
        parser.error('supply the normal cargo test command after --')
    if any(arg.startswith('--message-format') for arg in command):
        parser.error('message format is supplied by this runner')
    sysroot = subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip()
    unit_env = compiler_environment(sysroot)
    before = source_fingerprint()
    started = time.monotonic()
    messages = []
    build_complete = False
    process = subprocess.Popen([*command, '--message-format=json-render-diagnostics'], cwd=ROOT,
                               stdout=subprocess.PIPE, text=True, env=unit_env)
    for line in process.stdout:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            print(line, end='', flush=True)
            continue
        if not isinstance(message, dict):
            print(line, end='', flush=True)
            continue
        if message.get('reason') == 'build-finished':
            build_complete = message.get('success') is True
        elif message.get('reason') == 'compiler-artifact':
            messages.append(message)
        elif message.get('reason') == 'compiler-message':
            print(message['message'].get('rendered', ''), end='', flush=True)
        elif message.get('reason') not in ('build-finished', 'build-script-executed'):
            print(line, end='', flush=True)
    unit_status = process.wait()
    elapsed = time.monotonic() - started
    # Preserve test failure aggregation: API checking is an independent outcome
    # after Cargo has completed, including when a runtime test has failed.
    try:
        if not build_complete:
            raise ApiError('Cargo did not complete a successful build; partial artifact streams cannot verify the API')
        if before != source_fingerprint():
            raise ApiError('source changed while normal unit tests ran')
        record = collect_artifacts(messages)
        record.update(source_sha256=before, cargo_command=command, unit_status=unit_status,
                      normal_unit_seconds=elapsed,
                      sysroot=sysroot)
        args.record.parent.mkdir(parents=True, exist_ok=True)
        args.record.write_text(json.dumps(record, indent=2) + '\n')
        report = check_current(record, args.accepted_base, capture=args.capture)
        print('API unit test PASS ' + json.dumps(report, sort_keys=True))
    except (ApiError, OSError, subprocess.SubprocessError) as error:
        print(f'API unit test FAIL: {error}', file=sys.stderr)
        return unit_status or 1
    return unit_status


if __name__ == '__main__':
    raise SystemExit(main())
