"""Windows proof identity and kernel-owned descendant lifetime (CI only).

The controller keeps its identity and network access. The proof account receives
an outbound LocalUser deny before its first proof instruction. No host defaults
or pre-existing firewall rules are modified.
"""
from __future__ import annotations
import ctypes
import base64
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import time
import tempfile
import uuid


def powershell(script, *, sensitive=False):
    # A pwsh workflow exports PowerShell 7 module paths; Windows PowerShell 5.1
    # must calculate its own paths or core modules such as Get-Acl fail to load.
    environment = {key: value for key, value in os.environ.items()
                   if key.casefold() != 'psmodulepath'}
    encoded = base64.b64encode(("$ErrorActionPreference='Stop';\n" + script).encode('utf-16le')).decode('ascii')
    result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-EncodedCommand', encoded],
                            text=True, encoding='utf-8', capture_output=True, timeout=60,
                            env=environment)
    if result.returncode:
        # Scripts may contain the temporary account password; never echo input.
        detail = 'account provisioning (details suppressed)' if sensitive else result.stderr[-2000:]
        raise RuntimeError('Windows identity control operation failed: ' + detail)
    return result.stdout.strip()


def literal(value):
    return "'" + str(value).replace("'", "''") + "'"


class Startup(ctypes.Structure):
    _fields_ = [('cb', W.DWORD), ('reserved', W.LPWSTR), ('desktop', W.LPWSTR),
                ('title', W.LPWSTR), ('x', W.DWORD), ('y', W.DWORD), ('xs', W.DWORD),
                ('ys', W.DWORD), ('xc', W.DWORD), ('yc', W.DWORD), ('fill', W.DWORD),
                ('flags', W.DWORD), ('show', W.WORD), ('reserved2_size', W.WORD),
                ('reserved2', ctypes.c_void_p), ('stdin', W.HANDLE),
                ('stdout', W.HANDLE), ('stderr', W.HANDLE)]


class ProcessInfo(ctypes.Structure):
    _fields_ = [('process', W.HANDLE), ('thread', W.HANDLE), ('pid', W.DWORD), ('tid', W.DWORD)]


class Limits(ctypes.Structure):
    _fields_ = [('process_time', ctypes.c_longlong), ('job_time', ctypes.c_longlong),
                ('flags', W.DWORD), ('min_working', ctypes.c_size_t),
                ('max_working', ctypes.c_size_t), ('active_limit', W.DWORD),
                ('affinity', ctypes.c_size_t), ('priority', W.DWORD), ('scheduling', W.DWORD)]


class ExtendedLimits(ctypes.Structure):
    _fields_ = [('basic', Limits), ('io', ctypes.c_ulonglong * 6),
                ('process_memory', ctypes.c_size_t), ('job_memory', ctypes.c_size_t),
                ('peak_process', ctypes.c_size_t), ('peak_job', ctypes.c_size_t)]


class Accounting(ctypes.Structure):
    _fields_ = [('times', ctypes.c_longlong * 4), ('faults', W.DWORD),
                ('total', W.DWORD), ('active', W.DWORD), ('terminated', W.DWORD)]


class Api:
    def __init__(self):
        self.kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        self.advapi = ctypes.WinDLL('advapi32', use_last_error=True)
        specifications = {
            'CreateJobObjectW': ([ctypes.c_void_p, W.LPCWSTR], W.HANDLE),
            'OpenJobObjectW': ([W.DWORD, W.BOOL, W.LPCWSTR], W.HANDLE),
            'SetInformationJobObject': ([W.HANDLE, ctypes.c_int, ctypes.c_void_p, W.DWORD], W.BOOL),
            'QueryInformationJobObject': ([W.HANDLE, ctypes.c_int, ctypes.c_void_p, W.DWORD, ctypes.c_void_p], W.BOOL),
            'AssignProcessToJobObject': ([W.HANDLE, W.HANDLE], W.BOOL),
            'TerminateJobObject': ([W.HANDLE, W.UINT], W.BOOL),
            'TerminateProcess': ([W.HANDLE, W.UINT], W.BOOL),
            'ResumeThread': ([W.HANDLE], W.DWORD),
            'WaitForSingleObject': ([W.HANDLE, W.DWORD], W.DWORD),
            'GetExitCodeProcess': ([W.HANDLE, ctypes.POINTER(W.DWORD)], W.BOOL),
            'CloseHandle': ([W.HANDLE], W.BOOL),
        }
        for name, (args, result) in specifications.items():
            function = getattr(self.kernel, name)
            function.argtypes, function.restype = args, result
        self.create = self.advapi.CreateProcessWithLogonW
        self.create.argtypes = [W.LPCWSTR, W.LPCWSTR, W.LPCWSTR, W.DWORD,
            W.LPCWSTR, W.LPWSTR, W.DWORD, ctypes.c_void_p, W.LPCWSTR,
            ctypes.POINTER(Startup), ctypes.POINTER(ProcessInfo)]
        self.create.restype = W.BOOL

    @staticmethod
    def check(result):
        if not result:
            raise ctypes.WinError(ctypes.get_last_error())
        return result

    def terminate_job(self, handle):
        self.check(self.kernel.TerminateJobObject(handle, 124))
        until = time.monotonic() + 10
        while True:
            info = Accounting()
            self.check(self.kernel.QueryInformationJobObject(handle, 1, ctypes.byref(info), ctypes.sizeof(info), None))
            if not info.active:
                return
            if time.monotonic() >= until:
                raise RuntimeError('proof job did not drain; retaining network deny')
            time.sleep(.02)


def recover(journal):
    """Idempotent recovery from every planned setup state; never trusts arbitrary names."""
    journal = Path(journal)
    if not journal.exists():
        return False
    record = json.loads(journal.read_text(encoding='utf-8'))
    if record.get('schema') != 1 or not re.fullmatch(r'scp[0-9a-f]{16}', record.get('account', '')):
        raise ValueError('invalid proof identity journal')
    account = record['account']
    if record.get('rule') != 'sc-proof-' + account:
        raise ValueError('invalid proof rule identity')
    api = Api()
    for name in record.get('jobs', []):
        if not re.fullmatch(re.escape(account) + r'-job-[0-9a-f]{32}', name):
            raise ValueError('invalid proof job identity')
        handle = api.kernel.OpenJobObjectW(0x1f003f, False, name)
        if handle:
            try:
                api.terminate_job(handle)
            finally:
                api.kernel.CloseHandle(handle)
        elif ctypes.get_last_error() != 2:
            raise ctypes.WinError(ctypes.get_last_error())
    # Access is restored only after every owned job has drained.
    powershell("$p=New-Object -ComObject HNetCfg.FwPolicy2; $p.Rules.Remove(" + literal(record['rule']) + ")")
    errors = []
    for root, saved in reversed(record.get('acls', [])):
        root, saved = Path(root), Path(saved)
        if root == Path(root.anchor) or not saved.is_relative_to(journal.parent) or not re.fullmatch(r'acl-[0-9]+\.txt', saved.name):
            raise ValueError('invalid proof ACL journal')
        try:
            subprocess.run(['icacls', str(root.parent), '/restore', str(saved), '/C'],
                           check=True, stdout=subprocess.DEVNULL, timeout=60)
        except (subprocess.SubprocessError, OSError) as error:
            errors.append(str(error))
    # Account and profile are unique to this proof, never runner accounts.
    powershell("$u=Get-LocalUser -Name " + literal(account) + " -ErrorAction SilentlyContinue; "
               "if($u){$sid=$u.SID.Value; Remove-LocalUser -Name " + literal(account) + "; "
               "Get-CimInstance Win32_UserProfile | Where-Object SID -eq $sid | Remove-CimInstance}")
    if errors:
        raise RuntimeError('proof ACL restoration failed: ' + '; '.join(errors))
    journal.unlink()
    return True


class Identity:
    def __init__(self, scratch, denied):
        if os.name != 'nt' or os.environ.get('GITHUB_ACTIONS') != 'true':
            raise RuntimeError('identity proof requires an ephemeral Windows Actions runner')
        self.scratch = Path(scratch).resolve()
        self.denied = [Path(path).resolve() for path in denied]
        self.account = 'scp' + uuid.uuid4().hex[:16]
        self.password = secrets.token_urlsafe(32) + '!aA9'
        self.control = Path(tempfile.mkdtemp(prefix='windows-identity-control-',
            dir=os.environ.get('SC_WINDOWS_IDENTITY_CONTROL_ROOT'))).resolve()
        self.journal = self.control / 'identity-recovery.json'
        self.record = {'schema': 1, 'account': self.account, 'rule': 'sc-proof-' + self.account,
                       'acls': [], 'jobs': []}
        self.active = False
        self.api = Api()

    def write(self):
        pending = self.journal.with_suffix('.pending')
        pending.write_text(json.dumps(self.record), encoding='utf-8')
        pending.replace(self.journal)

    def setup(self):
        self.write()  # Identity and rule intent precede every mutation.
        self.sid = powershell("$p=ConvertTo-SecureString " + literal(self.password) + " -AsPlainText -Force; "
            "$u=New-LocalUser -Name " + literal(self.account) + " -Password $p -AccountNeverExpires; $u.SID.Value", sensitive=True)
        self.record['sid'] = self.sid
        self.write()
        for index, root in enumerate([self.scratch, *self.denied]):
            if not root.exists():
                continue
            saved = self.control / f'acl-{index}.txt'
            subprocess.run(['icacls', str(root), '/save', str(saved), '/T', '/C'],
                           check=True, stdout=subprocess.DEVNULL, timeout=60)
            self.record['acls'].append([str(root), str(saved)])
            self.write()
            mode, rights = ('/grant', '(OI)(CI)(M)') if index == 0 else ('/deny', '(OI)(CI)(R)')
            subprocess.run(['icacls', str(root), mode, '*' + self.sid + ':' + rights, '/C'],
                           check=True, stdout=subprocess.DEVNULL, timeout=60)
        return self

    def activate(self):
        # SID condition covers future descendants and arbitrary executable names.
        powershell("New-NetFirewallRule -Name " + literal(self.record['rule']) +
            " -DisplayName " + literal(self.record['rule']) +
            " -Direction Outbound -Action Block -Profile Any -Protocol Any -Authentication NotRequired "
            "-LocalUser " + literal('D:(A;;CC;;;' + self.sid + ')') + " | Out-Null")
        self.active = True

    def environment(self):
        allowed = {'SYSTEMROOT', 'WINDIR', 'SYSTEMDRIVE', 'COMSPEC', 'PATH', 'PATHEXT',
                   'PROGRAMFILES', 'PROGRAMFILES(X86)', 'PROGRAMW6432', 'PROGRAMDATA',
                   'NUMBER_OF_PROCESSORS', 'PROCESSOR_ARCHITECTURE'}
        result = {key: value for key, value in os.environ.items() if key.upper() in allowed}
        result.update(TEMP=str(self.scratch), TMP=str(self.scratch), PYTHONUTF8='1')
        return result

    def run(self, command, *, timeout=30, baseline=False):
        if not self.active and not baseline:
            raise RuntimeError('proof launch before network policy')
        executable = shutil.which(command[0])
        if executable is None:
            raise FileNotFoundError(command[0])
        name = self.account + '-job-' + uuid.uuid4().hex
        self.record['jobs'].append(name)
        self.write()
        api = self.api
        job = api.check(api.kernel.CreateJobObjectW(None, name))
        limits = ExtendedLimits()
        limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, no breakaway.
        api.check(api.kernel.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)))
        info = ProcessInfo()
        try:
            import msvcrt
            with (self.scratch / 'stdout.txt').open('w+b') as stdout, (self.scratch / 'stderr.txt').open('w+b') as stderr, open(os.devnull, 'rb') as stdin:
                startup = Startup()
                startup.cb, startup.flags = ctypes.sizeof(startup), 0x100
                for field, stream in [('stdin', stdin), ('stdout', stdout), ('stderr', stderr)]:
                    handle = msvcrt.get_osfhandle(stream.fileno())
                    os.set_handle_inheritable(handle, True)
                    setattr(startup, field, handle)
                environment = self.environment()
                block = ctypes.create_unicode_buffer('\0'.join(f'{k}={v}' for k, v in sorted(environment.items())) + '\0')
                args = ctypes.create_unicode_buffer(subprocess.list2cmdline([executable, *command[1:]]))
                api.check(api.create(self.account, '.', self.password, 1, executable, args,
                                    0x4 | 0x400, block, str(self.scratch), ctypes.byref(startup), ctypes.byref(info)))
                try:
                    api.check(api.kernel.AssignProcessToJobObject(job, info.process))
                    if api.kernel.ResumeThread(info.thread) == 0xffffffff:
                        raise ctypes.WinError(ctypes.get_last_error())
                    status = api.kernel.WaitForSingleObject(info.process, int(timeout * 1000))
                    if status != 0:
                        raise TimeoutError('proof process exceeded bound')
                    code = W.DWORD()
                    api.check(api.kernel.GetExitCodeProcess(info.process, ctypes.byref(code)))
                finally:
                    # Also covers an assignment failure while the root is suspended.
                    api.kernel.TerminateProcess(info.process, 124)
                    api.terminate_job(job)
                    api.kernel.CloseHandle(info.thread)
                    api.kernel.CloseHandle(info.process)
                stdout.seek(0); stderr.seek(0)
                return subprocess.CompletedProcess(command, code.value,
                    stdout.read().decode('utf-8', errors='replace'), stderr.read().decode('utf-8', errors='replace'))
        finally:
            api.kernel.CloseHandle(job)

    def close(self):
        recover(self.journal)
        shutil.rmtree(self.control)
        self.active = False
