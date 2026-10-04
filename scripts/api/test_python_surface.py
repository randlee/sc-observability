import json
from pathlib import Path
import tempfile
import types
import unittest

from scripts.api.python_surface import compare, project


class RuntimeProjectionTests(unittest.TestCase):
    def module(self, definition):
        module = types.ModuleType('sc_test_api')
        exec(definition, module.__dict__)
        return module

    def test_runtime_signature_field_trait_and_reexport_mutations(self):
        original = 'class Value:\n    field: int\n    def read(self, value: int) -> int: ...\nAlias = Value\n'
        expected = project(self.module(original))
        for changed in [original.replace('value: int', 'value: str'), original.replace('field: int', 'field: str'),
                        original.replace('class Value:', 'class Parent: pass\nclass Value(Parent):'),
                        original.replace('Alias = Value', '_Alias = Value')]:
            with self.subTest(changed=changed):
                self.assertNotEqual(expected, project(self.module(changed)))

    def test_comparison_rejects_current_mutation_and_wrong_version(self):
        baseline = project(self.module('def read(value: int) -> int: ...'))
        changed = project(self.module('def read(value: str) -> int: ...'))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '1.0.json'
            path.write_text(json.dumps({'version': '1.0', 'format': 'python-runtime/v1', 'rows': baseline}))
            compare(baseline, path, '1.0')
            with self.assertRaisesRegex(AssertionError, 'Python API differs'):
                compare(changed, path, '1.0')
            with self.assertRaisesRegex(AssertionError, 'version differs'):
                compare(baseline, path, '2.0')

    def test_deterministic_rows(self):
        module = self.module('def read(value: int = 2) -> str: ...')
        self.assertEqual(project(module), project(module))


if __name__ == '__main__':
    unittest.main()
