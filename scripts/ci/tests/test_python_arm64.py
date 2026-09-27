import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from python_arm64 import (
    PE_ARM64_MACHINE,
    WINDOWS_ARM64_RUST_HOST,
    is_pe_arm64,
    pe_machine,
    require_native_windows_arm64,
    rustc_host,
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

    def test_extracts_rustc_host(self):
        self.assertEqual(
            rustc_host(f"rustc 1.94.1\nhost: {WINDOWS_ARM64_RUST_HOST}\nrelease: 1.94.1"),
            WINDOWS_ARM64_RUST_HOST,
        )
        self.assertIsNone(rustc_host("rustc 1.94.1\nrelease: 1.94.1"))

    def test_rejects_cross_compiler_host(self):
        with (patch("platform.system", return_value="Windows"),
              patch("platform.machine", return_value="ARM64"),
              patch("subprocess.check_output", return_value="host: x86_64-pc-windows-msvc\n")):
            with self.assertRaisesRegex(RuntimeError, "rustc host"):
                require_native_windows_arm64()


if __name__ == '__main__':
    unittest.main()
