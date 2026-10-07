"""Small validation helpers for the compatible-contract registry."""

import re
from collections import Counter
from collections.abc import Iterable, Mapping
from pathlib import Path

PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES = ("released public nominal identity",)

BASELINE_COMMIT = "c578912653233c7dc678fefe5af575118dbbaaa1"

RELEASED_TYPED_BUILTIN_IMPLS = frozenset({
    "sc_observability::typed::TypedLogSink for sc_observability::JsonlFileSink",
    "sc_observability::typed::TypedLogSink for sc_observability::ConsoleSink",
})
TRAIT_IMPL_CONTRACT_FIELDS = frozenset({
    "implementation",
    "baseline_declaration",
    "current_declaration",
    "baseline_source",
    "current_source",
    "conversion",
    "removable_paths",
    "removal_rationale",
})
TRAIT_IMPL_SOURCE_FIELDS = frozenset({"revision", "path", "owner"})

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

# Phase-F lead rulings retain these v1-only compatibility owners as deprecated
# while their eventual migration remains in flight.
RESTORED_V1_DEPRECATED_OWNERS: dict[str, tuple[str, ...]] = {
    "crates/sc-observability-types/src/errors.rs": ("TelemetryError",),
    "crates/sc-observability/src/error_codes.rs": (
        "LOGGER_SHUTDOWN",
        "LOGGER_MAINTENANCE_JOIN_TIMEOUT",
        "LOGGER_MAINTENANCE_WORKER_FAILED",
    ),
    "crates/sc-observability-types/src/error_codes.rs": ("ALL", "ALL"),
    "crates/sc-observe/src/lib.rs": (
        "ObservabilityConfig",
        "ObservabilityBuilder",
        "Observability",
        "builder",
        "emit",
        "health",
        "with_observability_health_provider",
    ),
}

# Path segments only: `foo_compat::` and `compatibility::` are not references.
COMPAT_PATH_REFERENCE = re.compile(r"\bcompat::")
COMPAT_USE_REFERENCE = re.compile(r"\buse\s[^;]*\bcompat\b(?!::)")
COMPAT_MODULE_DECLARATION = re.compile(r"\bmod\s+compat\b")

_ITEM_NAME = re.compile(r"\b(?:fn|struct|enum|trait|type|const|static|mod|union)\s+([A-Za-z_]\w*)")
_FIELD_OR_VARIANT_NAME = re.compile(r"(?:pub(?:\([^)]*\))?\s+)?([A-Za-z_]\w*)")
_REEXPORT_NAME = re.compile(r"\bpub(?:\([^)]*\))?\s+use\s+([^;]+);")


def has_placeholder_baseline_signature(signature: str) -> bool:
    """Return whether a registry signature is a known nominal-identity placeholder."""
    return any(signature.startswith(prefix) for prefix in PLACEHOLDER_BASELINE_SIGNATURE_PREFIXES)


def validate_contract_signatures(rows: Iterable[dict]) -> None:
    """Reject unsigned or placeholder signatures and unequal unchanged aliases."""
    for row in rows:
        symbol = row.get("symbol")
        baseline = row["baseline_signature"]
        canonical = row["canonical_signature"]
        if (
            not isinstance(baseline, str)
            or not baseline.strip()
            or has_placeholder_baseline_signature(baseline.strip())
        ):
            raise ValueError(f"compatibility registry has an unsigned or placeholder baseline contract: {symbol}")
        if canonical is not None and (
            not isinstance(canonical, str)
            or not canonical.strip()
            or has_placeholder_baseline_signature(canonical.strip())
        ):
            raise ValueError(f"compatibility registry has an unsigned or placeholder canonical contract: {symbol}")
        if row["treatment"] == "unchanged_alias" and canonical != baseline:
            raise ValueError(f"unchanged_alias baseline and canonical signatures differ: {symbol}")


def validate_trait_slot_contracts(rows: Iterable[dict]) -> None:
    """Reject trait-slot adapters without a signature change or with a removable canonical path."""
    for row in rows:
        symbol = row.get("symbol")
        if row["treatment"] == "new_adapter" and row["baseline_signature"] == row["canonical_signature"]:
            raise ValueError(f"trait-slot new_adapter baseline and canonical signatures are equal: {symbol}")
        canonical_path = (row.get("canonical_source") or {}).get("path")
        if canonical_path is not None and canonical_path in row["removable_paths"]:
            raise ValueError(f"trait-slot removable_paths contains its canonical source: {symbol}")


def validate_trait_impl_contracts(records: object) -> None:
    """Require the two released built-in TypedLogSink impl identities exactly once."""
    if not isinstance(records, list):
        raise ValueError("compatibility trait-impl contracts must be a list")
    implementations: list[str] = []
    for record in records:
        if not isinstance(record, dict) or set(record) != TRAIT_IMPL_CONTRACT_FIELDS:
            raise ValueError(f"malformed compatibility trait-impl contract: {record!r}")
        implementation = record["implementation"]
        if not isinstance(implementation, str) or not implementation.strip():
            raise ValueError(f"compatibility trait-impl contract has no implementation: {record!r}")
        implementations.append(implementation)
        for field in ("baseline_declaration", "current_declaration", "conversion", "removal_rationale"):
            if not isinstance(record[field], str) or not record[field].strip():
                raise ValueError(f"compatibility trait-impl contract has blank {field}: {implementation}")
        for field, revision in (("baseline_source", BASELINE_COMMIT), ("current_source", "selected_head")):
            source = record[field]
            if not isinstance(source, Mapping) or set(source) != TRAIT_IMPL_SOURCE_FIELDS:
                raise ValueError(f"compatibility trait-impl contract has malformed {field}: {implementation}")
            if source["revision"] != revision or any(
                not isinstance(source[key], str) or not source[key].strip()
                for key in ("path", "owner")
            ):
                raise ValueError(f"compatibility trait-impl contract has invalid {field}: {implementation}")
        removable = record["removable_paths"]
        if not isinstance(removable, list) or removable != ["crates/sc-observability/src/v1/compat.rs"]:
            raise ValueError(f"compatibility trait-impl contract has invalid removable paths: {implementation}")
    duplicates = [name for name, count in Counter(implementations).items() if count > 1]
    if duplicates:
        raise ValueError(f"duplicate compatibility trait-impl contracts: {', '.join(sorted(duplicates))}")
    if set(implementations) != RELEASED_TYPED_BUILTIN_IMPLS:
        missing = sorted(RELEASED_TYPED_BUILTIN_IMPLS - set(implementations))
        unknown = sorted(set(implementations) - RELEASED_TYPED_BUILTIN_IMPLS)
        raise ValueError(f"compatibility trait-impl contracts drifted: missing={missing}, unknown={unknown}")


def is_compat_source_path(relative_path: str) -> bool:
    """Return whether a repository-relative path is a compatibility source file."""
    normalized = relative_path.lstrip("/")
    return (
        normalized == "src/compat.rs"
        or normalized.endswith("/src/compat.rs")
        or "/src/compat/" in f"/{normalized}"
        or "/src/v1/" in f"/{normalized}"
        or normalized == "src/v1.rs"
        or normalized.endswith("/src/v1.rs")
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
        reexport = _REEXPORT_NAME.search(owner)
        if reexport:
            target = reexport.group(1).strip()
            match = re.search(r"([A-Za-z_]\w*)$", target)
            names.append(match.group(1) if match else target)
            continue
        match = _ITEM_NAME.search(owner) or _FIELD_OR_VARIANT_NAME.match(owner)
        names.append(match.group(1) if match else owner)
    return names


DEPRECATED_OWNER_EXCEPTION_FIELDS = frozenset({"file", "deprecated_symbols", "reason", "removal_point"})


def deprecated_owner_exception_records(
    registry: dict, baseline: Mapping[str, tuple[str, ...]] = DEPRECATED_OWNER_BASELINE
) -> dict[str, tuple[str, ...]]:
    """Return each excepted file's declared deprecated symbols after checking its record."""
    records = registry.get("deprecated_owner_exceptions", [])
    if not isinstance(records, list):
        raise ValueError("deprecated_owner_exceptions must be a list of records")
    declared: dict[str, tuple[str, ...]] = {}
    for record in records:
        if not isinstance(record, dict) or set(record) != DEPRECATED_OWNER_EXCEPTION_FIELDS:
            raise ValueError(f"malformed deprecated owner exception record: {record!r}")
        relative = record["file"]
        if not isinstance(relative, str) or not relative.strip():
            raise ValueError(f"deprecated owner exception has no file: {record!r}")
        if relative in declared:
            raise ValueError(f"duplicate deprecated owner exception: {relative}")
        if relative not in baseline and relative not in RESTORED_V1_DEPRECATED_OWNERS:
            raise ValueError(f"deprecated owner exception has no v1.4.1 baseline: {relative}")
        for field in ("reason", "removal_point"):
            if not isinstance(record[field], str) or not record[field].strip():
                raise ValueError(f"deprecated owner exception has a blank {field}: {relative}")
        symbols = record["deprecated_symbols"]
        if (
            not isinstance(symbols, list)
            or not symbols
            or any(not isinstance(symbol, str) or not symbol.strip() for symbol in symbols)
        ):
            raise ValueError(f"deprecated owner exception must name its deprecated symbols: {relative}")
        allowed = (*baseline.get(relative, ()), *RESTORED_V1_DEPRECATED_OWNERS.get(relative, ()))
        unknown = Counter(symbols) - Counter(allowed)
        if unknown:
            raise ValueError(
                f"deprecated owner exception declares symbols outside the v1.4.1 baseline: {relative}: "
                f"{', '.join(sorted(unknown.elements()))}"
            )
        declared[relative] = tuple(symbols)
    return declared


def validate_compatibility_source_boundary(
    root: Path,
    source_files: Iterable[Path],
    registry: dict,
    baseline: Mapping[str, tuple[str, ...]] = DEPRECATED_OWNER_BASELINE,
) -> None:
    """Reject canonical-to-compat imports and deprecated owners beyond the baseline."""
    deprecated_exceptions = deprecated_owner_exception_records(registry, baseline)
    root_reexport_exceptions = set(registry.get("compat_root_reexport_exceptions", []))
    owning_files = set()

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
        names = Counter(deprecated_owner_names(text))
        allowed = (*baseline.get(relative, ()), *RESTORED_V1_DEPRECATED_OWNERS.get(relative, ()))
        excess = names - Counter(allowed)
        if excess:
            raise ValueError(
                f"deprecated owner exceeds v1.4.1 baseline: {relative}: {', '.join(sorted(excess.elements()))}"
            )
        if names != Counter(deprecated_exceptions[relative]):
            raise ValueError(
                f"deprecated owner exception symbols differ from source: {relative}: "
                f"declared {', '.join(deprecated_exceptions[relative])}; source {', '.join(sorted(names.elements()))}"
            )
        owning_files.add(relative)
    for relative in sorted(set(deprecated_exceptions) - owning_files):
        raise ValueError(f"deprecated owner exception names a file without deprecated owners: {relative}")
