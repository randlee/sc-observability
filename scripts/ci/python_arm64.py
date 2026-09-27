"""Small, dependency-free native PE/ARM64 qualification helper."""
from __future__ import annotations

import platform
import sys

PE_SIGNATURE = b"PE\0\0"
PE_ARM64_MACHINE = 0xAA64
WINDOWS_ARM64_POLICY = {"id": "windows-arm64", "runner": "windows-11-arm", "machine": "ARM64", "wheel_platform": "win_arm64", "rust_target": "aarch64-pc-windows-msvc"}


def apply_windows_arm64_overlay(policy: dict) -> dict:
    """Add or validate the temporary D.10 policy row; D.18 removes it on activation."""
    row = next((item for item in policy["platforms"] if item["id"] == "windows-arm64"), None)
    if row is None:
        policy["platforms"].append(WINDOWS_ARM64_POLICY.copy())
    elif any(
        key != "runner" and row.get(key) != value
        or key == "runner" and key in row and row[key] != value
        for key, value in WINDOWS_ARM64_POLICY.items()
    ):
        raise RuntimeError("Windows ARM64 policy row differs from the D.10 handoff")
    return policy


def pe_machine(data: bytes) -> int | None:
    """Return the PE COFF machine value, or ``None`` for malformed bytes."""
    if len(data) < 64 or data[:2] != b"MZ":
        return None
    offset = int.from_bytes(data[60:64], "little")
    if offset < 64 or offset + 6 > len(data) or data[offset:offset + 4] != PE_SIGNATURE:
        return None
    return int.from_bytes(data[offset + 4:offset + 6], "little")


def is_pe_arm64(data: bytes) -> bool:
    """Whether bytes contain a structurally valid native Windows ARM64 PE."""
    return pe_machine(data) == PE_ARM64_MACHINE


def require_native_windows_arm64() -> None:
    """Reject Windows emulation/cross-build runners before native evidence runs."""
    if (platform.system() != "Windows" or platform.machine().upper() != "ARM64"
            or sys.implementation.name != "cpython"):
        raise RuntimeError("native Windows ARM64 runner and CPython are required")


def main() -> None:
    """Run the native Windows ARM64 preflight used by workflow jobs."""
    require_native_windows_arm64()


if __name__ == "__main__":
    main()
