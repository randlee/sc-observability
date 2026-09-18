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
                if powershell('(Get-Acl '+literal(denied)+').Sddl') != original_acl: raise RuntimeError('ACL recovery mismatch')
                with socket.create_connection((ip,443),timeout=5): pass
                report['events'].append({'name':'normal_cleanup','status':'passed'})
                # Recovery from a real ACL-only setup state, before rule creation.
                partial=Identity(scratch,[denied]); identity=partial
                partial.setup()
                if not recover(partial.journal): raise RuntimeError('partial journal missing')
                shutil.rmtree(partial.control); identity=None
                if powershell('(Get-Acl '+literal(denied)+').Sddl') != original_acl: raise RuntimeError('partial recovery ACL mismatch')
                report['events'].append({'name':'acl_only_recovery','status':'passed'})
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


if __name__=='__main__': main()
