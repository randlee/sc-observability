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
    try:
        result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-EncodedCommand', encoded],
                                text=True, encoding='utf-8', capture_output=True, timeout=60,
                                env=environment)
    except subprocess.TimeoutExpired:
        raise RuntimeError('Windows identity control operation exceeded 60 seconds') from None
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
        self.get_security = self.advapi.GetFileSecurityW
        self.get_security.argtypes = [W.LPCWSTR, W.DWORD, ctypes.c_void_p, W.DWORD, ctypes.POINTER(W.DWORD)]
        self.get_security.restype = W.BOOL
        self.set_security = self.advapi.SetFileSecurityW
        self.set_security.argtypes = [W.LPCWSTR, W.DWORD, ctypes.c_void_p]
        self.set_security.restype = W.BOOL

    def save_acls(self, root, destination):
        records = []
        paths = [root]
        for parent, directories, files in os.walk(root, followlinks=False):
            paths.extend(Path(parent) / name for name in directories + files)
        for path in paths:
            if path.is_symlink() or (hasattr(path, 'is_junction') and path.is_junction()):
                raise RuntimeError('ACL snapshot refuses reparse paths')
            size = W.DWORD()
            self.get_security(str(path), 7, None, 0, ctypes.byref(size))
            if not size.value:
                raise ctypes.WinError(ctypes.get_last_error())
            data = ctypes.create_string_buffer(size.value)
            self.check(self.get_security(str(path), 7, data, size.value, ctypes.byref(size)))
            records.append([path.relative_to(root).as_posix(), base64.b64encode(data.raw).decode('ascii')])
        destination.write_text(json.dumps(records), encoding='utf-8')

    def restore_acls(self, root, source):
        # SetNamedSecurityInfo/Set-Acl converts old descriptors to automatic
        # inheritance, changing their control bits. The documented legacy
        # SetFileSecurity API deliberately does not propagate inheritance;
        # replay each saved descriptor so existing descendants are exact too.
        for relative, encoded in reversed(json.loads(source.read_text(encoding='utf-8'))):
            path = root / relative
            if Path(relative).is_absolute() or '..' in Path(relative).parts:
                raise ValueError('ACL snapshot escapes its root')
            if path.exists():
                data = ctypes.create_string_buffer(base64.b64decode(encoded, validate=True))
                self.check(self.set_security(str(path), 4, data))

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
    # TEMP may use RUNNER~1 while the protected journal records the long name.
    # Compare canonical filesystem paths, not two spellings of the same root.
    journal = Path(journal).resolve()
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
    # A controller may die after suspended process creation but before job
    # assignment. Reap that narrow gap by the unique account SID before any
    # access is restored. This is recovery, never the enforcement boundary.
    powershell("$u=Get-LocalUser -Name " + literal(account) + " -ErrorAction SilentlyContinue; "
        "if($u){$sid=$u.SID.Value; $until=[DateTime]::UtcNow.AddSeconds(10); do { "
        "$owned=@(Get-CimInstance Win32_Process | Where-Object { "
        "(Invoke-CimMethod -InputObject $_ -MethodName GetOwnerSid -ErrorAction SilentlyContinue).Sid -eq $sid }); "
        "if(!$owned.Count){break}; foreach($p in $owned){ "
        "$result=Invoke-CimMethod -InputObject $p -MethodName Terminate -Arguments @{Reason=124} -ErrorAction SilentlyContinue }; "
        "if([DateTime]::UtcNow -ge $until){throw 'proof identity processes did not drain; retaining deny'}; "
        "Start-Sleep -Milliseconds 50 } while($true)}")
    # Access is restored only after every owned job and identity process drains.
    powershell("$p=New-Object -ComObject HNetCfg.FwPolicy2; $p.Rules.Remove(" + literal(record['rule']) + ")")
    errors = []
    for root, saved, sddl in reversed(record.get('acls', [])):
        root, saved = Path(root), Path(saved)
        if root == Path(root.anchor) or not saved.is_relative_to(journal.parent) or not re.fullmatch(r'acl-[0-9]+\.txt', saved.name):
            raise ValueError('invalid proof ACL journal')
        try:
            api.restore_acls(root, saved)
        except (subprocess.SubprocessError, OSError, RuntimeError) as error:
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
    def __init__(self, scratch, denied, readable=()):
        if os.name != 'nt' or os.environ.get('GITHUB_ACTIONS') != 'true':
            raise RuntimeError('identity proof requires an ephemeral Windows Actions runner')
        self.scratch = Path(scratch).resolve()
        self.denied = [Path(path).resolve() for path in denied]
        self.readable = [Path(path).resolve() for path in readable]
        if any(path.is_relative_to(root) for path in [self.scratch, *self.readable] for root in self.denied):
            raise ValueError("proof scratch/tools overlap a denied root")
        self.account = 'scp' + uuid.uuid4().hex[:16]
        self.password = secrets.token_urlsafe(32) + '!aA9'
        self.control = Path(tempfile.mkdtemp(prefix='windows-identity-control-',
            dir=os.environ.get('SC_WINDOWS_IDENTITY_CONTROL_ROOT'))).resolve()
        controller = powershell('[Security.Principal.WindowsIdentity]::GetCurrent().User.Value')
        subprocess.run(['icacls', str(self.control), '/inheritance:r', '/grant:r',
                        '*' + controller + ':(OI)(CI)(F)', '*S-1-5-18:(OI)(CI)(F)'],
                       check=True, stdout=subprocess.DEVNULL, timeout=60)
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
        roots = [(self.scratch, "/grant", "(OI)(CI)(M)")]
        roots += [(root, "/grant", "(OI)(CI)(RX)") for root in self.readable]
        roots += [(root, "/deny", "(OI)(CI)(R)") for root in self.denied]
        for index, (root, mode, rights) in enumerate(roots):
            if not root.exists():
                continue
            saved = self.control / f'acl-{index}.txt'
            self.api.save_acls(root, saved)
            original = powershell('(Get-Acl -LiteralPath ' + literal(root) + ').Sddl')
            self.record['acls'].append([str(root), str(saved), original])
            self.write()
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

    def environment(self, extra=None):
        allowed = {'SYSTEMROOT', 'WINDIR', 'SYSTEMDRIVE', 'COMSPEC', 'PATH', 'PATHEXT',
                   'PROGRAMFILES', 'PROGRAMFILES(X86)', 'PROGRAMW6432', 'PROGRAMDATA',
                   'NUMBER_OF_PROCESSORS', 'PROCESSOR_ARCHITECTURE', 'LIB', 'LIBPATH',
                   'INCLUDE', 'VSINSTALLDIR', 'VCINSTALLDIR', 'VCTOOLSINSTALLDIR',
                   'UCRTVERSION', 'WINDOWSSDKLIBVERSION', 'WINDOWSSDKDIR'}
        result = {key: value for key, value in os.environ.items() if key.upper() in allowed}
        for key, value in (extra or {}).items():
            if key.upper() in allowed or key.startswith(('CARGO_', 'PYO3_', 'PYTHON', 'XDG_', 'SC_TAURI_QUALIFICATION_')) or key in {'RUSTC', 'RUSTDOC', 'RUSTFLAGS', 'SC_OBSERVABILITY_RUNTIME_TEST'}:
                result[key] = value
        profile = self.scratch / 'proof-profile'
        for path in (profile, profile / 'AppData' / 'Local', profile / 'AppData' / 'Roaming'):
            path.mkdir(parents=True, exist_ok=True)
        result.update(TEMP=str(self.scratch / 'temporary'), TMP=str(self.scratch / 'temporary'),
                      PYTHONUTF8='1', USERPROFILE=str(profile), HOME=str(profile),
                      APPDATA=str(profile / 'AppData' / 'Roaming'), LOCALAPPDATA=str(profile / 'AppData' / 'Local'))
        (self.scratch / 'temporary').mkdir(exist_ok=True)
        return result

    def spawn(self, command, *, cwd=None, environment=None, stdout=None, stderr=None, baseline=False):
        if not self.active and not baseline:
            raise RuntimeError('proof launch before network policy')
        return ProofProcess(self, command, cwd or self.scratch, self.environment(environment), stdout, stderr)

    def run(self, command, *, cwd=None, environment=None, timeout=30, baseline=False):
        if not self.active and not baseline:
            raise RuntimeError('proof launch before network policy')
        # Regular files avoid compiler descendants holding a capture pipe open.
        with tempfile.TemporaryFile(dir=self.control) as stdout, tempfile.TemporaryFile(dir=self.control) as stderr:
            process = self.spawn(command, cwd=cwd, environment=environment, stdout=stdout, stderr=stderr, baseline=baseline)
            try:
                process.wait(timeout)
            finally:
                process.close()
            stdout.seek(0); stderr.seek(0)
            return subprocess.CompletedProcess(command, process.returncode,
                stdout.read().decode('utf-8', errors='replace'), stderr.read().decode('utf-8', errors='replace'))

    def close(self):
        recover(self.journal)
        shutil.rmtree(self.control)
        self.active = False


class ProofProcess:
    """Minimal Popen-compatible process with an identity-bound, non-breakaway job."""
    def __init__(self, identity, command, cwd, environment, stdout, stderr):
        import msvcrt
        self.api = identity.api
        self.returncode = None
        self.closed = False
        self.stdout = None
        self.info = ProcessInfo()
        executable = shutil.which(str(command[0]), path=environment.get('PATH'))
        if executable is None:
            raise FileNotFoundError(command[0])
        command_line = subprocess.list2cmdline([executable, *map(str, command[1:])])
        # CreateProcessWithLogonW has a smaller bound than CreateProcessW.
        # Stage large probe scripts as files instead of passing inline source.
        if len(command_line.encode('utf-16le')) // 2 >= 1024:
            raise ValueError('proof command exceeds CreateProcessWithLogonW 1024-character limit; stage source as a file')
        name = identity.account + '-job-' + uuid.uuid4().hex
        identity.record['jobs'].append(name)
        identity.write()
        self.job = self.api.check(self.api.kernel.CreateJobObjectW(None, name))
        owned = []
        try:
            limits = ExtendedLimits()
            limits.basic.flags = 0x2000  # Kill-on-close; no breakaway flags.
            self.api.check(self.api.kernel.SetInformationJobObject(self.job, 9, ctypes.byref(limits), ctypes.sizeof(limits)))
            if stdout == subprocess.PIPE:
                read, write = os.pipe()
                self.stdout = os.fdopen(read, 'rb')
                stdout = os.fdopen(write, 'wb')
                owned.append(stdout)
            if stdout is None:
                stdout = open(os.devnull, 'wb'); owned.append(stdout)
            if stderr is None:
                stderr = open(os.devnull, 'wb'); owned.append(stderr)
            stdin = open(os.devnull, 'rb'); owned.append(stdin)
            startup = Startup()
            startup.cb, startup.flags = ctypes.sizeof(startup), 0x100
            for field, stream in [('stdin', stdin), ('stdout', stdout), ('stderr', stderr)]:
                handle = msvcrt.get_osfhandle(stream.fileno())
                os.set_handle_inheritable(handle, True)
                setattr(startup, field, handle)
            block = ctypes.create_unicode_buffer('\0'.join(f'{key}={value}' for key,value in sorted(environment.items()))+'\0')
            args = ctypes.create_unicode_buffer(command_line)
            self.api.check(self.api.create(identity.account, '.', identity.password, 1, executable, args,
                0x4 | 0x400, block, str(cwd), ctypes.byref(startup), ctypes.byref(self.info)))
            self.pid = self.info.pid
            self.api.check(self.api.kernel.AssignProcessToJobObject(self.job,self.info.process))
            if self.api.kernel.ResumeThread(self.info.thread)==0xffffffff:
                raise ctypes.WinError(ctypes.get_last_error())
        except BaseException:
            if self.info.process:
                self.api.kernel.TerminateProcess(self.info.process,124)
                self.api.kernel.CloseHandle(self.info.process)
                self.api.kernel.CloseHandle(self.info.thread)
            self.api.terminate_job(self.job)
            self.api.kernel.CloseHandle(self.job)
            if self.stdout is not None: self.stdout.close()
            raise
        finally:
            for stream in owned: stream.close()

    def poll(self):
        if self.returncode is not None: return self.returncode
        status = self.api.kernel.WaitForSingleObject(self.info.process,0)
        if status==258: return None
        if status!=0: raise ctypes.WinError(ctypes.get_last_error())
        code = W.DWORD()
        self.api.check(self.api.kernel.GetExitCodeProcess(self.info.process,ctypes.byref(code)))
        self.returncode = code.value
        return self.returncode

    def wait(self, timeout=None):
        if self.closed: return self.returncode
        status = self.api.kernel.WaitForSingleObject(self.info.process,0xffffffff if timeout is None else int(timeout*1000))
        if status==258: raise subprocess.TimeoutExpired('identity proof',timeout)
        if status!=0: raise ctypes.WinError(ctypes.get_last_error())
        self.poll()
        self.close()
        return self.returncode

    def kill(self):
        if not self.closed:
            self.api.terminate_job(self.job)

    def close(self):
        if self.closed: return
        self.kill()
        self.poll()
        self.api.kernel.CloseHandle(self.info.thread)
        self.api.kernel.CloseHandle(self.info.process)
        self.api.kernel.CloseHandle(self.job)
        self.closed = True
