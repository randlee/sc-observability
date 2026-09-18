#!/usr/bin/env python3
"""Retain failed-proof diagnostics if an ephemeral Windows proof worker dies."""
import argparse
import json
import os
import platform
import shutil
import subprocess
import tempfile
from pathlib import Path

from _windows_identity import recover


def invalidate_evidence(directory, result):
    path = directory / 'platform.json'
    if result and path.exists():
        report = json.loads(path.read_text(encoding='utf-8'))
        report['status'] = 'failed'
        report['windows_supervisor_exit'] = result
        path.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence', type=Path, default=Path('target/tauri-qualification'))
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if platform.system() != 'Windows' or os.environ.get('GITHUB_ACTIONS') != 'true' or not command:
        parser.error('requires an ephemeral Windows CI runner and a proof command')
    with tempfile.TemporaryDirectory(prefix='windows-proof-supervisor-') as temporary:
        environment = dict(os.environ, SC_WINDOWS_IDENTITY_CONTROL_ROOT=str(Path(temporary).resolve()))
        process = subprocess.Popen(command, env=environment)
        result = 125
        try:
            result = process.wait(timeout=1800)
        except subprocess.TimeoutExpired:
            print('WINDOWS_PROOF_SUPERVISOR_TIMEOUT: qualification failed after 1800 seconds', flush=True)
            try:
                subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=20)
            except subprocess.TimeoutExpired:
                pass
            if process.poll() is None:
                process.kill()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                pass
            result = 124
        finally:
            # Recovery always invalidates success, even if the worker somehow
            # exited zero while leaving access restrictions behind.
            recovery_errors = []
            recovered = []
            args.evidence.mkdir(parents=True, exist_ok=True)
            for journal in Path(temporary).glob('windows-identity-control-*/identity-recovery.json'):
                try:
                    recover(journal)
                    recovered.append(str(journal))
                except Exception as error:
                    recovery_errors.append(str(error))
                    shutil.copytree(journal.parent, args.evidence / ('failed-recovery-' + str(len(recovery_errors))), dirs_exist_ok=True)
                    print(f'WINDOWS_PROOF_RECOVERY_ERROR: {error}', flush=True)
                finally:
                    result = result or 125
            (args.evidence / 'windows-supervisor.json').write_text(json.dumps(
                {'exit': result, 'recovered': recovered, 'recovery_errors': recovery_errors}, indent=2), encoding='utf-8')
            invalidate_evidence(args.evidence, result)
        print(f'WINDOWS_PROOF_SUPERVISOR_EXIT {result}', flush=True)
        raise SystemExit(result)


if __name__ == '__main__':
    main()
