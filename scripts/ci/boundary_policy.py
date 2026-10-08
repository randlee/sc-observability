"""Read first-party dependency policy from boundary manifests."""

from pathlib import Path
import tomllib


def is_first_party_dependency(name: str) -> bool:
    return (
        name == "sc-observe"
        or name.startswith("sc-observability")
        or name.startswith("sc-otel")
    )


HOME_DISCOVERY_TOKENS = (
    "dirs::home_dir",
    "dirs_next::home_dir",
    "home_dir()",
    'var("HOME")',
    'var_os("HOME")',
)
HOME_TEST_ISOLATION_TOKENS = (
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
)


def discovers_home_paths(relative_path: Path, text: str) -> bool:
    """Shared crate source must not discover home or XDG locations.

    Integration tests under `crates/<crate>/tests/` may set XDG markers to
    isolate a child process, but must still reject home-directory discovery.
    """
    parts = relative_path.parts
    is_crate_test_path = len(parts) >= 3 and parts[0] == "crates" and parts[2] == "tests"
    tokens = HOME_DISCOVERY_TOKENS
    if not is_crate_test_path:
        tokens += HOME_TEST_ISOLATION_TOKENS
    return any(token in text for token in tokens)


def boundary_manifest(root: Path, package: str) -> tuple[Path, dict]:
    matches = []
    for manifest_path in (root / "boundaries").glob("*/*.toml"):
        document = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        if document.get("owner_package") == package:
            matches.append((manifest_path, document))

    if len(matches) != 1:
        paths = [str(path) for path, _ in matches]
        raise ValueError(f"{package} must have exactly one boundary manifest: {paths}")
    return matches[0]


def _string_list(manifest_path: Path, document: dict, field: str) -> list[str]:
    try:
        values = document["dependencies"][field]
    except KeyError as error:
        raise ValueError(f"{manifest_path} lacks dependencies.{field}") from error
    if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
        raise ValueError(f"{manifest_path} dependencies.{field} must be a string list")
    return values


def allowed_dependencies(root: Path, package: str) -> set[str]:
    manifest_path, document = boundary_manifest(root, package)
    return set(_string_list(manifest_path, document, "allowed_dependencies"))


def allowed_dependents(root: Path, package: str) -> set[str]:
    manifest_path, document = boundary_manifest(root, package)
    return set(_string_list(manifest_path, document, "allowed_dependents"))


def forbidden_edges(root: Path, package: str) -> set[str]:
    manifest_path, document = boundary_manifest(root, package)
    try:
        edges = document["dependencies"]["forbidden_edges"]
    except KeyError as error:
        raise ValueError(f"{manifest_path} lacks dependencies.forbidden_edges") from error
    if not isinstance(edges, list):
        raise ValueError(f"{manifest_path} dependencies.forbidden_edges must be a list")
    targets = set()
    for edge in edges:
        if (
            not isinstance(edge, dict)
            or set(edge) != {"from", "to"}
            or not all(isinstance(value, str) for value in edge.values())
        ):
            raise ValueError(
                f"{manifest_path} forbidden_edges entries must be {{ from, to }} strings"
            )
        if edge["from"] != package:
            raise ValueError(
                f"{manifest_path} forbidden edge must start at {package}: {edge}"
            )
        targets.add(edge["to"])
    return targets


def validate_allowed_dependents(
    root: Path, package: str, dependencies: set[str]
) -> None:
    for dependency in sorted(
        dependency for dependency in dependencies if is_first_party_dependency(dependency)
    ):
        if package not in allowed_dependents(root, dependency):
            raise ValueError(f"{package} is not an allowed dependent of {dependency}")


def validate_first_party_dependencies(
    root: Path, package: str, dependencies: set[str]
) -> None:
    forbidden = sorted(dependencies & forbidden_edges(root, package))
    if forbidden:
        raise ValueError(f"{package} declares forbidden edge(s): {forbidden}")

    actual = {dependency for dependency in dependencies if is_first_party_dependency(dependency)}
    expected = allowed_dependencies(root, package)
    if actual != expected:
        raise ValueError(
            f"{package} first-party dependency drift: "
            f"expected {sorted(expected)}, found {sorted(actual)}"
        )

    validate_allowed_dependents(root, package, dependencies)
