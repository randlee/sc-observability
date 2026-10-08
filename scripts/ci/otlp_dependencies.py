"""Shared OTLP dependency check used by the two existing boundary gates."""

import re
import tomllib
from pathlib import Path


def effective_features(inherited, declaration) -> tuple[set[str], bool]:
    """Returns the features and default-features Cargo enables for a
    declaration, merging a workspace-inherited entry with local additions."""
    inherited = {"version": inherited} if isinstance(inherited, str) else inherited
    declaration = declaration if isinstance(declaration, dict) else {}
    enabled = set(inherited.get("features", [])) | set(declaration.get("features", []))
    defaults = inherited.get("default-features", True) or declaration.get("default-features", False)
    return enabled, defaults


DEPENDENCY_KINDS = ("dependencies", "dev-dependencies", "build-dependencies")


def dependency_sections(manifest):
    """Yields each dependency table, including target-specific tables."""
    for kind in DEPENDENCY_KINDS:
        yield kind, manifest.get(kind, {})
    for target in manifest.get("target", {}).values():
        for kind in DEPENDENCY_KINDS:
            yield kind, target.get(kind, {})


def resolved_package(key, declaration):
    """Returns the package selected by a Cargo dependency declaration."""
    if isinstance(declaration, dict):
        return declaration.get("package", key)
    return key


def validate_transport_dependencies(root: Path) -> set[str]:
    def load(path):
        return tomllib.loads((root / path).read_text(encoding="utf-8"))

    workspace = load("Cargo.toml")["workspace"]["dependencies"]
    manifest = load("crates/sc-observability-otlp/Cargo.toml")
    document = load("policy/otlp-transport.toml")
    policy = document["transport"]
    locked = {(p["name"], p["version"]) for p in load("Cargo.lock")["package"]}
    features = manifest["features"]

    for kind, dependencies in dependency_sections(manifest):
        if kind == "dev-dependencies":
            continue
        if any(
            resolved_package(key, declaration) == "sc-observe"
            for key, declaration in dependencies.items()
        ):
            raise SystemExit(
                f"OTLP dependency sc-observe: dev-only; it must not appear in [{kind}]"
            )
    # Validate the reviewed transport closure first. A dependency can later become a
    # direct, policy-governed transport (for example tonic for generated OTLP
    # clients); that must not change this invariant's diagnostic or let a
    # missing reviewed transitive pin be masked by a per-transport comparison.
    for name, version in document["sdk_transport_lock"].items():
        if (name, version) not in locked:
            raise SystemExit(f"OTLP transport {name}: reviewed lock pin {version} missing")

    def activated(feature, visited=None):
        visited = set() if visited is None else visited
        if feature in visited:
            return set()
        visited.add(feature)
        result = set()
        for entry in features.get(feature, []):
            if entry.startswith("dep:"):
                result.add(entry[4:])
            elif "/" in entry:
                dependency = entry.split("/", 1)[0]
                if not dependency.endswith("?"):
                    result.add(dependency)
            elif entry in features:
                result.update(activated(entry, visited))
            else:
                result.add(entry)
        return result

    def activates_directly(entry, dependency):
        return entry == f"dep:{dependency}" or entry.split("/", 1)[0] == dependency

    core = manifest["dependencies"].get("sc-observability")
    if not isinstance(core, dict) or core.get("optional") is not True:
        raise SystemExit("OTLP dependency sc-observability: must be optional")
    core_features = sorted(
        feature
        for feature, entries in features.items()
        if any(activates_directly(entry, "sc-observability") for entry in entries)
    )
    if core_features != ["log-sink"]:
        raise SystemExit(
            "OTLP dependency sc-observability: must be activated only by log-sink, "
            f"found {core_features}"
        )

    backends = sorted({backend for rule in policy.values() for backend in rule["backends"]})
    for backend in backends:
        if backend not in features:
            raise SystemExit(f"OTLP transport policy: backend {backend} is not a feature")

    for name, rule in policy.items():
        prefix = f"OTLP transport {name}: "
        declaration = manifest["dependencies"].get(name, {})
        if not isinstance(declaration, dict) or declaration.get("optional") is not True:
            raise SystemExit(prefix + "must be optional")
        if declaration.get("workspace") is not True:
            raise SystemExit(prefix + "must inherit the reviewed workspace pin")
        inherited = workspace[name]
        inherited = {"version": inherited} if isinstance(inherited, str) else inherited
        version = rule["version"]
        if not re.fullmatch(r"=\d+\.\d+\.\d+", version):
            raise SystemExit(prefix + "policy version must be an exact pin")
        package = resolved_package(name, inherited)
        if inherited.get("version") != version or (package, version[1:]) not in locked:
            raise SystemExit(prefix + "workspace/lock pin differs from transport policy")
        enabled, defaults = effective_features(inherited, declaration)
        if enabled != set(rule["features"]) or defaults != rule["default_features"]:
            raise SystemExit(prefix + "effective dependency features differ from transport policy")
        if name in activated("default"):
            raise SystemExit(prefix + "must not be enabled by default")
        for backend in backends:
            if (name in activated(backend)) != (backend in rule["backends"]):
                raise SystemExit(prefix + f"incorrect binding to {backend}")
    return set(policy)
