#!/usr/bin/env python3
"""Build and verify self-contained prepublication Cargo source bundles."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path,PurePosixPath

class BundleError(ValueError):
    def __init__(self,code,message):super().__init__(f'{code}: {message}');self.code=code

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def safe(root,relative):
    parts=PurePosixPath(relative)
    if parts.is_absolute() or '..' in parts.parts or '\\' in relative or ':' in relative:raise BundleError('BUNDLE_ESCAPING_PATH',relative)
    result=(root/relative).resolve()
    if not result.is_relative_to(root.resolve()):raise BundleError('BUNDLE_ESCAPING_PATH',relative)
    return result

def archive_members(archive):
    with tarfile.open(archive,'r:gz') as stream:
        members=stream.getmembers();seen=set()
        for member in members:
            safe(Path('/bundle'),member.name)
            if not member.isfile() or member.name in seen:raise BundleError('BUNDLE_ESCAPING_PATH',f'nonregular/duplicate archive member: {member.name}')
            seen.add(member.name)
        return members

def manifests_confined(root):
    for manifest in root.rglob('Cargo.toml'):
        document=tomllib.loads(manifest.read_text())
        def walk(value):
            if isinstance(value,dict):
                if 'path' in value and isinstance(value['path'],str):
                    target=(manifest.parent/value['path']).resolve()
                    if not target.is_relative_to(root.resolve()):raise BundleError('BUNDLE_ESCAPING_PATH',f'{manifest.relative_to(root)}: {value["path"]}')
                if 'git' in value:raise BundleError('BUNDLE_ESCAPING_PATH',f'git dependency in {manifest}')
                for child in value.values():walk(child)
            elif isinstance(value,list):
                for child in value:walk(child)
        walk(document)

def verify_bundle(root):
    root=root.resolve()
    try:manifest=json.loads((root/'manifest.json').read_text())
    except FileNotFoundError as exc:raise BundleError('BUNDLE_MISSING_MEMBER','manifest.json') from exc
    if manifest.get('schema_version')!=1:raise BundleError('BUNDLE_INVALID_MANIFEST','unsupported bundle version')
    # Reject escaping paths before following any archive, manifest or digest pointer.
    for relative in manifest['files']:safe(root,relative)
    for entry in manifest['packages']:
        archive=safe(root,entry['archive'])
        if not archive.is_file():raise BundleError('BUNDLE_MISSING_MEMBER',entry['archive'])
        archive_members(archive)
    for path in root.rglob("*"):
        if path.is_symlink():raise BundleError("BUNDLE_ESCAPING_PATH",str(path.relative_to(root)))
    manifests_confined(root)
    for relative,expected in manifest['files'].items():
        file=safe(root,relative)
        if not file.is_file():raise BundleError('BUNDLE_MISSING_MEMBER',relative)
        if digest(file)!=expected:raise BundleError('BUNDLE_STALE_LOCK' if relative=='Cargo.lock' else 'BUNDLE_CHECKSUM_MISMATCH',relative)
    return manifest

def command(arguments,cwd,**kwargs):
    result=subprocess.run(arguments,cwd=cwd,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE,**kwargs)
    if result.returncode:raise BundleError('BUNDLE_COMMAND_FAILED',f'{arguments!r}\n{result.stderr}')
    return result.stdout

def build(root_manifest,output):
    root_manifest=root_manifest.resolve();output=output.resolve()
    if output.exists():raise BundleError('BUNDLE_OUTPUT_EXISTS',str(output))
    source_root = next((p for p in [root_manifest.parent, *root_manifest.parent.parents] if (p/'Cargo.toml').is_file() and 'workspace' in tomllib.loads((p/'Cargo.toml').read_text())), root_manifest.parent)
    seen=set()
    workspace=tomllib.loads((source_root/'Cargo.toml').read_text()).get('workspace',{})
    def preflight(manifest):
        if manifest in seen:return
        seen.add(manifest)
        if not manifest.resolve().is_relative_to(source_root):raise BundleError('BUNDLE_ESCAPING_PATH',str(manifest))
        document=tomllib.loads(manifest.read_text())
        tables=[document,*document.get('target',{}).values()]
        for table in tables:
            for section in ('dependencies','build-dependencies','dev-dependencies'):
                for name,spec in table.get(section,{}).items():
                    if not isinstance(spec,dict):continue
                    base=manifest.parent
                    if spec.get('workspace'):
                        spec=workspace.get('dependencies',{}).get(name,{})
                        base=source_root
                    if not isinstance(spec,dict):continue
                    if 'git' in spec:raise BundleError('BUNDLE_ESCAPING_PATH','git dependency is not bundled')
                    if 'path' in spec:
                        child=(base/spec['path']/'Cargo.toml').resolve()
                        if not child.is_relative_to(source_root):raise BundleError('BUNDLE_ESCAPING_PATH',str(child))
                        if not child.is_file():raise BundleError('BUNDLE_MISSING_MEMBER',str(child))
                        preflight(child)
    preflight(root_manifest)
    # --locked rejects a stale source lock before any package staging.
    try:metadata=json.loads(command(['cargo','metadata','--locked','--format-version','1','--manifest-path',str(root_manifest)],root_manifest.parent))
    except BundleError as exc:raise BundleError('BUNDLE_STALE_LOCK',str(exc)) from exc
    source=Path(metadata['workspace_root']).resolve()
    by_id={p['id']:p for p in metadata['packages']}
    candidates=[p for p in by_id.values() if Path(p['manifest_path']).resolve()==root_manifest]
    if len(candidates)!=1:raise BundleError('BUNDLE_INVALID_MANIFEST','root package identity is ambiguous')
    root=candidates[0];nodes={n['id']:n for n in metadata['resolve']['nodes']}
    closure=set()
    def visit(pid):
        if pid in closure:return
        closure.add(pid)
        for dependency in nodes[pid]['deps']:visit(dependency['pkg'])
        # Cargo's active resolve graph omits disabled optional first-party edges.
        # Bundles must also support the root's later feature/target selections.
        for dependency in by_id[pid]['dependencies']:
            if dependency.get('path'):
                target=(Path(dependency['path'])/'Cargo.toml').resolve()
                if not target.is_relative_to(source):raise BundleError('BUNDLE_ESCAPING_PATH',str(target))
                matches=[p['id'] for p in by_id.values() if Path(p['manifest_path']).resolve()==target]
                if len(matches)!=1:raise BundleError('BUNDLE_MISSING_MEMBER',str(target))
                visit(matches[0])
    visit(root['id'])
    unpublished=sorted((by_id[pid] for pid in closure if by_id[pid]['source'] is None),key=lambda p:p['name'])
    for package in unpublished:
        if not Path(package['manifest_path']).resolve().is_relative_to(source):raise BundleError('BUNDLE_ESCAPING_PATH',package['manifest_path'])
        if package.get('publish')==[]:raise BundleError('BUNDLE_INVALID_MANIFEST',f'private package in production closure: {package["name"]}')
    output.mkdir(parents=True)
    build_target=output/'package-build'
    args=['cargo','package','--locked','--allow-dirty','--no-verify','--target-dir',str(build_target)]
    for package in unpublished:args+=['-p',package['name']]
    package_log=command(args,source);(output/'package.log').write_text(package_log)
    archives=output/'archives';archives.mkdir();packages_dir=output/'packages';packages_dir.mkdir()
    entries=[]
    qualified_stage=source/'docs/plans/phase-b/evidence/b2-final/stage'
    qualified={}
    qualified_evidence=None
    if qualified_stage.exists():
        from _log_staging import verify_stage
        qualified_evidence=verify_stage(qualified_stage,root['version'])
        qualified={p['name']:p for p in qualified_evidence['packages']}
    for package in unpublished:
        stem=f'{package["name"]}-{package["version"]}'
        archive=archives/f'{stem}.crate'
        if package['name'] in qualified:
            staged=qualified[package['name']]
            shutil.copyfile(qualified_stage/staged['archive'],archive)
        else:shutil.copyfile(build_target/'package'/archive.name,archive)
        members=archive_members(archive)
        if any(not m.name.startswith(stem+'/') for m in members):raise BundleError('BUNDLE_ESCAPING_PATH','archive root identity mismatch')
        with tarfile.open(archive,'r:gz') as stream:stream.extractall(packages_dir,filter='data')
        normalized=tomllib.loads((packages_dir/stem/'Cargo.toml').read_text())
        if normalized['package']['name']!=package['name'] or normalized['package']['version']!=package['version']:raise BundleError('BUNDLE_INVALID_MANIFEST','archive package identity mismatch')
        entries.append({'name':package['name'],'version':package['version'],'archive':archive.relative_to(output).as_posix(),'archive_sha256':digest(archive),'root':f'packages/{stem}','provenance':'qualified-B.2-archive' if package['name'] in qualified else 'unpublished-cargo-package','qualified_source_commit':qualified_evidence['source_commit'] if package['name'] in qualified else None})
    shutil.rmtree(build_target)
    patches='\n'.join(f'{p["name"]} = {{ path = "{p["root"]}" }}' for p in entries)
    dependencies='\n'.join(f'{p["name"]} = "={p["version"]}"' for p in entries)
    members=json.dumps([p['root'] for p in entries])
    (output/'Cargo.toml').write_text('[package]\nname = "binding-source-consumer"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n\n[workspace]\nmembers = '+members+'\n\n[dependencies]\n'+dependencies+'\nserde_json = "1"\n\n[patch.crates-io]\n'+patches+'\n')
    (output/'src').mkdir();(output/'src/main.rs').write_text('fn main() { println!("binding source bundle ready"); }\n')
    # Resolve the actual staged layout, then vendor the complete registry closure.
    command(['cargo','generate-lockfile'],output)
    vendor_config=command(['cargo','vendor','--locked','vendor'],output)
    (output/'.cargo').mkdir();(output/'.cargo/config.toml').write_text(vendor_config)
    command(['cargo','metadata','--locked','--offline','--format-version','1'],output)
    manifests_confined(output)
    files={p.relative_to(output).as_posix():digest(p) for p in sorted(output.rglob('*')) if p.is_file() and p.relative_to(output).parts[0] not in ('src','target')}
    source_sha=command(['git','rev-parse','HEAD'],source).strip()
    evidence={'schema_version':1,'source_commit':source_sha,'root_package':root['name'],'root_version':root['version'],'publication':'pending_B.7','packages':entries,'files':files,'lock_sha256':digest(output/'Cargo.lock'),'source_lock_sha256':digest(source/'Cargo.lock'),'package_command':args[:args.index('--target-dir')]+['--target-dir','<bundle>/package-build']+args[args.index('--target-dir')+2:]}
    (output/'manifest.json').write_text(json.dumps(evidence,indent=2,sort_keys=True)+'\n')
    verify_bundle(output)
    return evidence

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root-manifest',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--verify',action='store_true')
    args=parser.parse_args()
    try:
        if args.verify:verify_bundle(args.output)
        elif args.root_manifest:build(args.root_manifest,args.output)
        else:parser.error('--root-manifest required when building')
    except (BundleError,OSError,ValueError,tarfile.TarError) as exc:raise SystemExit(str(exc))
    print('BUNDLE_VERIFIED: '+str(args.output))
if __name__=='__main__':main()
