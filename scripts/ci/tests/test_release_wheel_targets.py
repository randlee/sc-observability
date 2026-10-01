"""Keep installed release validation aligned with the declared platform policy."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    'release_wheel_targets_under_test', ROOT / '.github/scripts/release_python.py')
RELEASE_PYTHON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE_PYTHON)


class ReleaseWheelTargetTests(unittest.TestCase):
    def setUp(self):
        manifest = tomllib.loads((ROOT / 'release/publish-artifacts.toml').read_text())
        self.distribution, = manifest['python_distributions']
        self.arm = next(wheel for wheel in self.distribution['wheels']
                        if wheel['id'] == 'windows-arm64')

    def test_real_manifest_targets_cover_release_platform_policy(self):
        policy = json.loads((ROOT / 'release/python-platform-policy.json').read_text())
        targets = RELEASE_PYTHON.wheel_targets(self.distribution)
        self.assertEqual({target['platform'] for target in targets},
                         {platform['wheel_platform'] for platform in policy['platforms']})
        arm_policy = next(platform for platform in policy['platforms']
                          if platform['id'] == 'windows-arm64')
        self.assertEqual((self.arm['target'], self.arm['os'], self.arm['platform']),
                         (arm_policy['rust_target'], arm_policy['runner'], arm_policy['wheel_platform']))
        paths = [Path(f"sc_observability-2.0.0-cp310-abi3-{target['platform']}.whl")
                 for target in targets]
        RELEASE_PYTHON.verify_platforms(self.distribution, paths)
        with self.assertRaisesRegex(ValueError, 'missing'):
            RELEASE_PYTHON.verify_platforms(self.distribution, paths[:-1])

    def test_arm64_target_rejects_wrong_runner_or_platform(self):
        for field, value in [('os', 'windows-2022'), ('os', 'ubuntu-24.04-arm'),
                             ('platform', 'win_amd64'), ('platform', 'win32')]:
            with self.subTest(field=field, value=value):
                with self.assertRaisesRegex(ValueError, 'mismatch'):
                    RELEASE_PYTHON.wheel_targets({'wheels': [{**self.arm, field: value}]})

    def test_unsupported_target_and_setuptools_remain_rejected(self):
        with self.assertRaisesRegex(ValueError, 'unsupported explicit wheel target'):
            RELEASE_PYTHON.wheel_targets({'wheels': [{**self.arm, 'target': 'i686-pc-windows-msvc'}]})
        with self.assertRaisesRegex(ValueError, 'require maturin'):
            RELEASE_PYTHON.wheel_targets({'wheels': [self.arm], 'build_system': 'setuptools'})

    def test_exact_ci_manifest_validation_command(self):
        result = subprocess.run([
            sys.executable, '.github/scripts/release_artifacts.py', 'validate-manifest',
            '--manifest', 'release/publish-artifacts.toml', '--workspace-toml', 'Cargo.toml',
        ], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('manifest validation passed', result.stdout)


if __name__ == '__main__':
    unittest.main()
