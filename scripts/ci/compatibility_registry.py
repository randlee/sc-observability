"""Small validation helpers for the compatible-contract registry."""

import re
from collections import Counter
from collections.abc import Iterable, Mapping
from pathlib import Path

PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES = ("released public nominal identity",)

BASELINE_COMMIT = "c578912653233c7dc678fefe5af575118dbbaaa1"

# Deprecated owners each excepted file carried at the pinned v1.4.1 commit.
# An exception covers only these names; any other deprecated owner in the file
# is new deprecated surface and must live under `src/compat`.
DEPRECATED_OWNER_BASELINE: dict[str, tuple[str, ...]] = {
    "crates/sc-observability-types/src/errors.rs": (
        "IdentityError", "InitError", "EventError", "FlushError", "ShutdownError",
        "ProjectionError", "SubscriberError", "LogSinkError", "ExportError",
    ),
    "crates/sc-observability/src/lib.rs": ("max_age_days",),
    "crates/sc-observability/src/runtime.rs": (
        "builder", "new", "log", "try_log", "try_log_with_outcome", "emit", "flush",
    ),
    "crates/sc-observability-otlp/src/assembly.rs": ("push",),
}

# Path segments only: `foo_compat::` and `compatibility::` are not references.
COMPAT_PATH_REFERENCE = re.compile(r"\bcompat::")
COMPAT_USE_REFERENCE = re.compile(r"\buse\s[^;]*\bcompat\b(?!::)")
COMPAT_MODULE_DECLARATION = re.compile(r"\bmod\s+compat\b")

_ITEM_NAME = re.compile(r"\b(?:fn|struct|enum|trait|type|const|static|mod|union)\s+([A-Za-z_]\w*)")
_FIELD_OR_VARIANT_NAME = re.compile(r"(?:pub(?:\([^)]*\))?\s+)?([A-Za-z_]\w*)")


def has_placeholder_baseline_signature(signature: str) -> bool:
    """Return whether a registry signature is a known nominal-identity placeholder."""
    return any(signature.startswith(prefix) for prefix in PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES)


def validate_contract_signatures(rows: Iterable[dict]) -> None:
    """Reject unsigned or placeholder signatures and unequal unchanged aliases."""
    for row in rows:
        symbol = row.get("symbol")
        baseline = row["baseline_signature"]
        canonical = row["canonical_signature"]
        if not isinstance(baseline, str) or not baseline or has_placeholder_baseline_signature(baseline):
            raise ValueError(f"compatibility registry has an unsigned or placeholder baseline contract: {symbol}")
        if canonical is not None and (
            not isinstance(canonical, str) or not canonical or has_placeholder_baseline_signature(canonical)
        ):
            raise ValueError(f"compatibility registry has an unsigned or placeholder canonical contract: {symbol}")
        if row["treatment"] == "unchanged_alias" and canonical != baseline:
            raise ValueError(f"unchanged_alias baseline and canonical signatures differ: {symbol}")


def is_compat_source_path(relative_path: str) -> bool:
    """Return whether a repository-relative path is a compatibility source file."""
    normalized = relative_path.lstrip("/")
    return (
        normalized == "src/compat.rs"
        or normalized.endswith("/src/compat.rs")
        or "/src/compat/" in f"/{normalized}"
    )


def is_crate_root_path(relative_path: str) -> bool:
    """Return whether a repository-relative path is a library crate root."""
    normalized = relative_path.lstrip("/")
    return normalized == "src/lib.rs" or normalized.endswith("/src/lib.rs")


def is_allowed_compat_reference_source(
    relative_path: str, root_reexport_exceptions: set[str]
) -> bool:
    """Return whether a source path may reference a compatibility module."""
    normalized = relative_path.lstrip("/")
    return is_compat_source_path(normalized) or (
        normalized.endswith("/lib.rs") and normalized in root_reexport_exceptions
    )


def _attribute_end(text: str, start: int) -> int:
    """Return the offset just past the attribute opened by `#[` at `start`."""
    depth = 0
    for offset in range(start, len(text)):
        if text[offset] == "[":
            depth += 1
        elif text[offset] == "]":
            depth -= 1
            if depth == 0:
                return offset + 1
    return len(text)


def _owner_start(text: str, offset: int) -> int:
    """Skip whitespace, line comments and further attributes before an owner."""
    while offset < len(text):
        if text[offset].isspace():
            offset += 1
        elif text.startswith("//", offset):
            end = text.find("\n", offset)
            offset = len(text) if end < 0 else end
        elif text.startswith("#[", offset):
            offset = _attribute_end(text, offset)
        else:
            break
    return offset


def deprecated_owner_names(text: str) -> list[str]:
    """Return the name of each item, field or variant carrying `#[deprecated]`.

    The owner is the first code after the attribute block, on the attribute's
    own line when item text follows the closing bracket.
    """
    names = []
    consumed = 0
    line_start = 0
    for line in text.splitlines(keepends=True):
        start = line_start + len(line) - len(line.lstrip())
        line_start += len(line)
        if start < consumed or not text.startswith("#[deprecated", start):
            continue
        consumed = _owner_start(text, _attribute_end(text, start))
        end = text.find("\n", consumed)
        owner = text[consumed : len(text) if end < 0 else end].strip()
        match = _ITEM_NAME.search(owner) or _FIELD_OR_VARIANT_NAME.match(owner)
        names.append(match.group(1) if match else owner)
    return names


def validate_compatibility_source_boundary(
    root: Path,
    source_files: Iterable[Path],
    registry: dict,
    baseline: Mapping[str, tuple[str, ...]] = DEPRECATED_OWNER_BASELINE,
) -> None:
    """Reject canonical-to-compat imports and deprecated owners beyond the baseline."""
    deprecated_exceptions = set(registry.get("deprecated_owner_exceptions", []))
    root_reexport_exceptions = set(registry.get("compat_root_reexport_exceptions", []))
    for relative in sorted(deprecated_exceptions - set(baseline)):
        raise ValueError(f"deprecated owner exception has no v1.4.1 baseline: {relative}")

    for path in source_files:
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        is_compat_source = is_compat_source_path(relative)
        if not is_allowed_compat_reference_source(relative, root_reexport_exceptions) and (
            COMPAT_PATH_REFERENCE.search(text) or COMPAT_USE_REFERENCE.search(text)
        ):
            raise ValueError(f"canonical source imports compatibility module: {relative}")
        if (
            not is_compat_source
            and not is_crate_root_path(relative)
            and COMPAT_MODULE_DECLARATION.search(text)
        ):
            raise ValueError(f"compatibility module declared outside a crate root: {relative}")
        if is_compat_source or "#[deprecated" not in text:
            continue
        if relative not in deprecated_exceptions:
            raise ValueError(f"deprecated owner is outside compat without registry exception: {relative}")
        excess = Counter(deprecated_owner_names(text)) - Counter(baseline[relative])
        if excess:
            raise ValueError(
                f"deprecated owner exceeds v1.4.1 baseline: {relative}: {', '.join(sorted(excess.elements()))}"
            )
