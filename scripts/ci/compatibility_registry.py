"""Small validation helpers for the compatible-contract registry."""

PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES = ("released public nominal identity",)


def has_placeholder_baseline_signature(signature: str) -> bool:
    """Return whether a registry signature is a known nominal-identity placeholder."""
    return any(signature.startswith(prefix) for prefix in PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES)
