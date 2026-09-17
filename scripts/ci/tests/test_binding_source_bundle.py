"""Boundary tests invoking the real helper before Cargo can follow bad paths."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
HELPER=ROOT/'scripts/ci/build_binding_source_bundle.py'
class SourceBoundaryTests(unittest.TestCase):
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
