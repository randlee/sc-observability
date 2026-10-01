"""Read first-party dependency policy from boundary manifests."""

from pathlib import Path
import tomllib


def is_first_party_dependency(name: str) -> bool:
    return name == "sc-observe" or name.startswith("sc-observability")


def allowed_dependencies(root: Path, package: str) -> set[str]:
    matches = []
    for manifest_path in (root / "boundaries").glob("*/*.toml"):
        document = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        if document.get("owner_package") == package:
            matches.append((manifest_path, document))

    if len(matches) != 1:
        paths = [str(path) for path, _ in matches]
        raise ValueError(f"{package} must have exactly one boundary manifest: {paths}")

    manifest_path, document = matches[0]
    try:
        dependencies = document["dependencies"]["allowed_dependencies"]
    except KeyError as error:
        raise ValueError(
            f"{manifest_path} lacks dependencies.allowed_dependencies"
        ) from error
    if not isinstance(dependencies, list) or not all(
        isinstance(dependency, str) for dependency in dependencies
    ):
        raise ValueError(
            f"{manifest_path} dependencies.allowed_dependencies must be a string list"
        )
    return set(dependencies)


def validate_first_party_dependencies(
    root: Path, package: str, dependencies: set[str]
) -> None:
    actual = {dependency for dependency in dependencies if is_first_party_dependency(dependency)}
    expected = allowed_dependencies(root, package)
    if actual != expected:
        raise ValueError(
            f"{package} first-party dependency drift: "
            f"expected {sorted(expected)}, found {sorted(actual)}"
        )
