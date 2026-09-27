import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from python_arm64 import (PE_ARM64_MACHINE, is_pe_arm64, main, pe_machine,
                          require_native_windows_arm64)


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

    @patch('python_arm64.sys.implementation.name', 'cpython')
    @patch('python_arm64.platform.machine', return_value='ARM64')
    @patch('python_arm64.platform.system', return_value='Windows')
    def test_workflow_preflight_cli_accepts_native_runner(self, system, machine):
        self.assertIsNone(main())
        system.assert_called_once_with()
        machine.assert_called_once_with()

    @patch('python_arm64.sys.implementation.name', 'cpython')
    @patch('python_arm64.platform.machine', return_value='AMD64')
    @patch('python_arm64.platform.system', return_value='Windows')
    def test_workflow_preflight_rejects_emulated_runner(self, _system, _machine):
        with self.assertRaisesRegex(RuntimeError, 'native Windows ARM64'):
            require_native_windows_arm64()


if __name__ == '__main__':
    unittest.main()
