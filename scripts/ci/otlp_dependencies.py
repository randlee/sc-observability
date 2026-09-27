"""Shared OTLP dependency check used by the two existing boundary gates."""

import re
import tomllib
from pathlib import Path


def validate_transport_dependencies(root: Path) -> set[str]:
    def load(path):
        return tomllib.loads((root / path).read_text(encoding="utf-8"))

    workspace = load("Cargo.toml")["workspace"]["dependencies"]
    manifest = load("crates/sc-observability-otlp/Cargo.toml")
    boundary = load("boundaries/sc-observability-otlp/otlp.toml")
    policy = boundary["transport"]
    locked = {(p["name"], p["version"]) for p in load("Cargo.lock")["package"]}
    features = manifest["features"]

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
            raise SystemExit(prefix + "boundary version must be an exact pin")
        if inherited.get("version") != version or (name, version[1:]) not in locked:
            raise SystemExit(prefix + "workspace/lock pin differs from boundary record")
        enabled = set(inherited.get("features", [])) | set(declaration.get("features", []))
        defaults = inherited.get("default-features", True) or declaration.get("default-features", False)
        if enabled != set(rule["features"]) or defaults != rule["default_features"]:
            raise SystemExit(prefix + "effective dependency features differ from boundary record")
        if name in activated("default"):
            raise SystemExit(prefix + "must not be enabled by default")
        for backend in ("otlp-sdk", "legacy-http-json"):
            if (name in activated(backend)) != (backend in rule["backends"]):
                raise SystemExit(prefix + f"incorrect binding to {backend}")
    for name, version in boundary["sdk_transport_lock"].items():
        if (name, version) not in locked:
            raise SystemExit(f"OTLP SDK transport {name}: reviewed lock pin {version} missing")
    return set(policy)
