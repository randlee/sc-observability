"""Small validation helpers for the compatible-contract registry."""

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
