#!/usr/bin/env python3
"""Binding-aware release manifest helper, parallel to .github/scripts/release_artifacts.py.

release_artifacts.py's cmd_validate_manifest hard-requires every crate's
cargo_toml directory to be a root-workspace member. That does not hold for
release/bindings-artifacts.toml: sc-observability-tauri is deliberately its
own standalone Cargo workspace (an empty `[workspace]` table in its own
Cargo.toml, not a member of the root workspace's `members` list), and the
`[[packages]]` entries (pypi/npm) are not Cargo crates at all. This script
implements the equivalent validation for that mixed manifest shape instead of
bending release_artifacts.py to fit it.

No entry in release/bindings-artifacts.toml carries a literal version field.
Versions are always resolved live from each artifact's own manifest
(Cargo.toml's `[package].version`, pyproject.toml's `[project].version`,
package.json's `.version`) at validate/verify time, exactly as
release_artifacts.py already does for the 6-crate manifest.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

COMMAND_TIMEOUT_SECONDS = 900


class ManifestError(SystemExit):
    """Raised (as SystemExit) for any manifest validation failure."""


def load_raw_manifest(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise ManifestError(f"manifest not found: {path}")
    return tomllib.loads(path.read_text(encoding="utf-8"))


def all_entries(data: dict[str, Any]) -> list[dict[str, Any]]:
    """Return every [[crates]] and [[packages]] entry, tagged with its table."""
    entries: list[dict[str, Any]] = []
    for crate in data.get("crates", []):
        entries.append({**crate, "_table": "crates"})
    for package in data.get("packages", []):
        entries.append({**package, "_table": "packages"})
    if not entries:
        raise ManifestError("manifest must define at least one [[crates]] or [[packages]] entry")
    return entries


def workspace_members(workspace_toml: Path) -> set[str]:
    data = tomllib.loads(workspace_toml.read_text(encoding="utf-8"))
    return set(data.get("workspace", {}).get("members", []))


def workspace_version(workspace_toml: Path) -> str:
    data = tomllib.loads(workspace_toml.read_text(encoding="utf-8"))
    return data["workspace"]["package"]["version"]


def cargo_package_name(cargo_toml: Path) -> str:
    data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    return data["package"]["name"]


def cargo_package_version(cargo_toml: Path, workspace_version_value: str) -> str:
    data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    pkg_version = data["package"]["version"]
    if isinstance(pkg_version, str):
        return pkg_version
    if isinstance(pkg_version, dict) and pkg_version.get("workspace") is True:
        return workspace_version_value
    raise ManifestError(f"{cargo_toml}: unsupported version shape: {pkg_version!r}")


def pyproject_name_and_version(pyproject_toml: Path) -> tuple[str, str]:
    data = tomllib.loads(pyproject_toml.read_text(encoding="utf-8"))
    project = data["project"]
    return project["name"], project["version"]


def package_json_name_and_version(package_json: Path) -> tuple[str, str]:
    data = json.loads(package_json.read_text(encoding="utf-8"))
    return data["name"], data["version"]


def _manifest_kind(manifest_path: Path) -> str:
    if manifest_path.name == "pyproject.toml":
        return "pypi"
    if manifest_path.name == "package.json":
        return "npm"
    raise ManifestError(f"unrecognized package manifest file name: {manifest_path}")


def _require_nonempty_str(entry: dict[str, Any], key: str, artifact: str) -> str:
    value = entry.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ManifestError(f"{artifact}: {key!r} must be a non-empty string")
    return value


def validate(manifest_path: Path, workspace_toml: Path) -> tuple[list[dict[str, Any]], int, int]:
    """Run every structural/ordering/dependency check. Returns (entries, ready_count, pending_count)."""
    data = load_raw_manifest(manifest_path)
    entries = all_entries(data)
    members = workspace_members(workspace_toml)

    # Duplicate artifact names across [[crates]] and [[packages]].
    seen_artifacts: set[str] = set()
    for entry in entries:
        artifact = entry.get("artifact")
        if not artifact:
            raise ManifestError(f"entry missing 'artifact' field: {entry}")
        if artifact in seen_artifacts:
            raise ManifestError(f"duplicate artifact: {artifact}")
        seen_artifacts.add(artifact)

    by_artifact = {entry["artifact"]: entry for entry in entries}

    # Duplicate publish_order values (single shared numeric space, see the
    # manifest's own header comment).
    orders: dict[int, str] = {}
    for entry in entries:
        order = entry.get("publish_order")
        if not isinstance(order, int):
            raise ManifestError(f"{entry['artifact']}: publish_order must be an integer")
        if order in orders:
            raise ManifestError(
                f"duplicate publish_order {order}: {orders[order]} and {entry['artifact']}"
            )
        orders[order] = entry["artifact"]

    # depends_on must name defined artifacts, and publish_order must be
    # strictly greater than every dependency's publish_order.
    for entry in entries:
        artifact = entry["artifact"]
        depends_on = entry.get("depends_on", [])
        if not isinstance(depends_on, list):
            raise ManifestError(f"{artifact}: depends_on must be a list")
        for dependency in depends_on:
            if dependency not in by_artifact:
                raise ManifestError(f"{artifact}: depends_on references undefined artifact: {dependency}")
            dep_order = by_artifact[dependency]["publish_order"]
            if entry["publish_order"] <= dep_order:
                raise ManifestError(
                    f"{artifact}: publish_order ({entry['publish_order']}) must be greater than "
                    f"dependency {dependency}'s publish_order ({dep_order})"
                )

    status_values = {"ready", "pending"}
    ready_count = 0
    pending_count = 0

    for entry in entries:
        artifact = entry["artifact"]
        status = entry.get("status")
        if status not in status_values:
            raise ManifestError(f"{artifact}: status must be one of {sorted(status_values)}, got {status!r}")

        if status == "pending":
            _require_nonempty_str(entry, "pending_reason", artifact)
            pending_count += 1
            continue

        ready_count += 1

        if entry["_table"] == "crates":
            cargo_toml = Path(entry["cargo_toml"])
            if not cargo_toml.exists():
                raise ManifestError(f"{artifact}: status=ready but cargo_toml not found: {cargo_toml}")
            actual_name = cargo_package_name(cargo_toml)
            if actual_name != entry["package"]:
                raise ManifestError(
                    f"{artifact}: package name mismatch: manifest={entry['package']!r} actual={actual_name!r}"
                )
            if entry.get("workspace_member") is True:
                crate_dir = str(cargo_toml.parent).replace("\\", "/")
                if crate_dir not in members:
                    raise ManifestError(
                        f"{artifact}: workspace_member=true but {crate_dir!r} is not in root workspace members"
                    )
        else:  # packages
            manifest_file = Path(entry["manifest_path"])
            if not manifest_file.exists():
                raise ManifestError(f"{artifact}: status=ready but manifest_path not found: {manifest_file}")
            kind = entry.get("kind")
            expected_kind = _manifest_kind(manifest_file)
            if kind != expected_kind:
                raise ManifestError(
                    f"{artifact}: kind {kind!r} does not match manifest file type (expected {expected_kind!r})"
                )
            if kind == "pypi":
                actual_name, _ = pyproject_name_and_version(manifest_file)
            else:
                actual_name, _ = package_json_name_and_version(manifest_file)
            if actual_name != entry["package"]:
                raise ManifestError(
                    f"{artifact}: package name mismatch: manifest={entry['package']!r} actual={actual_name!r}"
                )

    return entries, ready_count, pending_count


def check_platform_matrix(entry: dict[str, Any]) -> None:
    """Fail closed if a ready entry declares a platform matrix reference that is

    missing, unreadable, or declares an empty platform/interpreter matrix
    (the "missing-platform" rejection required by B.7/C05). Entries with no
    `platform_matrix_ref` field (e.g. plain crates) are exempt -- this check
    only applies to artifacts that opt into declaring one.
    """
    artifact = entry["artifact"]
    matrix_ref = entry.get("platform_matrix_ref")
    if not matrix_ref:
        return
    matrix_path = Path(matrix_ref)
    if not matrix_path.exists():
        raise ManifestError(
            f"{artifact}: missing-platform rejection: platform_matrix_ref not found: {matrix_ref}"
        )
    policy_ref = _require_nonempty_str(entry, "platform_policy_ref", artifact)
    policy_path = Path(policy_ref)
    if not policy_path.exists():
        raise ManifestError(
            f"{artifact}: missing-platform rejection: platform_policy_ref not found: {policy_ref}"
        )
    try:
        policy = json.loads(policy_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ManifestError(
            f"{artifact}: missing-platform rejection: platform_policy_ref is not valid JSON: "
            f"{policy_ref}: {error}"
        )
    platforms = policy.get("platforms")
    if not isinstance(platforms, list) or not platforms:
        raise ManifestError(
            f"{artifact}: missing-platform rejection: platform_policy_ref {policy_ref} declares "
            "an empty or missing platform matrix"
        )
    interpreters = policy.get("interpreters")
    if not isinstance(interpreters, list) or not interpreters:
        raise ManifestError(
            f"{artifact}: missing-platform rejection: platform_policy_ref {policy_ref} declares "
            "an empty or missing interpreter matrix"
        )


def cmd_validate_manifest(args: argparse.Namespace) -> int:
    entries, ready_count, pending_count = validate(Path(args.manifest), Path(args.workspace_toml))
    for entry in entries:
        if entry["status"] == "ready":
            check_platform_matrix(entry)
    print(
        f"bindings manifest validation passed "
        f"({ready_count} ready, {pending_count} pending, {len(entries)} total entries)"
    )
    return 0


def cmd_list_publish_plan(args: argparse.Namespace) -> int:
    entries, _, _ = validate(Path(args.manifest), Path(args.workspace_toml))
    entries_sorted = sorted(entries, key=lambda item: (item["publish_order"], item["artifact"]))

    only_kind = getattr(args, "only_kind", None)
    if only_kind:
        entries_sorted = [
            entry for entry in entries_sorted
            if ("crate" if entry["_table"] == "crates" else entry["kind"]) == only_kind
        ]

    if args.require_ready:
        not_ready = [entry for entry in entries_sorted if entry["status"] != "ready"]
        if not_ready:
            lines = [
                f"  - {entry['artifact']} (status={entry['status']}): "
                f"{entry.get('pending_reason', 'no reason recorded')}"
                for entry in not_ready
            ]
            raise ManifestError(
                "list-publish-plan --require-ready: refusing to publish, "
                f"{len(not_ready)} non-ready artifact(s) present:\n" + "\n".join(lines)
            )

    if getattr(args, "require_secrets", False):
        blocked = []
        for entry in entries_sorted:
            if entry["status"] != "ready":
                continue
            secret_name = entry.get("registry_secret")
            if not secret_name:
                continue
            if entry.get("registry_secret_configured") is not True:
                blocked.append(f"{entry['artifact']} (needs {secret_name!r})")
        if blocked:
            raise ManifestError(
                "list-publish-plan --require-secrets: blocked-on-auth: "
                f"{len(blocked)} ready artifact(s) need a registry secret that is not yet "
                "owner-deferred (registry_secret_configured is not approved for this candidate). "
                "The sc-publish owner must provision and authorize credentials before publication; "
                "it must not be silently permitted:\n" + "\n".join(f"  - {b}" for b in blocked)
            )

    # Row format (pipe-delimited): kind|package|wait_seconds|status|workspace_member|manifest_path
    #   - kind: "crate" for [[crates]] entries, else the package kind ("pypi"/"npm").
    #   - workspace_member: "true"/"false" -- for crates, whether cargo_toml is a root
    #     workspace member (publish via `-p <package>`) or a standalone manifest that
    #     needs `--manifest-path` (e.g. bindings/tauri, a standalone Cargo workspace).
    #     Always "false" for [[packages]] rows (not applicable).
    #   - manifest_path: cargo_toml for crates, manifest_path for packages.
    for entry in entries_sorted:
        kind = "crate" if entry["_table"] == "crates" else entry["kind"]
        wait_seconds = entry.get("wait_after_publish_seconds", 0)
        workspace_member = entry.get("workspace_member") is True
        manifest_path = entry["cargo_toml"] if entry["_table"] == "crates" else entry["manifest_path"]
        print(
            f"{kind}|{entry['package']}|{wait_seconds}|{entry['status']}|"
            f"{str(workspace_member).lower()}|{manifest_path}"
        )
    return 0


def cmd_verify_versions(args: argparse.Namespace) -> int:
    entries, ready_count, pending_count = validate(Path(args.manifest), Path(args.workspace_toml))
    version = workspace_version(Path(args.workspace_toml))

    checked = 0
    for entry in entries:
        if entry["status"] != "ready":
            continue
        artifact = entry["artifact"]
        if entry["_table"] == "crates":
            actual = cargo_package_version(Path(entry["cargo_toml"]), version)
        else:
            manifest_file = Path(entry["manifest_path"])
            if entry["kind"] == "pypi":
                _, actual = pyproject_name_and_version(manifest_file)
            else:
                _, actual = package_json_name_and_version(manifest_file)
        if actual != version:
            raise ManifestError(
                f"{artifact}: version mismatch: expected workspace version {version!r}, got {actual!r}"
            )
        checked += 1

    print(
        f"bindings version verification passed "
        f"(workspace version={version}, {checked} ready entries checked, {pending_count} pending skipped)"
    )
    return 0


def git_head_commit() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], capture_output=True, text=True,
        timeout=COMMAND_TIMEOUT_SECONDS,
    )
    if result.returncode != 0:
        raise ManifestError(f"git rev-parse HEAD failed: {result.stderr.strip()}")
    return result.stdout.strip()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def content_fingerprint(path: Path) -> str:
    """A sha256 over an archive's (path, content) entries, sorted, ignoring

    container-level metadata such as per-entry mtimes.

    Confirmed empirically (2026-09-17): two back-to-back `maturin sdist`
    invocations against the same commit produce byte-different .tar.gz files
    (the tar header embeds a real build-time mtime for at least one entry)
    even though every extracted file's content is identical. A raw sha256 of
    the compressed artifact is therefore not a reliable changed-byte signal
    for this specific archive format -- it would flag harmless timestamp
    jitter as tampering. This fingerprint is the fix: it hashes each member's
    path and content, sorted by path, so real content changes are still
    caught but timestamp-only rebuild noise is not. `cargo package` .crate
    tarballs and maturin wheels were both observed to already be
    byte-reproducible across back-to-back builds in the same environment,
    but this fingerprint is applied uniformly for robustness rather than
    special-casing the one format observed to need it.
    """
    digest = hashlib.sha256()
    if path.name.endswith((".crate", ".tar.gz")):
        with tarfile.open(path, "r:gz") as tar:
            members = sorted((m for m in tar.getmembers() if m.isfile()), key=lambda m: m.name)
            for member in members:
                digest.update(member.name.encode("utf-8"))
                digest.update(b"\0")
                extracted = tar.extractfile(member)
                if extracted is not None:
                    for chunk in iter(lambda: extracted.read(1 << 20), b""):
                        digest.update(chunk)
                digest.update(b"\0")
    elif path.suffix == ".whl" or path.suffix == ".zip":
        with zipfile.ZipFile(path) as zf:
            for name in sorted(zf.namelist()):
                digest.update(name.encode("utf-8"))
                digest.update(b"\0")
                digest.update(zf.read(name))
                digest.update(b"\0")
    else:
        return sha256_file(path)
    return digest.hexdigest()


def build_crate_artifact(entry: dict[str, Any], dest_dir: Path) -> dict[str, Any]:
    """Build the real distributable .crate tarball for a ready [[crates]] entry.

    Uses `cargo package --exclude-lockfile` rather than a plain `cargo package`.
    A plain `cargo package` regenerates an embedded Cargo.lock by resolving this
    crate's workspace-internal dependencies (e.g. sc-observability-types) against
    the *live* crates.io index -- which fails today with a real "failed to select
    a version" error, because those dependencies are not yet published at this
    candidate workspace version. `--exclude-lockfile` skips that resolution step
    and still produces the real candidate tarball (the same file set cargo would
    upload, respecting Cargo.toml include/exclude), which is what this evidence
    record hashes. This is a deliberate, documented scope limit, not a fabricated
    pass -- see the "note" field below and verify with:
        cargo package --locked --no-verify --manifest-path <cargo_toml>
    which reproduces the same "failed to select a version" error until the
    workspace-internal dependencies are actually live on crates.io.
    """
    artifact = entry["artifact"]
    cargo_toml = Path(entry["cargo_toml"])
    package = entry["package"]
    workspace_member = entry.get("workspace_member") is True

    artifacts_dir = dest_dir / "artifacts"
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    for stale in artifacts_dir.glob(f"{package}-*.crate"):
        stale.unlink()

    with tempfile.TemporaryDirectory(prefix=f"cargo-pkg-{artifact}-") as scratch:
        scratch_dir = Path(scratch)
        cmd = [
            "cargo", "package", "--locked", "--no-verify", "--exclude-lockfile",
            "--target-dir", str(scratch_dir),
        ]
        if workspace_member:
            cmd += ["-p", package]
        else:
            cmd += ["--manifest-path", str(cargo_toml)]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=COMMAND_TIMEOUT_SECONDS)
        if result.returncode != 0:
            raise ManifestError(
                f"{artifact}: cargo package failed (exit {result.returncode}):\n"
                f"{result.stdout}\n{result.stderr}"
            )
        produced_candidates = sorted((scratch_dir / "package").glob(f"{package}-*.crate"))
        if not produced_candidates:
            raise ManifestError(
                f"{artifact}: cargo package produced no .crate file under {scratch_dir / 'package'}"
            )
        produced = produced_candidates[0]
        dest_path = artifacts_dir / produced.name
        shutil.copy2(produced, dest_path)

    digest = sha256_file(dest_path)
    return {
        "table": "crates",
        "build_command": " ".join(cmd),
        "files": {
            "crate": {
                "artifact_path": str(dest_path),
                "sha256": digest,
                "content_sha256": content_fingerprint(dest_path),
                "size_bytes": dest_path.stat().st_size,
            }
        },
        "lockfile_included": False,
        "note": (
            "Cargo.lock intentionally excluded via --exclude-lockfile: one or more "
            "workspace-internal dependencies are not yet published at this candidate "
            "version on crates.io, so cargo package's default lockfile-resolution step "
            "cannot complete today. This hashes the real candidate source-tree bytes "
            "cargo would tar up (respecting Cargo.toml include/exclude), not the final "
            "registry-resolved lockfile a live `cargo publish` will embed once "
            "dependencies are actually live."
        ),
    }


def build_pypi_artifacts(entry: dict[str, Any], dest_dir: Path) -> dict[str, Any]:
    """Build a real sdist and a host-platform wheel for a ready pypi package entry.

    Reuses maturin (the project's existing build backend, per
    scripts/ci/validate_python_bindings.sh) rather than reinventing packaging
    with a separate `python -m build` toolchain.
    """
    artifact = entry["artifact"]
    manifest_path = Path(entry["manifest_path"])
    cargo_manifest = manifest_path.parent / "Cargo.toml"
    if not cargo_manifest.exists():
        raise ManifestError(f"{artifact}: expected sibling Cargo.toml not found: {cargo_manifest}")

    artifacts_dir = dest_dir / "artifacts" / "pypi"
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    for stale in list(artifacts_dir.glob("*.tar.gz")) + list(artifacts_dir.glob("*.whl")):
        stale.unlink()

    sdist_cmd = [
        "uvx", "--from", "maturin==1.10.2", "maturin", "sdist",
        "--manifest-path", str(cargo_manifest), "--out", str(artifacts_dir),
    ]
    sdist_result = subprocess.run(sdist_cmd, capture_output=True, text=True, timeout=COMMAND_TIMEOUT_SECONDS)
    if sdist_result.returncode != 0:
        raise ManifestError(
            f"{artifact}: maturin sdist failed (exit {sdist_result.returncode}):\n"
            f"{sdist_result.stdout}\n{sdist_result.stderr}"
        )
    sdist_candidates = sorted(artifacts_dir.glob("*.tar.gz"))
    if not sdist_candidates:
        raise ManifestError(f"{artifact}: maturin sdist produced no .tar.gz under {artifacts_dir}")
    sdist_path = sdist_candidates[-1]

    wheel_cmd = [
        "uvx", "--from", "maturin==1.10.2", "maturin", "build", "--release", "--locked",
        "--manifest-path", str(cargo_manifest), "--out", str(artifacts_dir),
    ]
    wheel_result = subprocess.run(wheel_cmd, capture_output=True, text=True, timeout=COMMAND_TIMEOUT_SECONDS)
    if wheel_result.returncode != 0:
        raise ManifestError(
            f"{artifact}: maturin build (wheel) failed (exit {wheel_result.returncode}):\n"
            f"{wheel_result.stdout}\n{wheel_result.stderr}"
        )
    wheel_candidates = sorted(artifacts_dir.glob("*.whl"))
    if not wheel_candidates:
        raise ManifestError(f"{artifact}: maturin build produced no .whl under {artifacts_dir}")
    wheel_path = wheel_candidates[-1]

    files = {}
    for kind, path in (("sdist", sdist_path), ("wheel", wheel_path)):
        files[kind] = {
            "artifact_path": str(path),
            "sha256": sha256_file(path),
            "content_sha256": content_fingerprint(path),
            "size_bytes": path.stat().st_size,
        }
    return {
        "table": "packages",
        "kind": "pypi",
        "build_command": " && ".join([" ".join(sdist_cmd), " ".join(wheel_cmd)]),
        "files": files,
        "note": (
            "Wheel built natively for the current host platform/interpreter only. "
            "The full 5-platform x 5-interpreter release matrix is covered separately "
            "by .github/workflows/b4a-python-distributions.yml against "
            "release/python-platform-policy.json; this evidence record proves the "
            "sdist/host-wheel build machinery and content-level reproducibility "
            "(see content_sha256 / content_fingerprint's docstring: the sdist .tar.gz "
            "container itself is not byte-reproducible run-to-run because maturin "
            "embeds a real build-time mtime in at least one tar entry, confirmed "
            "2026-09-17) at this commit only, not the full platform matrix."
        ),
    }


def cmd_build_evidence(args: argparse.Namespace) -> int:
    manifest = Path(args.manifest)
    workspace_toml = Path(args.workspace_toml)
    entries, _, _ = validate(manifest, workspace_toml)
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    version = workspace_version(workspace_toml)
    commit = git_head_commit()
    generated_at = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    artifacts: dict[str, Any] = {}
    for entry in sorted(entries, key=lambda item: (item["publish_order"], item["artifact"])):
        if entry["status"] != "ready":
            continue
        artifact = entry["artifact"]
        if entry["_table"] == "crates":
            artifacts[artifact] = build_crate_artifact(entry, out_dir)
        elif entry["kind"] == "pypi":
            artifacts[artifact] = build_pypi_artifacts(entry, out_dir)
        else:
            # npm remains pending until its owner-authorized publication gate
            # and registry credential are available; build-evidence never
            # builds pending entries.
            continue

    record = {
        "schema_version": 1,
        "source_commit": commit,
        "generated_at": generated_at,
        "candidate_version": version,
        "artifacts": artifacts,
    }
    evidence_path = out_dir / "bindings-candidate.json"
    evidence_path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        f"bindings evidence build passed (commit={commit}, version={version}, "
        f"{len(artifacts)} ready artifact(s) built, evidence={evidence_path})"
    )
    return 0


def cmd_verify_evidence(args: argparse.Namespace) -> int:
    manifest = Path(args.manifest)
    workspace_toml = Path(args.workspace_toml)
    entries, _, _ = validate(manifest, workspace_toml)

    evidence_path = Path(args.evidence)
    if not evidence_path.exists():
        raise ManifestError(f"verify-evidence: evidence file not found: {evidence_path}")
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))

    current_commit = git_head_commit()
    recorded_commit = evidence.get("source_commit")
    if recorded_commit != current_commit:
        raise ManifestError(
            "verify-evidence: wrong-source rejection: evidence.source_commit "
            f"{recorded_commit!r} does not match current HEAD {current_commit!r}. This "
            "evidence record was built from a different commit than the one being "
            "verified; rebuild evidence at the commit you intend to verify."
        )

    evidence_artifacts = evidence.get("artifacts", {})
    ready_entries = [entry for entry in entries if entry["status"] == "ready"]

    checked = 0
    mismatches: list[str] = []
    with tempfile.TemporaryDirectory(prefix="bindings-verify-evidence-") as tmp:
        tmp_dir = Path(tmp)
        for entry in ready_entries:
            artifact = entry["artifact"]
            if entry["_table"] == "packages" and entry["kind"] == "npm":
                continue  # npm is never built by build-evidence; nothing to verify

            recorded = evidence_artifacts.get(artifact)
            if recorded is None:
                raise ManifestError(
                    f"verify-evidence: missing-evidence rejection: {artifact!r} is "
                    "status=ready but has no evidence record; run build-evidence first"
                )

            if entry["_table"] == "crates":
                fresh = build_crate_artifact(entry, tmp_dir / artifact)
            else:
                fresh = build_pypi_artifacts(entry, tmp_dir / artifact)

            recorded_files = recorded.get("files", {})
            for kind, fresh_file in fresh["files"].items():
                recorded_file = recorded_files.get(kind)
                if recorded_file is None:
                    raise ManifestError(
                        f"verify-evidence: missing-evidence rejection: {artifact}/{kind} has "
                        "no recorded file entry in evidence"
                    )
                # Compare content_sha256 (a canonical, per-entry content hash
                # that ignores archive container timestamps) when available,
                # falling back to the raw sha256 otherwise. A raw sha256 of
                # the compressed artifact is not always a reliable
                # changed-byte signal by itself -- see content_fingerprint's
                # docstring for the confirmed maturin sdist non-determinism
                # this was built to tolerate without weakening the check.
                fresh_hash = fresh_file.get("content_sha256", fresh_file["sha256"])
                recorded_hash = recorded_file.get("content_sha256", recorded_file.get("sha256"))
                if fresh_hash != recorded_hash:
                    mismatches.append(
                        f"{artifact}/{kind}: recorded hash={recorded_hash} freshly-built hash={fresh_hash}"
                    )
                else:
                    checked += 1

    if mismatches:
        raise ManifestError(
            "verify-evidence: changed-byte rejection: freshly-built artifact bytes do not "
            "match the evidence record for:\n" + "\n".join(f"  - {m}" for m in mismatches)
        )

    print(
        f"bindings evidence verification passed (commit={current_commit}, "
        f"{checked} artifact file(s) re-hashed and matched, evidence generated_at="
        f"{evidence.get('generated_at')})"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("validate-manifest")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", default="Cargo.toml")
    p.set_defaults(func=cmd_validate_manifest)

    p = sub.add_parser("list-publish-plan")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", default="Cargo.toml")
    p.add_argument("--require-ready", action="store_true")
    p.add_argument(
        "--require-secrets", action="store_true",
        help="fail closed (blocked-on-auth) if a ready entry needs a registry_secret "
             "that is not registry_secret_configured=true",
    )
    p.add_argument(
        "--only-kind", choices=["crate", "pypi", "npm"], default=None,
        help="limit printed rows (and --require-ready/--require-secrets checks) to "
             "entries of this kind, so e.g. a crates-only publish job's --require-secrets "
             "is never blocked by an unrelated pypi/npm package's unconfigured secret",
    )
    p.set_defaults(func=cmd_list_publish_plan)

    p = sub.add_parser("verify-versions")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", required=True)
    p.set_defaults(func=cmd_verify_versions)

    p = sub.add_parser("build-evidence")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", default="Cargo.toml")
    p.add_argument("--out", required=True)
    p.set_defaults(func=cmd_build_evidence)

    p = sub.add_parser("verify-evidence")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace-toml", default="Cargo.toml")
    p.add_argument("--evidence", required=True)
    p.set_defaults(func=cmd_verify_evidence)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
