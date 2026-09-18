#!/usr/bin/env python3
"""Retain failed-proof diagnostics if an ephemeral Windows proof worker dies."""
import argparse
import json
import os
import platform
import re
import subprocess
import tempfile
from pathlib import Path

from _python_distribution import DistributionError
from _python_sandbox import Sandbox, registered_checkouts


def recover(path, allowed_roots, temporary_root):
    if not path.exists():
        return False
    record = json.loads(path.read_text(encoding='utf-8'))
    if record.get('schema_version') != 1 or not re.fullmatch(r'sc-observability-proof-[0-9a-f]{32}', record.get('firewall', '')):
        raise DistributionError('invalid proof recovery identity')
    rules = record.get('firewall_rules', [record['firewall']])
    prefix = record['firewall'] + '-allow-'
    if (not isinstance(rules, list) or
            any(not isinstance(name, str) or
                (name != record['firewall'] and
                 (not name.startswith(prefix) or not name[len(prefix):].isdigit()))
                for name in rules)):
        raise DistributionError('invalid proof recovery rules')
    acls = [(Path(root), Path(saved)) for root, saved in record['acls']]
    for root, saved in acls:
        if root not in allowed_roots or '..' in saved.parts or not saved.is_relative_to(temporary_root) or not re.fullmatch(r'acl-\d+\.txt', saved.name):
            raise DistributionError('proof recovery path outside owned roots')
    sandbox = Sandbox.__new__(Sandbox)
    sandbox.firewall = record['firewall']
    sandbox.firewall_rules = rules
    sandbox.acls = acls
    print('WINDOWS_PROOF_RECOVERY: restoring saved isolation after worker failure', flush=True)
    try:
        sandbox.remove_firewall()
    finally:
        sandbox.restore_acls()
    path.unlink()
    return True


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
    allowed = set(registered_checkouts(Path.cwd())) | {Path.home() / '.cargo'}
    temporary_root = Path(tempfile.gettempdir()).resolve()
    with tempfile.TemporaryDirectory(prefix='windows-proof-supervisor-') as temporary:
        recovery = Path(temporary).resolve() / 'recovery.json'
        environment = dict(os.environ, SC_PROOF_RECOVERY_FILE=str(recovery))
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
            try:
                if recover(recovery, allowed, temporary_root):
                    result = result or 125
            except Exception as error:
                result = result or 125
                print(f'WINDOWS_PROOF_RECOVERY_ERROR: {error}', flush=True)
            finally:
                invalidate_evidence(args.evidence, result)
        print(f'WINDOWS_PROOF_SUPERVISOR_EXIT {result}', flush=True)
        raise SystemExit(result)


if __name__ == '__main__':
    main()
