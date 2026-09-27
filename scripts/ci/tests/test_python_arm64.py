import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from python_arm64 import PE_ARM64_MACHINE, is_pe_arm64, pe_machine
from python_test_fixtures import pe


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


if __name__ == '__main__':
    unittest.main()
