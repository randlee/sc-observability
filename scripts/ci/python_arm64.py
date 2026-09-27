"""Small, dependency-free native PE/ARM64 qualification helper."""
from __future__ import annotations

PE_SIGNATURE = b"PE\0\0"
PE_ARM64_MACHINE = 0xAA64
WINDOWS_ARM64_RUST_HOST = "aarch64-pc-windows-msvc"


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


def rustc_host(verbose_version: str) -> str | None:
    """Extract the Rust compiler host triple from ``rustc -vV`` output."""
    for line in verbose_version.splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ")
    return None


def require_native_windows_arm64() -> None:
    """Reject Windows emulation/cross-build runners before native evidence runs."""
    import platform
    import subprocess
    import sys

    if (platform.system() != "Windows" or platform.machine().upper() != "ARM64"
            or sys.implementation.name != "cpython"):
        raise RuntimeError("native Windows ARM64 runner and CPython are required")
    host = rustc_host(subprocess.check_output(["rustc", "-vV"], text=True))
    if host != WINDOWS_ARM64_RUST_HOST:
        raise RuntimeError(
            f"native Windows ARM64 rustc host is required; found {host or 'missing host'}"
        )
