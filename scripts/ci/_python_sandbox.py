"""Execute distribution proofs with checkout/cache reads and network denied."""
from __future__ import annotations

import json
import os
import platform
import shutil
import signal
import socket
import subprocess
import sys
import sysconfig
import time
import tempfile
import uuid
from pathlib import Path
from contextlib import contextmanager

from _python_distribution import DistributionError


def bounded_command(command: list[str], cwd: Path, environment: dict, timeout: float = 900) -> subprocess.CompletedProcess:
    """Bound command lifetime without waiting for inherited output handles."""
    # Compiler service descendants can retain their parent's output handles.
    # Regular files preserve output without making completion depend on EOF.
    with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
        process = subprocess.Popen(command, cwd=cwd, env=environment, stdout=stdout_file,
                                   stderr=stderr_file, start_new_session=os.name != 'nt')
        def captured(handle):
            size = os.fstat(handle.fileno()).st_size
            handle.seek(0)
            return handle.read(size).decode('utf-8', errors='replace')
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            if os.name == 'nt':
                try:
                    subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=20)
                except subprocess.TimeoutExpired:
                    pass
            else:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            if process.poll() is None:
                process.kill()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                pass
            raise DistributionError(f'qualification command exceeded {timeout:g} seconds: {command}\n'
                                    + captured(stdout_file) + '\n' + captured(stderr_file)) from error
        return subprocess.CompletedProcess(command, process.returncode,
                                           captured(stdout_file), captured(stderr_file))


def registered_checkouts(checkout: Path) -> list[Path]:
    result = subprocess.check_output(['git', '-c', 'safe.directory=' + str(checkout.resolve()), 'worktree', 'list', '--porcelain'], cwd=checkout, text=True)
    return sorted({Path(line.removeprefix('worktree ')).resolve()
                   for line in result.splitlines() if line.startswith('worktree ')})


class Sandbox:
    """Every child command uses the same deny policy; unsupported isolation fails."""

    def __init__(self, scratch: Path, checkouts: list[Path]):
        self.scratch = scratch.resolve()
        self.denied = sorted(set(checkouts + [Path.home() / '.cargo']))
        if any(self.scratch.is_relative_to(path) for path in self.denied):
            raise DistributionError('proof directory must be outside all checkouts and Cargo caches')
        self.commands: list[dict] = []
        self.identity = None
        self.prefix: list[str] = []
        self.env = {key: value for key, value in os.environ.items()
                    if not key.startswith(('CARGO_', 'RUST', 'PYO3_', 'PYTHONPATH', 'PYTHONHOME'))}
        tool = lambda name: subprocess.check_output(['rustup', 'which', '--toolchain', '1.94.1', name], text=True).strip()
        self.cargo, self.rustc = tool('cargo'), tool('rustc')
        (self.scratch / 'temporary').mkdir(exist_ok=True)
        self.env.update(TMPDIR=str(self.scratch / 'temporary'), TEMP=str(self.scratch / 'temporary'), TMP=str(self.scratch / 'temporary'), CARGO_HOME=str(self.scratch / 'cargo-home'),
                        CARGO_TARGET_DIR=str(self.scratch / 'cargo-target'),
                        RUSTC=self.rustc, RUSTDOC=tool('rustdoc'),
                        PATH=str(Path(self.cargo).parent) + os.pathsep + os.environ['PATH'],
                        PYO3_PYTHON=sys.executable, PYTHONDONTWRITEBYTECODE='1')
        self.system = platform.system()
        if self.system == 'Linux':
            library_dir = sysconfig.get_config_var('LIBDIR')
            if library_dir:
                self.env['LD_LIBRARY_PATH'] = str(library_dir) + os.pathsep + self.env.get('LD_LIBRARY_PATH', '')
        self.cache_probe = Path.home() / '.cargo' / ('sc-observability-probe-' + uuid.uuid4().hex)
        self.network_ip = socket.gethostbyname('index.crates.io')
        with socket.create_connection((self.network_ip, 443), timeout=10):
            pass
        self.cache_probe.write_text('qualification cache denial sentinel')

    def __enter__(self):
        if self.system == 'Darwin':
            profile = self.scratch / 'isolation.sb'
            profile.write_text('(version 1)\n(allow default)\n(deny network*)\n' + '\n'.join(
                f'(deny file-read* (subpath {json.dumps(str(path))}))' for path in self.denied))
            self.prefix = ['/usr/bin/sandbox-exec', '-f', str(profile)]
        elif self.system == 'Linux':
            if not shutil.which('bwrap'):
                raise DistributionError('bubblewrap is required; isolation cannot be skipped')
            self.prefix = ['bwrap', '--die-with-parent', '--unshare-net', '--ro-bind', '/', '/',
                           '--dev-bind', '/dev', '/dev', '--ro-bind', '/proc', '/proc',
                           '--bind', str(self.scratch), str(self.scratch)]
            for path in self.denied:
                if path.exists():
                    self.prefix += ['--tmpfs', str(path)]
            self.prefix += ['--']
        elif self.system == 'Windows':
            from _windows_identity import Identity
            if not os.environ.get('SC_WINDOWS_IDENTITY_CONTROL_ROOT'):
                raise DistributionError('Windows proof requires the independent recovery supervisor')
            self.identity = Identity(self.scratch, self.denied)
            try:
                self.identity.setup()
                self.identity.activate()
            except BaseException:
                self.identity.close()
                raise
        else:
            raise DistributionError(f'unsupported sandbox platform: {self.system}')
        return self

    def __exit__(self, *_):
        try:
            if self.identity is not None:
                self.identity.close()
        finally:
            self.cache_probe.unlink(missing_ok=True)

    @contextmanager
    def network_denial(self, program: Path | None = None):
        """The identity policy spans the complete sandbox and every descendant."""
        if self.system == 'Windows' and (self.identity is None or not self.identity.active):
            raise DistributionError('Windows proof identity policy is not active')
        yield

    def spawn(self, command, cwd, *, stdout=None, stderr=None):
        with self.network_denial():
            if self.system == 'Windows':
                return self.identity.spawn(command, cwd=cwd, environment=self.env,
                                           stdout=stdout, stderr=stderr)
            return subprocess.Popen(self.prefix + command, cwd=cwd, env=self.env,
                                    stdout=stdout, stderr=stderr)

    def run(self, command: list[str], cwd: Path, *, expect_failure: bool = False) -> str:
        print('B4A_COMMAND ' + json.dumps(command), flush=True)
        started = time.monotonic()
        with self.network_denial(Path(command[0])):
            if self.system == 'Windows':
                result = self.identity.run(command, cwd=cwd, environment=self.env, timeout=900)
            else:
                result = bounded_command(self.prefix + command, cwd, self.env)
        print(f'B4A_EXIT {result.returncode} after {time.monotonic() - started:.2f}s', flush=True)
        self.commands.append({'command': command, 'exit_code': result.returncode,
                              'stdout': result.stdout, 'stderr': result.stderr})
        if (result.returncode == 0) == expect_failure:
            raise DistributionError(f'isolation command had unexpected result: {command}\nSTDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}')
        return result.stdout

    def prove_denials(self, python: str, checkout: Path) -> dict:
        """The destination was verified reachable before applying the deny policy."""
        code = '''import pathlib,socket,subprocess,sys
for item in sys.argv[1:3]:
 try: pathlib.Path(item).read_bytes()
 except (PermissionError, FileNotFoundError): pass
 else: raise SystemExit('forbidden file readable: '+item)
sock=socket.socket(); sock.settimeout(2)
try: sock.connect((sys.argv[3],443))
except OSError: pass
else: raise SystemExit('network remained reachable')
# A fresh interpreter attempts its socket before launching the next child.
# All three generations also repeat the actual checkout/cache file probes.
if int(sys.argv[4]):
 subprocess.run([sys.executable,'-I',__file__,*sys.argv[1:4],str(int(sys.argv[4])-1)],check=True,timeout=15)
print('CHECKOUT_CACHE_NETWORK_DENIED')
'''
        probe = self.scratch / 'isolation-probe.py'
        probe.write_text(code, encoding='utf-8')
        output = self.run([python, '-I', str(probe), str(checkout / 'Cargo.toml'),
                           str(self.cache_probe), self.network_ip, '2'], self.scratch)
        if output.count('CHECKOUT_CACHE_NETWORK_DENIED') != 3:
            raise DistributionError('missing isolation denial proof')
        return {'checkout': True, 'cargo_cache': True, 'network': True,
                'process_generations': 3, 'denied_roots': [str(path) for path in self.denied]}
