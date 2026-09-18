"""Boundary tests invoking the real helper before Cargo can follow bad paths."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
import importlib.util
ROOT=Path(__file__).resolve().parents[3]
HELPER=ROOT/'scripts/ci/build_binding_source_bundle.py'
SPEC=importlib.util.spec_from_file_location('binding_source_bundle', HELPER)
BUNDLE=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(BUNDLE)
class SourceBoundaryTests(unittest.TestCase):
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
