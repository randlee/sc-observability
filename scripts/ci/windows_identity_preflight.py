#!/usr/bin/env python3
"""Small actual Windows identity proof; no build matrix or publication."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

from _windows_identity import Identity, literal, powershell, recover

PROBE = r'''
using System;
using System.Diagnostics;
using System.Net.Sockets;
using System.Security.Principal;
class Probe {
 static int Main(string[] args) {
  bool connected=false;
  using(var client=new TcpClient()) {
   try { var ar=client.BeginConnect(args[0],443,null,null);
    if(ar.AsyncWaitHandle.WaitOne(2000)) {client.EndConnect(ar); connected=true;}
   } catch(SocketException) {}
  }
  Console.WriteLine("PROBE depth="+args[2]+" connected="+connected+" sid="+WindowsIdentity.GetCurrent().User.Value);
  if(connected != (args[1]=="allow")) return 10;
  int depth=int.Parse(args[2]);
  if(depth>0) {
   string path=System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"probe-"+(depth-1)+".exe");
   var start=new ProcessStartInfo(path,args[0]+" "+args[1]+" "+(depth-1));
   start.UseShellExecute=false;
   using(var child=Process.Start(start)) {
    if(!child.WaitForExit(15000)) return 11;
    return child.ExitCode;
   }
  }
  if(args[1]=="denyhold") {
   System.IO.File.WriteAllText(System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"leaf-ready"),Process.GetCurrentProcess().Id.ToString());
   System.Threading.Thread.Sleep(30000);
  }
  return 0;
 }
}
'''


def snapshot():
    return json.loads(powershell("@{profiles=@(Get-NetFirewallProfile | Sort-Object Name | Select-Object Name,Enabled,DefaultInboundAction,DefaultOutboundAction); "
        "rules=@(Get-NetFirewallRule | Sort-Object Name | Select-Object Name,Enabled,Direction,Action); "
        "users=@(Get-LocalUser | Sort-Object Name | Select-Object Name,@{n='SID';e={$_.SID.Value}})} | ConvertTo-Json -Depth 5 -Compress"))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=Path('target/windows-identity-preflight'))
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    report={'status':'failed','source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(), 'events':[]}
    before=snapshot()
    identity=None
    stop=threading.Event()
    samples=[]
    try:
        ip=socket.gethostbyname('index.crates.io')
        with socket.create_connection((ip,443),timeout=5): pass
        with tempfile.TemporaryDirectory(prefix='windows-proof-preflight-') as temporary:
            scratch=Path(temporary)
            denied=scratch.parent / (scratch.name+'-denied')
            denied.mkdir()
            (denied/'sentinel').write_text('must not read')
            original_acl=powershell('(Get-Acl '+literal(denied)+').Sddl')
            report['acl_before']=original_acl
            original_child_acl=powershell('(Get-Acl '+literal(denied/'sentinel')+').Sddl')
            try:
                executable=scratch/'probe-2.exe'
                powershell('Add-Type -TypeDefinition '+literal(PROBE)+' -Language CSharp -OutputAssembly '+literal(executable)+' -OutputType ConsoleApplication')
                for level in (0,1): shutil.copyfile(executable,scratch/f'probe-{level}.exe')
                identity=Identity(scratch,[denied])
                report['journal']=str(identity.journal)
                identity.setup()
                baseline=identity.run([str(executable),ip,'allow','2'],baseline=True)
                report['events'].append({'name':'same_identity_baseline','code':baseline.returncode,'stdout':baseline.stdout,'stderr':baseline.stderr})
                if baseline.returncode or baseline.stdout.count('connected=True') != 3: raise RuntimeError('proof identity baseline did not connect')
                identity.activate()
                rule=json.loads(powershell('Get-NetFirewallRule -Name '+literal(identity.record['rule'])+' | Select-Object Name,Enabled,Direction,Action,Profile | ConvertTo-Json -Compress'))
                security=powershell('Get-NetFirewallRule -Name '+literal(identity.record['rule'])+' | Get-NetFirewallSecurityFilter | Select-Object LocalUser,Authentication | ConvertTo-Json -Compress')
                report['effective_rule']=rule
                report['effective_security']=json.loads(security)
                if identity.sid not in security: raise RuntimeError('effective rule lost identity condition')
                with urllib.request.urlopen('https://api.github.com/rate_limit',timeout=10) as response:
                    report['controller_https_status']=response.status
                def controller():
                    while not stop.is_set():
                        try:
                            with socket.create_connection((ip,443),timeout=3): pass
                            samples.append({'connected':True,'at':time.monotonic()})
                        except OSError as error: samples.append({'connected':False,'error':str(error)})
                        stop.wait(.25)
                monitor=threading.Thread(target=controller)
                monitor.start()
                try:
                    for iteration in range(3):
                        result=identity.run([str(executable),ip,'deny','2'])
                        report['events'].append({'name':'immediate_descendants','iteration':iteration,'code':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
                        if result.returncode or result.stdout.count('connected=False') != 3: raise RuntimeError('a proof process connected or did not execute')
                    result=identity.run([sys.executable,'-I','-c',"from pathlib import Path; import sys\ntry: Path(sys.argv[1]).read_bytes()\nexcept PermissionError: print('DENIED')\nelse: raise SystemExit(12)",str(denied/'sentinel')])
                    report['events'].append({'name':'file_denial','code':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
                    if result.returncode or 'DENIED' not in result.stdout: raise RuntimeError('checkout/cache ACL deny failed')
                finally:
                    stop.set(); monitor.join(timeout=5)
                if len(samples)<2 or not all(s['connected'] for s in samples): raise RuntimeError('controller connectivity was interrupted')
                identity.close(); identity=None
                report['acl_after']=powershell('(Get-Acl '+literal(denied)+').Sddl')
                if report['acl_after'] != original_acl: raise RuntimeError('ACL recovery mismatch')
                if powershell('(Get-Acl '+literal(denied/'sentinel')+').Sddl') != original_child_acl: raise RuntimeError('descendant ACL recovery mismatch')
                with socket.create_connection((ip,443),timeout=5): pass
                report['events'].append({'name':'normal_cleanup','status':'passed'})
                # Recovery from a real ACL-only setup state, before rule creation.
                partial=Identity(scratch,[denied]); identity=partial
                partial.setup()
                if not recover(partial.journal): raise RuntimeError('partial journal missing')
                shutil.rmtree(partial.control); identity=None
                if powershell('(Get-Acl '+literal(denied)+').Sddl') != original_acl: raise RuntimeError('partial recovery ACL mismatch')
                report['events'].append({'name':'acl_only_recovery','status':'passed'})
                # An abruptly terminated controller owns the only job handles.
                # The independent parent then recovers its persistent policy.
                crash=subprocess.run([sys.executable,__file__,'--crash-worker',str(scratch),str(denied),ip],timeout=40)
                if crash.returncode != 91: raise RuntimeError('forced worker failure was not observed')
                leaf=int((scratch/'leaf-ready').read_text())
                control_root=Path(os.environ['SC_WINDOWS_IDENTITY_CONTROL_ROOT'])
                journals=list(control_root.glob('windows-identity-control-*/identity-recovery.json'))
                if len(journals)!=1: raise RuntimeError('crashed worker recovery journal missing')
                recover(journals[0])
                shutil.rmtree(journals[0].parent)
                if powershell('Get-CimInstance Win32_Process -Filter '+literal('ProcessId = '+str(leaf))+' | Select-Object -ExpandProperty ProcessId'):
                    raise RuntimeError('grandchild survived worker crash and recovery')
                if powershell('(Get-Acl '+literal(denied)+').Sddl') != original_acl: raise RuntimeError('crash recovery ACL mismatch')
                report['events'].append({'name':'worker_crash_live_grandchild_recovery','status':'passed'})
                # Kill the controller at the suspended-create/job-assignment gap.
                gap=subprocess.run([sys.executable,__file__,'--creation-gap-worker',str(scratch),str(denied),ip],timeout=40)
                if gap.returncode != 93: raise RuntimeError('creation-gap crash was not observed')
                journals=list(control_root.glob('windows-identity-control-*/identity-recovery.json'))
                if len(journals)!=1: raise RuntimeError('creation-gap recovery journal missing')
                record=json.loads(journals[0].read_text())
                owned_script="@(Get-CimInstance Win32_Process | Where-Object {(Invoke-CimMethod -InputObject $_ -MethodName GetOwnerSid -ErrorAction SilentlyContinue).Sid -eq "+literal(record['sid'])+"}).Count"
                if int(powershell(owned_script)) != 1: raise RuntimeError('suspended root missing from creation-gap proof')
                recover(journals[0]); shutil.rmtree(journals[0].parent)
                if int(powershell(owned_script)): raise RuntimeError('suspended root survived creation-gap recovery')
                if powershell('(Get-Acl '+literal(denied)+').Sddl') != original_acl: raise RuntimeError('creation-gap ACL mismatch')
                report['events'].append({'name':'suspended_creation_gap_recovery','status':'passed'})
                # Exercise the actual build caller and pipe-launch contract before
                # starting any expensive immutable-artifact matrix.
                from _python_sandbox import Sandbox, registered_checkouts
                sandbox=Sandbox(scratch,registered_checkouts(Path.cwd()))
                tools=[Path(sandbox.cargo).resolve().parent.parent,Path(sandbox.cargo).resolve()]
                tool_acls=[powershell('(Get-Acl -LiteralPath '+literal(path)+').Sddl') for path in tools]
                with sandbox:
                    report['production_denials']=sandbox.prove_denials(sys.executable,Path.cwd())
                    sandbox.run([sandbox.cargo,'--version'],scratch)
                    sandbox.run([sandbox.rustc,'--version'],scratch)
                    hello=scratch/'hello.rs'
                    hello.write_text('fn main() { println!("NATIVE_TOOLCHAIN_OK"); }')
                    sandbox.run([sandbox.rustc,str(hello),'-o',str(scratch/'hello.exe')],scratch)
                    if 'NATIVE_TOOLCHAIN_OK' not in sandbox.run([str(scratch/'hello.exe')],scratch):
                        raise RuntimeError('isolated native toolchain probe failed')
                    process=sandbox.spawn([sys.executable,'-I','-c','print("PIPE_LAUNCH_OK",flush=True)'],scratch,stdout=subprocess.PIPE)
                    try:
                        data=process.stdout.read()
                        if process.wait(timeout=10) or b'PIPE_LAUNCH_OK' not in data:
                            raise RuntimeError('identity pipe-launch probe failed')
                    finally:
                        process.close(); process.stdout.close()
                restored=[powershell('(Get-Acl -LiteralPath '+literal(path)+').Sddl') for path in tools]
                if restored != tool_acls: raise RuntimeError('provisioned toolchain ACL recovery mismatch')
                report['toolchain_acl_restoration']={'paths':list(map(str,tools)),'before':tool_acls,'after':restored}
                report['events'].append({'name':'production_sandbox_native_and_pipe','status':'passed'})
            finally:
                if identity is not None:
                    identity.close(); identity=None
                shutil.rmtree(denied)
        after=snapshot()
        if before != after: raise RuntimeError('firewall/account snapshot changed after recovery')
        report['snapshot_sha256']=hashlib.sha256(json.dumps(before,sort_keys=True).encode()).hexdigest()
        report['status']='passed'
    except BaseException as error:
        report['error']=f'{type(error).__name__}: {error}'
        raise
    finally:
        stop.set()
        report['controller_samples']=samples
        (args.output/'preflight.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
        print(json.dumps(report),flush=True)



def creation_gap_worker():
    scratch,denied,ip=Path(sys.argv[2]),Path(sys.argv[3]),sys.argv[4]
    identity=Identity(scratch,[denied]); identity.setup(); identity.activate()
    identity.api.kernel.AssignProcessToJobObject=lambda *_: os._exit(93)
    identity.run([str(scratch/'probe-2.exe'),ip,'deny','2'])
    raise RuntimeError('creation gap injection was not reached')


def crash_worker():
    scratch,denied,ip=Path(sys.argv[2]),Path(sys.argv[3]),sys.argv[4]
    identity=Identity(scratch,[denied]); identity.setup(); identity.activate()
    def interrupt():
        deadline=time.monotonic()+20
        while not (scratch/'leaf-ready').exists():
            if time.monotonic()>deadline: os._exit(92)
            time.sleep(.01)
        os._exit(91)
    threading.Thread(target=interrupt,daemon=True).start()
    identity.run([str(scratch/'probe-2.exe'),ip,'denyhold','2'],timeout=35)
    raise RuntimeError('crash worker unexpectedly returned')


def supervise():
    output=Path('target/windows-identity-preflight')
    output.mkdir(parents=True,exist_ok=True)
    result=125
    recovered=[]
    recovery_errors=[]
    with tempfile.TemporaryDirectory(prefix='windows-preflight-control-') as temporary:
        environment=dict(os.environ,SC_WINDOWS_IDENTITY_CONTROL_ROOT=temporary)
        process=subprocess.Popen([sys.executable,__file__,'--worker',*sys.argv[1:]],env=environment)
        try:
            result=process.wait(timeout=240)
        except subprocess.TimeoutExpired:
            subprocess.run(['taskkill','/PID',str(process.pid),'/T','/F'],timeout=20,check=False)
            process.wait(timeout=10)
            result=124
        finally:
            for journal in Path(temporary).glob('windows-identity-control-*/identity-recovery.json'):
                try:
                    recover(journal)
                    recovered.append(str(journal))
                except Exception as error:
                    recovery_errors.append(str(error))
                    shutil.copytree(journal.parent,output/('failed-recovery-'+str(len(recovery_errors))),dirs_exist_ok=True)
                finally:
                    result=result or 125
            report_path=output/'preflight.json'
            if result and report_path.exists():
                report=json.loads(report_path.read_text(encoding='utf-8'))
                report.update(status='failed',supervisor_exit=result)
                report_path.write_text(json.dumps(report,indent=2),encoding='utf-8')
            (output/'supervisor.json').write_text(json.dumps({'exit':result,'recovered':recovered,'recovery_errors':recovery_errors},indent=2),encoding='utf-8')
    raise SystemExit(result)


if __name__=='__main__':
    if '--creation-gap-worker' in sys.argv: creation_gap_worker()
    elif '--crash-worker' in sys.argv: crash_worker()
    elif '--worker' in sys.argv:
        sys.argv.remove('--worker'); main()
    else: supervise()
