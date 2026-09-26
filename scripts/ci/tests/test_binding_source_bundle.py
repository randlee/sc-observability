"""Boundary tests invoking the real helper before Cargo can follow bad paths."""
import json
import io
import subprocess
import sys
import tempfile
import tarfile
import unittest
from pathlib import Path
import importlib.util
from unittest import mock
ROOT=Path(__file__).resolve().parents[3]
HELPER_DIR=ROOT/'scripts/ci'
sys.path.insert(0,str(HELPER_DIR))
HELPER=ROOT/'scripts/ci/build_binding_source_bundle.py'
SPEC=importlib.util.spec_from_file_location('binding_source_bundle', HELPER)
BUNDLE=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(BUNDLE)
from _log_staging import PACKAGES, inspect_archive, sha256

class SourceBoundaryTests(unittest.TestCase):
    def command(self,root,*args):
        return subprocess.run(args,cwd=root,check=True,capture_output=True,text=True)

    def staged_archive(self,stage,name,version,source_commit):
        archive=stage/'archives'/f'{name}-{version}.crate';prefix=f'{name}-{version}'
        manifest='[package]\nname='+json.dumps(name)+'\nversion='+json.dumps(version)+'\nedition="2024"\nlicense="MIT"\n'
        if name=='sc-observability-types':manifest+='[dependencies]\nserde_json="1"\n'
        if name=='sc-observability-log':manifest+='[dependencies]\nsc-observability-log-macros={version='+json.dumps(f'={version}')+'}\n'
        files={f'{prefix}/Cargo.toml':manifest.encode(),f'{prefix}/LICENSE':b'MIT\n',f'{prefix}/src/lib.rs':b'pub fn fixture() {}\n',f'{prefix}/.cargo_vcs_info.json':json.dumps({'git':{'sha1':source_commit,'dirty':False}}).encode()}
        with tarfile.open(archive,'w:gz') as contents:
            for path,body in files.items():
                member=tarfile.TarInfo(path);member.size=len(body)
                contents.addfile(member,io.BytesIO(body))
        return archive

    def project_with_stage(self,root,root_version,stage_version,*,corrupt=False,missing_candidate=False):
        package=root/'crates/sc-observability-types';(package/'src').mkdir(parents=True)
        (root/'Cargo.toml').write_text('[workspace]\nmembers=["crates/sc-observability-types"]\nresolver="2"\n')
        (package/'Cargo.toml').write_text('[package]\nname="sc-observability-types"\nversion='+json.dumps(root_version)+'\nedition="2024"\nlicense="MIT"\n[dependencies]\nserde_json="1"\n')
        (package/'src/lib.rs').write_text('pub fn fixture() {}\n')
        (package/'LICENSE').write_text('MIT\n')
        self.command(root,'cargo','generate-lockfile');self.command(root,'git','init','-q');self.command(root,'git','add','.')
        self.command(root,'git','-c','user.name=Bundle Fixture','-c','user.email=bundle-fixture@example.invalid','commit','-qm','reviewed fixture')
        source_commit=self.command(root,'git','rev-parse','HEAD').stdout.strip()
        stage=root/'docs/plans/phase-b/evidence/b2-final/stage';(stage/'archives').mkdir(parents=True)
        packages=[]
        for name in PACKAGES:
            archive=self.staged_archive(stage,name,stage_version,source_commit)
            details=inspect_archive(archive,name,stage_version,source_commit)
            packages.append({'name':name,'version':stage_version,'archive':archive.relative_to(stage).as_posix(),'archive_sha256':sha256(archive),**details})
        manifest={'schema_version':1,'candidate_version':stage_version,'publication':'pending_B.7','source_commit':source_commit,'packages':packages}
        if missing_candidate:manifest.pop('candidate_version')
        if corrupt:manifest['packages'][0]['archive_sha256']='0'*64
        (stage/'stage-manifest.json').write_text(json.dumps(manifest))
        return package/'Cargo.toml'

    def test_build_uses_matching_qualified_stage_archives(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);manifest=self.project_with_stage(root,'1.4.0','1.4.0')
            evidence=BUNDLE.build(manifest,root/'bundle')
            self.assertEqual(evidence['packages'][0]['provenance'],'qualified-B.2-archive')
            self.assertEqual(evidence['packages'][0]['qualified_source_commit'],evidence['source_commit'])

    def test_build_uses_source_packages_for_a_verified_version_mismatch(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);manifest=self.project_with_stage(root,'2.0.0','1.4.0')
            with mock.patch('builtins.print') as output:
                evidence=BUNDLE.build(manifest,root/'bundle')
            self.assertEqual(evidence['packages'][0]['provenance'],'unpublished-cargo-package')
            self.assertIsNone(evidence['packages'][0]['qualified_source_commit'])
            output.assert_called_once_with('stage evidence 1.4.0 does not match root 2.0.0; using unpublished cargo packages')

    def test_build_rejects_a_corrupt_stage_before_version_mismatch_fallback(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);manifest=self.project_with_stage(root,'2.0.0','1.4.0',corrupt=True)
            with self.assertRaisesRegex(ValueError,'archive checksum mismatch'):
                BUNDLE.build(manifest,root/'bundle')

    def test_build_rejects_a_stage_without_candidate_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);manifest=self.project_with_stage(root,'2.0.0','1.4.0',missing_candidate=True)
            with self.assertRaisesRegex(BUNDLE.BundleError,'candidate_version must be a string'):
                BUNDLE.build(manifest,root/'bundle')

    def test_build_rejects_a_non_object_stage_manifest(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);manifest=self.project_with_stage(root,'2.0.0','1.4.0')
            stage=root/'docs/plans/phase-b/evidence/b2-final/stage/stage-manifest.json';stage.write_text('[]')
            with self.assertRaisesRegex(BUNDLE.BundleError,'stage manifest must be a JSON object'):
                BUNDLE.build(manifest,root/'bundle')

    def test_stale_qualified_archive_is_not_reused(self):
        evidence={'source_commit':'older','packages':[{'name':'binding-runtime'}]}
        self.assertEqual(BUNDLE.qualified_packages_for(evidence,'current'),{})
        self.assertEqual(BUNDLE.qualified_packages_for(evidence,'older'),{'binding-runtime': {'name':'binding-runtime'}})

    def invoke(self,root):
        return subprocess.run([sys.executable,str(HELPER),'--root-manifest',str(root/'Cargo.toml'),'--output',str(root/'bundle')],capture_output=True,text=True)
    def project(self,root,extra=''):
        (root/'src').mkdir();(root/'src/lib.rs').write_text('pub fn fixture() {}\n')
        (root/'Cargo.toml').write_text('[package]\nname="bundle-boundary-fixture"\nversion="0.1.0"\nedition="2024"\n[workspace]\n'+extra)
    def test_escaping_dependency_rejected_before_cargo(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);self.project(root,'[dependencies]\nescape={path="../outside"}\n')
            result=self.invoke(root);self.assertNotEqual(result.returncode,0);self.assertIn('BUNDLE_ESCAPING_PATH',result.stderr);self.assertFalse((root/'bundle').exists())
    def test_missing_dependency_has_exact_code(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);self.project(root,'[dependencies]\nmissing={path="missing"}\n')
            result=self.invoke(root);self.assertNotEqual(result.returncode,0);self.assertIn('BUNDLE_MISSING_MEMBER',result.stderr)
    def test_stale_source_lock_is_never_regenerated(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);self.project(root)
            lock='version = 4\n[[package]]\nname="bundle-boundary-fixture"\nversion="0.0.1"\n';(root/'Cargo.lock').write_text(lock)
            result=self.invoke(root);self.assertNotEqual(result.returncode,0);self.assertIn('BUNDLE_STALE_LOCK',result.stderr);self.assertEqual((root/'Cargo.lock').read_text(),lock)
    def test_path_only_dependency_rejected_before_staging(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);self.project(root,'[dependencies]\nlocal={path="local"}\n')
            local=root/'local';local.mkdir();(local/'Cargo.toml').write_text('[package]\nname="local"\nversion="0.1.0"\nedition="2024"\n')
            result=self.invoke(root);self.assertNotEqual(result.returncode,0);self.assertIn('BUNDLE_MISSING_VERSION',result.stderr);self.assertFalse((root/'bundle').exists())
    def test_workspace_path_only_dependency_rejected_before_staging(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);self.project(root,'[workspace.dependencies]\nlocal={path="local"}\n[dependencies]\nlocal.workspace=true\n')
            local=root/'local';local.mkdir();(local/'Cargo.toml').write_text('[package]\nname="local"\nversion="0.1.0"\nedition="2024"\n')
            result=self.invoke(root);self.assertNotEqual(result.returncode,0);self.assertIn('BUNDLE_MISSING_VERSION',result.stderr);self.assertFalse((root/'bundle').exists())
    def test_standalone_manifest_closure_preserves_workspace_inheritance(self):
        import json
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            (root/'Cargo.toml').write_text('[workspace]\nmembers=["shared"]\nexclude=["adapter"]\nresolver="2"\n[workspace.package]\nversion="1.0.149"\nedition="2024"\n')
            shared=root/'shared';(shared/'src').mkdir(parents=True)
            (shared/'Cargo.toml').write_text('[package]\nname="serde_json"\nversion.workspace=true\nedition.workspace=true\n')
            (shared/'src/lib.rs').write_text('pub fn fixture() {}\n')
            adapter=root/'adapter';(adapter/'src').mkdir(parents=True)
            (adapter/'Cargo.toml').write_text('[package]\nname="standalone-binding-fixture"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\nserde_json={path="../shared",version="=1.0.149"}\n')
            (adapter/'src/lib.rs').write_text('pub fn fixture() { serde_json::fixture(); }\n')
            (root/'.gitignore').write_text('adapter/bundle/\n')
            def run(*args):subprocess.run(args,cwd=root,check=True,capture_output=True,text=True)
            run('cargo','generate-lockfile')
            run('cargo','generate-lockfile','--manifest-path',str(adapter/'Cargo.toml'))
            run('git','init','-q');run('git','add','.')
            run('git','-c','user.name=Binding Fixture','-c','user.email=binding-fixture@example.invalid','commit','-qm','reviewed isolated fixture')
            result=self.invoke(adapter)
            self.assertEqual(result.returncode,0,result.stderr)
            record=json.loads((adapter/'bundle/manifest.json').read_text())
            self.assertEqual({item['name'] for item in record['packages']},{'serde_json','standalone-binding-fixture'})
            self.assertEqual(len(record['package_commands']),2)
            self.assertEqual(record['registry_selection'],[])
    def test_target_specific_registry_selection_matches_reviewed_lock(self):
        import json
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            self.project(root,'[dependencies]\nserde_json="=1.0.149"\n[target.\'cfg(windows)\'.dependencies]\nlibc="=0.2.189"\n')
            (root/'.gitignore').write_text('bundle/\n')
            def run(*args):subprocess.run(args,cwd=root,check=True,capture_output=True,text=True)
            run('cargo','generate-lockfile')
            run('git','init','-q')
            run('git','add','.')
            run('git','-c','user.name=Binding Fixture','-c','user.email=binding-fixture@example.invalid','commit','-qm','reviewed fixture')
            result=self.invoke(root);self.assertEqual(result.returncode,0,result.stderr)
            record=json.loads((root/'bundle/manifest.json').read_text())
            selection={(p['name'],p['version']) for p in record['registry_selection']}
            self.assertIn(('serde_json','1.0.149'),selection)
            self.assertIn(('libc','0.2.189'),selection)
            self.assertTrue(all(p['checksum'] for p in record['registry_selection']))
if __name__=='__main__':unittest.main()
