#!/usr/bin/env python3
"""Execute actual bundled DTO conversions with network/checkouts/cache denied."""
from __future__ import annotations
import argparse
import io
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from build_binding_source_bundle import BundleError,verify_bundle,digest
ROOT=Path(__file__).resolve().parents[2]

def isolated_prefix(directory,checkout_roots):
    if platform.system()=='Darwin':
        profile=directory/'isolation.sb'
        denies='\n'.join(f'(deny file-read* (subpath {json.dumps(str(root))}))' for root in checkout_roots)
        profile.write_text('(version 1)\n(allow default)\n(deny network*)\n'+denies+'\n')
        return ['/usr/bin/sandbox-exec','-f',str(profile)]
    if platform.system()=='Linux' and shutil.which('bwrap'):
        prefix=['bwrap','--die-with-parent','--unshare-net','--ro-bind','/','/','--dev-bind','/dev','/dev','--proc','/proc','--bind',str(directory),str(directory)]
        for root in checkout_roots:
            if root.exists():prefix+=['--tmpfs',str(root)]
        return prefix+['--']
    raise RuntimeError('BUNDLE_ISOLATION_UNAVAILABLE: sandbox-exec or bwrap required; gate cannot be skipped')

def main():
    p=argparse.ArgumentParser();p.add_argument('--bundle',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);p.add_argument('--consumer-source',type=Path,default=ROOT/'scripts/ci/fixtures/binding-consumer/main.rs');p.add_argument('--expected-marker',default='BINDING_CONSUMER_OK');args=p.parse_args()
    manifest=verify_bundle(args.bundle)
    with tempfile.TemporaryDirectory(prefix='binding-isolated-') as temporary:
        external=Path(temporary).resolve();artifact=external/'artifact';shutil.copytree(args.bundle,artifact)
        verify_bundle(artifact)
        (artifact/'src/main.rs').write_bytes(args.consumer_source.read_bytes())
        cargo=Path(subprocess.check_output(['rustup','which','cargo'],text=True).strip());rustc=Path(subprocess.check_output(['rustup','which','rustc'],text=True).strip())
        if '1.94.1' not in subprocess.check_output([str(rustc),'--version'],text=True):raise RuntimeError('requires Rust 1.94.1')
        home=Path.home();deny={ROOT,home/'.cargo'}
        common=Path(subprocess.check_output(['git','rev-parse','--git-common-dir'],cwd=ROOT,text=True).strip()).resolve()
        deny.add(common.parent)
        # Every registered worktree is denied, not merely the initiating checkout.
        for line in subprocess.check_output(['git','worktree','list','--porcelain'],cwd=ROOT,text=True).splitlines():
            if line.startswith('worktree '):deny.add(Path(line[9:]).resolve())
        prefix=isolated_prefix(external,sorted(deny))
        env={k:v for k,v in os.environ.items() if not k.startswith(('CARGO_','RUST'))}
        (external/'tmp').mkdir()
        env['TMPDIR']=str(external/'tmp')
        env.update(CARGO_HOME=str(external/'cargo-home'),CARGO_TARGET_DIR=str(external/'target'),RUSTC=str(rustc),RUSTDOC=str(cargo.with_name('rustdoc')),PATH=str(cargo.parent)+os.pathsep+os.environ['PATH'])
        commands=[]
        def execute(command):
            result=subprocess.run(prefix+command,cwd=artifact,env=env,text=True,capture_output=True)
            commands.append({'command':command,'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
            if result.returncode:raise RuntimeError(f'isolated command failed: {command}\n{result.stderr}')
            return result.stdout
        # Prove the policy denies source/cache reads and network, independently of cargo offline mode.
        probes={}
        for name,command in [('checkout',['/bin/cat',str(ROOT/'Cargo.toml')]),('cache',['/bin/ls',str(home/'.cargo/registry')]),('network',['/usr/bin/curl','--connect-timeout','2','https://index.crates.io/config.json'])]:
            probe=subprocess.run(prefix+command,cwd=artifact,env=env,text=True,capture_output=True)
            if probe.returncode==0:raise RuntimeError(f'isolation probe unexpectedly succeeded: {name}')
            probes[name]={'denied':True,'exit_code':probe.returncode}
        metadata=json.loads(execute([str(cargo),'metadata','--locked','--offline','--format-version','1']))
        for package in metadata['packages']:
            if not Path(package['manifest_path']).resolve().is_relative_to(artifact):raise RuntimeError(f'ambient dependency: {package["manifest_path"]}')
        output=execute([str(cargo),'run','--locked','--offline'])
        if args.expected_marker not in output:raise RuntimeError('missing conversion proof')
        negatives={}
        def negative(name,mutate,expected):
            copy=external/name;shutil.copytree(artifact,copy);mutate(copy)
            try:verify_bundle(copy)
            except BundleError as error:
                if error.code!=expected:raise RuntimeError(f'{name}: expected {expected}, got {error}') from error
                negatives[name]=error.code
            else:raise RuntimeError(f'{name}: negative accepted')
        negative('missing-member',lambda root:shutil.rmtree(root/manifest['packages'][0]['root']),'BUNDLE_MISSING_MEMBER')
        negative('stale-lock',lambda root:(root/'Cargo.lock').write_text((root/'Cargo.lock').read_text()+'\n# stale\n'),'BUNDLE_STALE_LOCK')
        negative('escaping-manifest',lambda root:(root/'Cargo.toml').write_text((root/'Cargo.toml').read_text()+'\n[dependencies.escape]\npath = "../../outside"\n'),'BUNDLE_ESCAPING_PATH')
        def escaping_archive(root):
            with tarfile.open(root/manifest['packages'][0]['archive'],'w:gz') as stream:
                info=tarfile.TarInfo('../outside');info.size=1;stream.addfile(info,io.BytesIO(b'x'))
        negative('escaping-archive',escaping_archive,'BUNDLE_ESCAPING_PATH')
        def changed_registry_selection(root):
            # Update integrity hashes deliberately: equivalence must catch selection
            # drift independently of the generic frozen-file checksum gate.
            lock=root/'Cargo.lock';text=lock.read_text();package=manifest['registry_selection'][0]
            old=f'name = "{package["name"]}"\nversion = "{package["version"]}"'
            text=text.replace(old,f'name = "{package["name"]}"\nversion = "999.0.0"',1);lock.write_text(text)
            record=json.loads((root/'manifest.json').read_text());record['files']['Cargo.lock']=digest(lock);record['lock_sha256']=digest(lock);(root/'manifest.json').write_text(json.dumps(record))
        negative('changed-registry-selection',changed_registry_selection,'BUNDLE_REGISTRY_DRIFT')
        def changed_requirement(root):
            entry=manifest['packages'][0];path=root/entry['root']/'Cargo.toml';text=path.read_text()
            key,spec=next((k,v) for k,v in entry['reviewed_requirements'].items() if k.startswith('/dependencies/'));alias=key.rsplit('/',1)[-1];start=text.index(f'[dependencies.{alias}]');old=f'version = "{spec["version"]}"';text=text[:start]+text[start:].replace(old,'version = "999.0.0"',1);path.write_text(text)
            record=json.loads((root/'manifest.json').read_text());record['files'][path.relative_to(root).as_posix()]=digest(path);(root/'manifest.json').write_text(json.dumps(record))
        negative('changed-normalized-requirement',changed_requirement,'BUNDLE_REQUIREMENT_DRIFT')
        evidence={'status':'passed','source_commit':manifest['source_commit'],'bundle_manifest_sha256':digest(args.bundle/'manifest.json'),'archives':{p['name']:p['archive_sha256'] for p in manifest['packages']},'registry_selection':manifest['registry_selection'],'reviewed_requirements':{p['name']:p['reviewed_requirements'] for p in manifest['packages']},'lock_sha256':manifest['lock_sha256'],'dependency_provenance':[{k:p[k] for k in ('name','version','source')} for p in metadata['packages']],'isolation_probes':probes,'sandbox_prefix':prefix,'denied_roots':[str(p) for p in sorted(deny)],'sandbox_policy':(external/'isolation.sb').read_text() if (external/'isolation.sb').exists() else None,'cargo_home':env['CARGO_HOME'],'target_dir':env['CARGO_TARGET_DIR'],'negative_results':negatives,'consumer_output':output.strip(),'commands':commands,'publication':'pending_B.7'}
        args.evidence.parent.mkdir(parents=True,exist_ok=True);args.evidence.write_text(json.dumps(evidence,sort_keys=True,indent=2)+'\n')
    print('BUNDLE_ISOLATED_CONSUMER_PASSED: positive and six exact-code negatives')
if __name__=='__main__':main()
