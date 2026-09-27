import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from python_arm64 import (
    PE_ARM64_MACHINE,
    is_pe_arm64,
    pe_machine,
    require_native_windows_arm64,
)


def pe(machine: int, offset: int = 0x80) -> bytes:
    image = bytearray(offset + 8)
    image[:2] = b'MZ'
    image[60:64] = offset.to_bytes(4, 'little')
    image[offset:offset + 4] = b'PE\0\0'
    image[offset + 4:offset + 6] = machine.to_bytes(2, 'little')
    return bytes(image)


class WindowsArm64Tests(unittest.TestCase):
    def test_accepts_native_arm64_pe(self):
        image = pe(PE_ARM64_MACHINE)
        self.assertEqual(pe_machine(image), PE_ARM64_MACHINE)
        self.assertTrue(is_pe_arm64(image))

    def test_rejects_x86_and_x64_pe(self):
        for machine in (0x014C, 0x8664):
            with self.subTest(machine=hex(machine)):
                self.assertFalse(is_pe_arm64(pe(machine)))

    def test_rejects_malformed_headers(self):
        truncated = bytearray(pe(PE_ARM64_MACHINE))
        truncated[60:64] = (0x1000).to_bytes(4, 'little')
        for image in (b'', b'MZ', b'not-a-pe', bytes(truncated),
                      pe(PE_ARM64_MACHINE)[:0x84].replace(b'PE\0\0', b'NOPE')):
            with self.subTest(image=image):
                self.assertIsNone(pe_machine(image))
                self.assertFalse(is_pe_arm64(image))

    def test_requires_native_windows_arm64_cpython(self):
        rejected = (
            ('Windows', 'AMD64', 'cpython'),
            ('Linux', 'aarch64', 'cpython'),
            ('Windows', 'ARM64', 'pypy'),
        )
        for system, machine, implementation in rejected:
            with self.subTest(system=system, machine=machine, implementation=implementation):
                with (patch('platform.system', return_value=system),
                      patch('platform.machine', return_value=machine),
                      patch('sys.implementation', SimpleNamespace(name=implementation))):
                    with self.assertRaisesRegex(
                            RuntimeError, 'native Windows ARM64 runner and CPython are required'):
                        require_native_windows_arm64()

        with (patch('platform.system', return_value='Windows'),
              patch('platform.machine', return_value='ARM64'),
              patch('sys.implementation', SimpleNamespace(name='cpython'))):
            require_native_windows_arm64()


if __name__ == '__main__':
    unittest.main()
