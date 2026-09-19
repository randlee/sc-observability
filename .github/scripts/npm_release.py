#!/usr/bin/env python3
"""Build npm archives once; verify and publish immutable release bytes by tag."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import urllib.error
import urllib.parse
import urllib.request

from release_manifest import load_manifest

REGISTRY = "https://registry.npmjs.org"


def packages(manifest):
    entries = manifest.get("npm_packages", [])
    names = set()
    assets = set()
    for entry in entries:
        name, source = entry["name"], Path(entry["source"])
        if not re.fullmatch(r"(?:@[a-z0-9][a-z0-9._-]*/)?[a-z0-9][a-z0-9._-]*", name):
            raise ValueError("invalid npm package name")
        asset = filename(name, "")
        if name in names or asset in assets or source.is_absolute() or ".." in source.parts:
            raise ValueError("duplicate npm package or unsafe source path")
        names.add(name)
        assets.add(asset)
    if bool(entries) != ("npm" in manifest.get("channels", {})):
        raise ValueError("npm_packages and channels.npm must be declared together")
    return entries


def version(tag):
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", tag):
        raise ValueError("expected vX.Y.Z with optional prerelease")
    return tag[1:]


def filename(name, release_version):
    return name.replace("@", "").replace("/", "-") + "-" + release_version + ".tgz"


def metadata(path):
    with tarfile.open(path, "r:gz") as archive:
        contents = archive.getmembers()
        for member in contents:
            parts = Path(member.name).parts
            if not parts or parts[0] != "package" or ".." in parts or not (member.isfile() or member.isdir()):
                raise ValueError("unsafe npm archive member")
        members = [m for m in contents if m.name == "package/package.json"]
        if len(members) != 1 or not members[0].isfile() or members[0].size > 1024 * 1024:
            raise ValueError("archive must contain exactly one regular package/package.json")
        return json.load(archive.extractfile(members[0]))


def check_metadata(path, name, release_version):
    data = metadata(path)
    if data.get("name") != name or data.get("version") != release_version or data.get("private"):
        raise ValueError("npm archive identity/version/private mismatch")
    config = data.get("publishConfig", {})
    if config.get("access", "public") != "public" or "tag" in config:
        raise ValueError("npm publishConfig conflicts with shared public/tag policy")
    if config.get("registry", REGISTRY).rstrip("/") != REGISTRY:
        raise ValueError("npm archive redirects publication to another registry")


def build(manifest, tag, asset_dir):
    entries = packages(manifest)
    if not entries:
        return
    release_version = version(tag)
    asset_dir.mkdir(parents=True, exist_ok=True)
    for entry in entries:
        source = Path(entry["source"]).resolve()
        if not source.is_relative_to(Path.cwd().resolve()):
            raise ValueError("npm source escapes checkout")
        data = json.loads((source / "package.json").read_text())
        if data.get("name") != entry["name"] or data.get("version") != release_version or data.get("private"):
            raise ValueError("source npm package must match release name/version and be public")
        subprocess.run(["npm", "ci"], cwd=source, check=True)
        subprocess.run(["npm", "run", "build", "--if-present"], cwd=source, check=True)
        subprocess.run(["npm", "pack", "--ignore-scripts", "--pack-destination", str(asset_dir.resolve())], cwd=source, check=True)
        check_metadata(asset_dir / filename(entry["name"], release_version), entry["name"], release_version)


def verify(manifest, tag, asset_dir):
    release_version = version(tag)
    checksums = {}
    for line in (asset_dir / "checksums.txt").read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        name = name.lstrip("*")
        if name in checksums or not re.fullmatch("[0-9a-f]{64}", digest):
            raise ValueError("invalid or duplicate release checksum")
        checksums[name] = digest
    verified = []
    for entry in packages(manifest):
        path = asset_dir / filename(entry["name"], release_version)
        if path.is_symlink() or not path.is_file():
            raise ValueError("missing regular npm asset")
        if checksums.get(path.name) != hashlib.sha256(path.read_bytes()).hexdigest():
            raise ValueError("npm asset checksum mismatch")
        check_metadata(path, entry["name"], release_version)
        verified.append((entry["name"], path))
    return verified


def registry_version(name, release_version):
    url = f"{REGISTRY}/{urllib.parse.quote(name, safe='')}/{urllib.parse.quote(release_version, safe='')}"
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            data = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise RuntimeError("npm registry lookup indeterminate") from None
    except (OSError, ValueError):
        raise RuntimeError("npm registry lookup indeterminate") from None
    if data.get("name") != name or data.get("version") != release_version:
        raise ValueError("npm registry identity mismatch")
    return data


def identical(data, path):
    expected = "sha512-" + base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode()
    if data.get("dist", {}).get("integrity") != expected:
        raise ValueError("published npm version differs from release bytes")


def publish(manifest, tag, asset_dir, dry_run=True):
    verified = verify(manifest, tag, asset_dir)  # validate every archive before any write
    release_version = version(tag)
    for name, path in verified:
        existing = registry_version(name, release_version)
        if existing is not None:
            identical(existing, path)
            print(f"already published: {name}@{release_version}")
            continue
        if dry_run:
            print(f"ready: {name}@{release_version}")
            continue
        # No lifecycle scripts, project npmrc, or rebuild in the credentialed leg.
        with tempfile.TemporaryDirectory() as temporary:
            command = ["npm", "publish", str(path.resolve()), "--ignore-scripts", "--access", "public", "--registry", REGISTRY, "--tag", "next" if "-" in release_version else "latest"]
            result = subprocess.run(command, cwd=temporary, capture_output=True)
        if result.returncode:
            # A race or a response lost after acceptance is recoverable only
            # when the registry confirms the exact bytes. Never print npm output.
            existing = registry_version(name, release_version)
            if existing is None:
                raise RuntimeError("npm publication failed; retry this channel by tag")
            identical(existing, path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "verify", "preflight", "publish"))
    parser.add_argument("--manifest", default="release/publish-artifacts.toml")
    parser.add_argument("--tag", required=True)
    parser.add_argument("--asset-dir", type=Path, default=Path("npm-dist"))
    args = parser.parse_args()
    manifest = load_manifest(Path(args.manifest))
    if args.command == "build":
        build(manifest, args.tag, args.asset_dir)
    elif args.command == "verify":
        verify(manifest, args.tag, args.asset_dir)
    else:
        if not packages(manifest):
            raise ValueError("npm channel is not enabled")
        publish(manifest, args.tag, args.asset_dir, dry_run=args.command == "preflight")


if __name__ == "__main__":
    main()
