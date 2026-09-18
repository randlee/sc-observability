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
import threading
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
        self.acls: list[tuple[Path, Path]] = []
        self.firewall = 'sc-observability-proof-' + uuid.uuid4().hex
        self.firewall_rules: list[str] = []
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
        recovery = os.environ.get('SC_PROOF_RECOVERY_FILE')
        self.recovery = Path(recovery) if recovery and self.system == 'Windows' else None
        if self.system == 'Linux':
            library_dir = sysconfig.get_config_var('LIBDIR')
            if library_dir:
                self.env['LD_LIBRARY_PATH'] = str(library_dir) + os.pathsep + self.env.get('LD_LIBRARY_PATH', '')
        self.cache_probe = Path.home() / '.cargo' / ('sc-observability-probe-' + uuid.uuid4().hex)
        self.network_ip = socket.gethostbyname('index.crates.io')
        with socket.create_connection((self.network_ip, 443), timeout=10):
            pass
        self.cache_probe.write_text('qualification cache denial sentinel')

    @staticmethod
    def powershell(script: str) -> None:
        subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-Command',
                        "$ErrorActionPreference='Stop'; " + script], check=True,
                       cwd=tempfile.gettempdir(), timeout=60)

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
            if os.environ.get('GITHUB_ACTIONS') != 'true':
                raise DistributionError('Windows ACL/firewall isolation requires an ephemeral CI runner')
            # Ephemeral CI runner: save and restore every ACL, and remove only our rule.
            account = subprocess.check_output(['whoami'], text=True).strip()
            try:
                for index, path in enumerate(self.denied):
                    if path.exists():
                        saved = self.scratch / f'acl-{index}.txt'
                        subprocess.run(['icacls', str(path), '/save', str(saved), '/T', '/C'], check=True,
                                       stdout=subprocess.DEVNULL)
                        self.acls.append((path, saved))
                        self.record_recovery()
                        subprocess.run(['icacls', str(path), '/deny', account + ':(OI)(CI)(R)', '/C'],
                                       check=True, stdout=subprocess.DEVNULL)
            except BaseException:
                self.__exit__(None, None, None)
                raise
        else:
            raise DistributionError(f'unsupported sandbox platform: {self.system}')
        return self

    def __exit__(self, *_):
        if self.system == 'Windows':
            try:
                self.remove_firewall()
            finally:
                self.restore_acls()
        self.cache_probe.unlink(missing_ok=True)
        if getattr(self, 'recovery', None):
            self.recovery.unlink(missing_ok=True)

    def record_recovery(self):
        if not getattr(self, 'recovery', None):
            return
        pending = self.recovery.with_suffix('.pending')
        pending.write_text(json.dumps({'schema_version': 1, 'firewall': self.firewall,
                                      'firewall_rules': self.firewall_rules,
                                      'acls': [[str(root), str(saved)] for root, saved in self.acls]}),
                           encoding='utf-8')
        pending.replace(self.recovery)

    def restore_acls(self):
        failures = []
        for path, saved in reversed(self.acls):
            try:
                subprocess.run(['icacls', str(path.parent), '/restore', str(saved), '/C'],
                               check=True, stdout=subprocess.DEVNULL, timeout=60)
            except (subprocess.SubprocessError, OSError) as error:
                failures.append(f'{path}: {error}')
        if failures:
            raise DistributionError('ACL restoration failed: ' + '; '.join(failures))

    def abort_windows_proof(self):
        # A stuck child or capture-pipe cleanup must not leave the ephemeral
        # runner disconnected. Restoring access is never a successful proof:
        # terminate the qualification unconditionally with a nonzero status.
        print('WINDOWS_SANDBOX_WATCHDOG_TIMEOUT: restoring isolation; qualification failed',
              file=sys.stderr, flush=True)
        try:
            self.__exit__(None, None, None)
        except Exception as error:
            print(f'WINDOWS_SANDBOX_RESTORATION_ERROR: {error}', file=sys.stderr, flush=True)
        finally:
            os._exit(124)

    def remove_firewall(self):
        # INetFwRules.Remove is an idempotent exact-name operation, including
        # when our rule is absent. Avoid enumerating the runner's firewall.
        # https://learn.microsoft.com/windows/win32/api/netfw/nf-netfw-inetfwrules-remove
        rules = '; '.join(f"$policy.Rules.Remove('{name}')" for name in getattr(self, 'firewall_rules', [self.firewall]))
        self.powershell("$policy = New-Object -ComObject HNetCfg.FwPolicy2; " + rules)

    @contextmanager
    def network_denial(self, program: Path | None = None):
        """Cover one complete command or real-webview process lifetime."""
        if self.system != 'Windows':
            yield
            return
        # Longer than the 900-second command bound plus kill and cleanup
        # allowances. Covers capture cleanup as well as the command itself.
        expired = threading.Event()
        def deadline():
            expired.set()
            self.abort_windows_proof()
        watchdog = threading.Timer(1050, deadline)
        watchdog.daemon = True
        watchdog.start()
        try:
            self.firewall_rules = []
            # Establish the deny boundary before any proof child starts. The
            # runner's own control processes are explicitly allowed to keep
            # the job alive; OverrideBlockRules makes this safe with the
            # process-tree-wide block below.
            allow_script = ("$names=@('Runner.Listener','Runner.Worker','Runner.PluginHost'); "
                            "$paths=@(Get-Process -Name $names -ErrorAction SilentlyContinue | "
                            "Where-Object Path | Select-Object -ExpandProperty Path -Unique); "
                            "$paths -join [Environment]::NewLine")
            output = subprocess.check_output(['powershell', '-NoProfile', '-NonInteractive', '-Command', allow_script], text=True)
            for index, path in enumerate(line for line in output.splitlines() if line.strip()):
                name = f'{self.firewall}-allow-{index}'
                self.powershell(f"New-NetFirewallRule -Name '{name}' -DisplayName '{name}' "
                                f"-Program '{path}' -Direction Outbound -Action Allow -OverrideBlockRules $true -Profile Any | Out-Null")
                self.firewall_rules.append(name)
                self.record_recovery()
            self.powershell(f"New-NetFirewallRule -Name '{self.firewall}' -DisplayName '{self.firewall}' "
                            "-Direction Outbound -Action Block -Profile Any | Out-Null")
            self.firewall_rules.append(self.firewall)
            self.record_recovery()
            yield
        finally:
            try:
                self.remove_firewall()
            finally:
                watchdog.cancel()
                watchdog.join(timeout=210)
                if expired.is_set() or watchdog.is_alive():
                    raise DistributionError('Windows sandbox watchdog exceeded its bound')

    def run(self, command: list[str], cwd: Path, *, expect_failure: bool = False) -> str:
        print('B4A_COMMAND ' + json.dumps(command), flush=True)
        started = time.monotonic()
        with self.network_denial(Path(command[0])):
            result = bounded_command(self.prefix + command, cwd, self.env)
        print(f'B4A_EXIT {result.returncode} after {time.monotonic() - started:.2f}s', flush=True)
        self.commands.append({'command': command, 'exit_code': result.returncode,
                              'stdout': result.stdout, 'stderr': result.stderr})
        if (result.returncode == 0) == expect_failure:
            raise DistributionError(f'isolation command had unexpected result: {command}\nSTDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}')
        return result.stdout

    def prove_denials(self, python: str, checkout: Path) -> dict:
        """The destination was verified reachable before applying the deny policy."""
        code = '''import pathlib,socket,sys
for item in sys.argv[1:3]:
 try: pathlib.Path(item).read_bytes()
 except (PermissionError, FileNotFoundError): pass
 else: raise SystemExit('forbidden file readable: '+item)
sock=socket.socket(); sock.settimeout(2)
try: sock.connect((sys.argv[3],443))
except OSError: pass
else: raise SystemExit('network remained reachable')
print('CHECKOUT_CACHE_NETWORK_DENIED')
'''
        output = self.run([python, '-I', '-c', code, str(checkout / 'Cargo.toml'),
                           str(self.cache_probe), self.network_ip], self.scratch)
        if 'CHECKOUT_CACHE_NETWORK_DENIED' not in output:
            raise DistributionError('missing isolation denial proof')
        return {'checkout': True, 'cargo_cache': True, 'network': True,
                'denied_roots': [str(path) for path in self.denied]}
