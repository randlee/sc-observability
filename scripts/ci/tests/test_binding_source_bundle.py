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
if __name__=='__main__':unittest.main()
