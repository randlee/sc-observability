import json
import enum
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

    def test_enum_default_constructor_is_canonical_but_custom_constructor_changes(self):
        default = 'import enum\nclass Cause(str, enum.Enum):\n    VALUE = "value"\n'
        expected = project(self.module(default))
        previous = enum.Enum.__init__

        def enum_init(self, *_):
            pass

        enum_init.__module__ = 'enum'
        enum_init.__qualname__ = 'Enum.__init__'
        try:
            enum.Enum.__init__ = enum_init
            self.assertEqual(expected, project(self.module(default)))
        finally:
            enum.Enum.__init__ = previous

        custom = ('import enum\nclass Cause(enum.Enum):\n    VALUE = ("value", "label")\n'
                  '    def __init__(self, value, label):\n        self.label = label\n')
        changed = ('import enum\nclass Cause(enum.Enum):\n    VALUE = ("value", "label")\n'
                   '    def __init__(self, value, label, priority=0):\n        self.label = label\n        self.priority = priority\n')
        custom_rows = project(self.module(custom))
        changed_rows = project(self.module(changed))
        custom_constructor = next(row for row in custom_rows if row.startswith('constructor sc_test_api.Cause '))
        changed_constructor = next(row for row in changed_rows if row.startswith('constructor sc_test_api.Cause '))
        self.assertNotEqual(custom_constructor, changed_constructor)
        self.assertNotEqual(custom_rows, changed_rows)


if __name__ == '__main__':
    unittest.main()
