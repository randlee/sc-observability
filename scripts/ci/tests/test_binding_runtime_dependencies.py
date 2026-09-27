import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts/ci'))

import validate_binding_runtime_dependencies as validator


class BindingRuntimeDependencyTests(unittest.TestCase):
    def test_missing_consumer_boundary_manifest_fails_closed(self):
        original_loader = validator.load_boundary_manifest

        def missing_tauri_manifest(package):
            if package == 'sc-observability-tauri':
                return None
            return original_loader(package)

        with mock.patch.object(validator, 'load_boundary_manifest', side_effect=missing_tauri_manifest):
            with self.assertRaisesRegex(ValueError, 'sc-observability-tauri boundary manifest is missing'):
                validator.main()


if __name__ == '__main__':
    unittest.main()
