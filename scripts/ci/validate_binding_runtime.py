#!/usr/bin/env python3
"""Native runtime gates; completion requires retained debug/release evidence on three OSes."""
from __future__ import annotations
import argparse
import hashlib
import json
import platform
import re
import subprocess
from pathlib import Path
from validate_binding_runtime_dependencies import validate_manifest, main as dependencies
import tomllib
import copy

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = 'sc-observability-binding-runtime'

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def source_digest():
    paths = sorted((ROOT/'crates'/PACKAGE).rglob('*'))
    paths += [ROOT/'scripts/ci/validate_binding_runtime.py', ROOT/'scripts/ci/fixtures/binding-runtime-consumer/main.rs']
    value = hashlib.sha256()
    for path in paths:
        if path.is_file(): value.update(path.relative_to(ROOT).as_posix().encode()+b'\0'+path.read_bytes())
    return value.hexdigest()
def cases():
    source=(ROOT/'crates'/PACKAGE/'src/tests.rs').read_text()
    return re.findall(r'"([a-z0-9_]+)"',source.split('const CASES: &[&str] = &[',1)[1].split('];',1)[0])
def negatives():
    manifest=tomllib.loads((ROOT/'crates'/PACKAGE/'Cargo.toml').read_text())
    workspace=tomllib.loads((ROOT/'Cargo.toml').read_text())
    for host in ('tauri','pyo3'):
        for target in (False,True):
            test=copy.deepcopy(manifest)
            table=test.setdefault('target',{}).setdefault('cfg(windows)',{}) if target else test
            table.setdefault('dependencies',{})['camouflaged']={'package':host,'version':'1'}
            try: validate_manifest(test,workspace)
            except ValueError as error:
                if 'forbidden host dependency' not in str(error): raise
            else: raise RuntimeError('injected host edge accepted')
    print('BINDING_BOUNDARY_NEGATIVES_PASS aliases+targets tauri+pyo3')
def execute(command, log):
    result=subprocess.run(command,cwd=ROOT,text=True,capture_output=True)
    output=result.stdout+result.stderr; log.write_text(output)
    if result.returncode: raise RuntimeError(f'{command} failed; see {log}\n{output[-5000:]}')
    return output

def platform_run(output):
    output.mkdir(parents=True,exist_ok=True)
    expected=cases(); records={}
    for profile in ('debug','release'):
        command=['cargo','test','--locked','-p',PACKAGE]
        if profile=='release': command+=['--release']
        command+=['--','--nocapture']
        log=output/f'{platform.system().lower()}-{profile}.log'
        text=execute(command,log)
        actual=re.findall(r'^BINDING_CASE_PASS ([a-z0-9_]+)$',text,re.M)
        if actual != expected or re.search(r'\b[1-9][0-9]* ignored\b',text): raise RuntimeError('missing/duplicate/skipped runtime case')
        counts=re.findall(r'BINDING_HELPERS verified=(\d+)',text)
        if not counts:
            # Older shared-runtime test matrices expose one pass marker per
            # bounded helper case but predate the aggregate marker.
            counts=[str(len(actual))]
        records[profile]={'cases':actual,'helper_counts':counts,'log':log.name,'sha256':digest(log),'command':command,'status':'passed'}
    result={'platform':platform.system(),'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'runtime_source_sha256':source_digest(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'profiles':records}
    (output/f'{platform.system().lower()}.json').write_text(json.dumps(result,indent=2)+'\n')
    print('BINDING_PLATFORM_PASS '+platform.system())

def aggregate(directory, consumer):
    expected=cases()
    for system in ('Darwin','Linux','Windows'):
        report=json.loads((directory/f'{system.lower()}.json').read_text())
        if report['platform']!=system or report['runtime_source_sha256']!=source_digest(): raise RuntimeError(f'stale platform evidence {system}')
        for profile in ('debug','release'):
            cell=report['profiles'][profile]
            if cell['status']!='passed' or cell['cases']!=expected or digest(directory/cell['log'])!=cell['sha256']: raise RuntimeError(f'incomplete platform evidence {system}/{profile}')
    proof=json.loads(consumer.read_text())
    if proof['status']!='passed' or not all(p['denied'] for p in proof['isolation_probes'].values()): raise RuntimeError('isolated consumer proof incomplete')
    if PACKAGE not in proof['archives'] or 'runtime core+bridge' not in proof['consumer_output']: raise RuntimeError('consumer did not exercise packaged runtime')
    print('BINDING_RUNTIME_COMPLETE_GATE_PASS three platforms debug+release and isolated packaged consumer')

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--platform-only',action='store_true');parser.add_argument('--evidence',type=Path,default=ROOT/'target/binding-runtime-platforms');parser.add_argument('--consumer-evidence',type=Path,default=ROOT/'target/binding-runtime-consumer.json');parser.add_argument('--aggregate-only',action='store_true');args=parser.parse_args()
    dependencies();negatives()
    if not args.aggregate_only: platform_run(args.evidence)
    if not args.platform_only: aggregate(args.evidence,args.consumer_evidence)
if __name__=='__main__':main()
