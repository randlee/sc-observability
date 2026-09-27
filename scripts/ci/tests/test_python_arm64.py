import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from python_arm64 import (
    PE_ARM64_MACHINE,
    WINDOWS_ARM64_POLICY,
    WINDOWS_ARM64_RUST_HOST,
    apply_windows_arm64_overlay,
    is_pe_arm64,
    main,
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

    @patch('python_arm64.sys.implementation.name', 'cpython')
    @patch('python_arm64.platform.machine', return_value='ARM64')
    @patch('python_arm64.platform.system', return_value='Windows')
    def test_workflow_preflight_cli_accepts_native_runner(self, system, machine):
        with patch('python_arm64.subprocess.check_output', return_value=f'host: {WINDOWS_ARM64_RUST_HOST}\n'):
            self.assertIsNone(main())
        system.assert_called_once_with()
        machine.assert_called_once_with()

    @patch('python_arm64.sys.implementation.name', 'cpython')
    @patch('python_arm64.platform.machine', return_value='AMD64')
    @patch('python_arm64.platform.system', return_value='Windows')
    def test_workflow_preflight_rejects_emulated_runner(self, _system, _machine):
        with self.assertRaisesRegex(RuntimeError, 'native Windows ARM64'):
            require_native_windows_arm64()

    def test_policy_overlay_adds_and_rejects_drift(self):
        policy = {"platforms": []}
        self.assertEqual(
            apply_windows_arm64_overlay(policy)["platforms"], [WINDOWS_ARM64_POLICY]
        )
        with self.assertRaisesRegex(RuntimeError, "differs"):
            apply_windows_arm64_overlay({"platforms": [{**WINDOWS_ARM64_POLICY, "runner": "x64"}]})

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
              patch('sys.implementation', SimpleNamespace(name='cpython')),
              patch('subprocess.check_output', return_value=f'host: {WINDOWS_ARM64_RUST_HOST}\n')):
            require_native_windows_arm64()


if __name__ == '__main__':
    unittest.main()
