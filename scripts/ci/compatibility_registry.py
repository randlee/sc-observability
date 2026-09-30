"""Small validation helpers for the compatible-contract registry."""

import re
from collections.abc import Iterable
from pathlib import Path

PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES = ("released public nominal identity",)


def has_placeholder_baseline_signature(signature: str) -> bool:
    """Return whether a registry signature is a known nominal-identity placeholder."""
    return any(signature.startswith(prefix) for prefix in PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES)


def is_compat_source_path(relative_path: str) -> bool:
    """Return whether a repository-relative path is a compatibility source file."""
    normalized = relative_path.lstrip("/")
    return (
        normalized == "src/compat.rs"
        or normalized.endswith("/src/compat.rs")
        or "/src/compat/" in f"/{normalized}"
    )


def is_allowed_compat_reference_source(
    relative_path: str, root_reexport_exceptions: set[str]
) -> bool:
    """Return whether a source path may reference a compatibility module."""
    normalized = relative_path.lstrip("/")
    return is_compat_source_path(normalized) or (
        normalized.endswith("/lib.rs") and normalized in root_reexport_exceptions
    )


def validate_compatibility_source_boundary(
    root: Path, source_files: Iterable[Path], registry: dict,
) -> None:
    """Reject canonical-to-compat imports and unrecorded deprecated owners."""
    deprecated_exceptions = set(registry.get("deprecated_owner_exceptions", []))
    root_reexport_exceptions = set(registry.get("compat_root_reexport_exceptions", []))
    compat_reference = re.compile(r"(?:crate::)?compat::|::compat::")

    for path in source_files:
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        is_compat_source = is_compat_source_path(relative)
        if not is_allowed_compat_reference_source(relative, root_reexport_exceptions) and compat_reference.search(text):
            raise ValueError(f"canonical source imports compatibility module: {relative}")
        if "#[deprecated" in text and not is_compat_source and relative not in deprecated_exceptions:
            raise ValueError(f"deprecated owner is outside compat without registry exception: {relative}")
