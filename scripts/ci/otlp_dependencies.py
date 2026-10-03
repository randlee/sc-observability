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


def validate_transport_dependencies(root: Path) -> set[str]:
    def load(path):
        return tomllib.loads((root / path).read_text(encoding="utf-8"))

    workspace = load("Cargo.toml")["workspace"]["dependencies"]
    manifest = load("crates/sc-observability-otlp/Cargo.toml")
    document = load("policy/otlp-transport.toml")
    policy = document["transport"]
    locked = {(p["name"], p["version"]) for p in load("Cargo.lock")["package"]}
    features = manifest["features"]

    # ADR-019 amendment: the hermetic integration collector adds tonic's
    # generated-service router only as a dev-dependency.  The policy records
    # every allowed dev-dependency and its exact effective features.
    reviewed_dev = document["dev_dependencies"]
    dev = set()
    for kind, dependencies in dependency_sections(manifest):
        if kind != "dev-dependencies" and "sc-observe" in dependencies:
            raise SystemExit(
                f"OTLP dependency sc-observe: dev-only; it must not appear in [{kind}]"
            )
    for kind, dependencies in dependency_sections(manifest):
        if kind != "dev-dependencies":
            continue
        for key, value in dependencies.items():
            if not isinstance(value, dict) or value.get("workspace") is not True or "package" in value:
                raise SystemExit(f"OTLP dev-dependency {key}: must inherit the reviewed workspace pin")
            dev.add(key)
            if key in reviewed_dev:
                enabled, defaults = effective_features(workspace.get(key, {}), value)
                expected = reviewed_dev[key]
                if enabled != set(expected["features"]) or defaults != expected["default_features"]:
                    raise SystemExit(f"OTLP dev-dependency {key}: effective features differ from policy")
    if dev != set(reviewed_dev):
        raise SystemExit(
            "OTLP dev-dependencies differ from policy: "
            f"unexpected {sorted(dev - set(reviewed_dev))}, "
            f"missing {sorted(set(reviewed_dev) - dev)}"
        )

    # Validate the reviewed SDK closure first. A dependency can later become a
    # direct, policy-governed transport (for example tonic for generated OTLP
    # clients); that must not change this invariant's diagnostic or let a
    # missing reviewed transitive pin be masked by a per-transport comparison.
    for name, version in document["sdk_transport_lock"].items():
        if (name, version) not in locked:
            raise SystemExit(f"OTLP SDK transport {name}: reviewed lock pin {version} missing")

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
        if inherited.get("version") != version or (name, version[1:]) not in locked:
            raise SystemExit(prefix + "workspace/lock pin differs from transport policy")
        enabled, defaults = effective_features(inherited, declaration)
        if enabled != set(rule["features"]) or defaults != rule["default_features"]:
            raise SystemExit(prefix + "effective dependency features differ from transport policy")
        if name in activated("default"):
            raise SystemExit(prefix + "must not be enabled by default")
        for backend in ("otlp-sdk", "sync-http"):
            if (name in activated(backend)) != (backend in rule["backends"]):
                raise SystemExit(prefix + f"incorrect binding to {backend}")
    return set(policy)


def validate_composition_harness(root: Path) -> None:
    """Enforces the ADR-019/ADR-020 composition-harness dependency exception."""

    def load(path):
        return tomllib.loads((root / path).read_text(encoding="utf-8"))

    rule = load("policy/otlp-transport.toml")["composition_harness"]
    workspace = load("Cargo.toml")["workspace"]
    shared = workspace.get("dependencies", {})
    harness_dir = (root / rule["manifest"]).parent.resolve()
    harness = load(rule["manifest"])
    prefix = "OTLP composition harness: "

    def resolve(key, declaration, manifest_dir):
        # Renamed (`package = ...`), path and workspace-inherited declarations
        # all resolve to the package and path Cargo actually uses.
        declaration = declaration if isinstance(declaration, dict) else {}
        base = manifest_dir
        if declaration.get("workspace") is True:
            inherited = shared.get(key, {})
            declaration = {**(inherited if isinstance(inherited, dict) else {}), **declaration}
            base = root
        path = declaration.get("path")
        return declaration.get("package", key), (base / path).resolve() if path else None

    if harness.get("package", {}).get("name") != rule["package"]:
        raise SystemExit(prefix + f"{rule['manifest']} must declare package {rule['package']}")
    if harness["package"].get("publish") is not False:
        raise SystemExit(prefix + "must set publish = false")
    reviewed = rule["dev_dependencies"]
    dev = set()
    for kind, dependencies in dependency_sections(harness):
        if kind != "dev-dependencies" and dependencies:
            raise SystemExit(prefix + f"must not declare {kind}")
        for key, value in dependencies.items():
            if not isinstance(value, dict) or value.get("workspace") is not True:
                raise SystemExit(prefix + f"{key} must inherit the reviewed workspace pin")
            package = resolve(key, value, harness_dir)[0]
            dev.add(package)
            if package in reviewed:
                enabled, defaults = effective_features(shared.get(key, {}), value)
                expected = reviewed[package]
                if enabled != set(expected["features"]) or defaults != expected["default_features"]:
                    raise SystemExit(prefix + f"{package} effective features differ from policy")
    if dev != set(reviewed):
        raise SystemExit(
            prefix + "dev-dependencies differ from policy: "
            f"unexpected {sorted(dev - set(reviewed))}, "
            f"missing {sorted(set(reviewed) - dev)}"
        )
    for member in workspace["members"]:
        member_dir = (root / member).resolve()
        if member_dir == harness_dir:
            continue
        for _, dependencies in dependency_sections(load(f"{member}/Cargo.toml")):
            for key, value in dependencies.items():
                package, path = resolve(key, value, member_dir)
                if package == rule["package"] or path == harness_dir:
                    raise SystemExit(prefix + f"{member} must not depend on the harness")
