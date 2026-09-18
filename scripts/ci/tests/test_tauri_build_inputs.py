"""Actual Cargo must reject mutated vendor reuse and accept pristine profiles."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from _tauri_build_inputs import inventory, materialize, verify_inputs


class BuildInputTests(unittest.TestCase):
    def test_real_vendor_build_mutation_cannot_cross_profiles(self):
        with tempfile.TemporaryDirectory(prefix='tauri-vendor-regression-') as temporary:
            root = Path(temporary)
            pristine = root / 'pristine'
            host = pristine / 'host'
            vendor = host / 'vendor/self-mutating-upstream'
            (host / 'src').mkdir(parents=True)
            (host / '.cargo').mkdir()
            (vendor / 'src').mkdir(parents=True)
            (host / 'Cargo.toml').write_text('[package]\nname="vendor-consumer"\nversion="0.1.0"\nedition="2021"\n[workspace]\n[dependencies]\nself-mutating-upstream="=1.0.0"\n')
            (host / 'src/main.rs').write_text('fn main() { assert_eq!(self_mutating_upstream::value(), 42); }\n')
            (host / '.cargo/config.toml').write_text('[source.crates-io]\nreplace-with="vendored"\n[source.vendored]\ndirectory="vendor"\n[net]\noffline=true\n')
            (vendor / 'Cargo.toml').write_text('[package]\nname="self-mutating-upstream"\nversion="1.0.0"\nedition="2021"\n')
            (vendor / 'src/lib.rs').write_text('pub fn value() -> u8 { 42 }\n')
            (vendor / 'reference.md').write_text('reviewed registry documentation\n')
            (vendor / 'build.rs').write_text('fn main() { println!("cargo:rerun-if-changed=reference.md"); std::fs::write(std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("reference.md"), "regenerated permissions\\n").unwrap(); }\n')
            checksums = inventory(vendor)
            (vendor / '.cargo-checksum.json').write_text(json.dumps({'files': checksums, 'package': None}))
            environment = {key: value for key, value in os.environ.items() if not key.startswith('CARGO_')}
            environment['CARGO_HOME'] = str(root / 'prepare-cargo-home')
            prepare = subprocess.run(['cargo', 'generate-lockfile', '--offline'], cwd=host, env=environment, capture_output=True, text=True)
            self.assertEqual(prepare.returncode, 0, prepare.stderr)
            expected = inventory(pristine)
            profiles = {profile: root / profile for profile in ('debug', 'release')}
            for destination in profiles.values():
                materialize(pristine, destination, expected)
            for profile, destination in profiles.items():
                environment.update(CARGO_HOME=str(root / (profile + '-cargo-home')),
                                   CARGO_TARGET_DIR=str(root / (profile + '-target')))
                command = ['cargo', 'run', '--locked', '--offline']
                if profile == 'release':
                    command.append('--release')
                built = subprocess.run(command, cwd=destination / 'host', env=environment, capture_output=True, text=True)
                self.assertEqual(built.returncode, 0, built.stderr)
                modified = destination / 'host/vendor/self-mutating-upstream/reference.md'
                self.assertNotEqual(hashlib.sha256(modified.read_bytes()).hexdigest(), checksums['reference.md'])
                verify_inputs(pristine, expected)
                reused = subprocess.run(['cargo', 'build', '--locked', '--offline', '--release'],
                                        cwd=destination / 'host', env=environment, capture_output=True, text=True)
                self.assertNotEqual(reused.returncode, 0)
                self.assertIn('checksum', reused.stderr)
            (vendor / 'reference.md').write_text('tampered pristine input\n')
            with self.assertRaisesRegex(ValueError, 'pristine build input drift'):
                materialize(pristine, root / 'rejected', expected)


if __name__ == '__main__':
    unittest.main()
