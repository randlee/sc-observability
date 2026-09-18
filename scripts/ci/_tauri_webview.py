"""Execute real webviews with a controllable OS pipe for native sink faults."""
from __future__ import annotations
import json
import subprocess
import threading
import time
from pathlib import Path


def execute(sandbox, executable: Path, host: Path, scratch: Path, output: Path):
    # Windows firewall policy is per-command; retain it for the whole webview
    # lifetime just as sandbox.run does for Cargo and denial probes.
    with sandbox.network_denial(executable):
        return _execute(sandbox, executable, host, scratch, output)


def _execute(sandbox, executable: Path, host: Path, scratch: Path, output: Path):
    control = scratch / 'output-control'
    sandbox.env['SC_TAURI_QUALIFICATION_CONTROL'] = str(control)
    drained = threading.Event()
    drained.set()
    stdout_path = output / 'webview-stdout.log'
    stderr_path = output / 'webview-stderr.log'
    with stdout_path.open('wb') as stdout, stderr_path.open('wb') as stderr:
        process = subprocess.Popen(sandbox.prefix + [str(executable)], cwd=host, env=sandbox.env,
                                   stdout=subprocess.PIPE, stderr=stderr)
        def drain():
            while True:
                drained.wait()
                chunk = process.stdout.read1(4096)
                if not chunk:
                    return
                stdout.write(chunk)
                stdout.flush()
        reader = threading.Thread(target=drain, name='qualification-stdout-reader', daemon=True)
        reader.start()
        deadline = time.monotonic() + 180
        transitions = []
        last_token = None
        try:
            while process.poll() is None:
                if time.monotonic() > deadline:
                    raise RuntimeError('actual webview execution exceeded 180 seconds')
                request_path = Path(str(control) + '.request')
                if request_path.exists():
                    try:
                        request = json.loads(request_path.read_text(encoding='utf-8'))
                    except (ValueError, OSError):
                        request = None
                    if request and request['token'] != last_token:
                        (drained.clear if request['paused'] else drained.set)()
                        last_token = request['token']
                        transitions.append(request)
                        Path(str(control) + '.ack').write_text(json.dumps({'token': last_token}))
                time.sleep(0.01)
        finally:
            drained.set()
            if process.poll() is None:
                process.kill()
            process.wait()
            reader.join(timeout=5)
            sandbox.commands.append({'command': [str(executable)], 'exit_code': process.returncode,
                                     'stdout_artifact': stdout_path.name, 'stderr_artifact': stderr_path.name,
                                     'output_gate_transitions': transitions})
        if process.returncode:
            raise RuntimeError(f'actual webview process failed: {process.returncode}')
        return transitions
