#!/usr/bin/env python3
"""Run isolated published-baseline and extracted-candidate consumer legs."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import tarfile
import tempfile
from pathlib import Path


BASELINE_VERSION = "1.2.0"
BASELINE_SOURCE_SHA = "dcc52685fd845c8d1bddde29199e799ae921cf5c"
PLATFORM_ASSERTIONS = (
    "baseline_exact_resolution", "candidate_extracted_archive_resolution",
    "threshold_filtering", "log_and_query", "level_state_and_reset", "stale_owner_after_shutdown",
)
EXPECTED_PACKAGES = (
    "sc-observability-types", "sc-observability", "sc-observe", "sc-observability-otlp",
)


def run(command: list[str], cwd: Path) -> None:
    subprocess.run(command, cwd=cwd, check=True)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verified_package(stage: Path, package: dict[str, object], root: Path) -> Path:
    """Verify immutable archive bytes, then consume only a fresh extraction."""
    relative = Path(str(package["archive"]))
    if relative.is_absolute() or ".." in relative.parts:
        raise SystemExit(f"archive path escapes stage: {relative}")
    archive = (stage / relative).resolve()
    if stage not in archive.parents or not archive.is_file() or sha256(archive) != package["archive_sha256"]:
        raise SystemExit(f"archive checksum mismatch: {archive}")
    archive_root = str(package["archive_root"])
    with tarfile.open(archive, "r:gz") as contents:
        names = sorted(member.name for member in contents.getmembers() if member.isfile())
        if names != package["checked_contents"] or any(not name.startswith(f"{archive_root}/") for name in names):
            raise SystemExit(f"archive inventory mismatch: {archive}")
        contents.extractall(root, filter="data")
    extracted = root / archive_root
    declared = stage / str(package["extracted_root"])
    if not declared.is_dir():
        raise SystemExit(f"declared extracted content differs from archive: {declared}")
    for name in names:
        relative = name.removeprefix(f"{archive_root}/")
        declared_file, verified_file = declared / relative, extracted / relative
        if not declared_file.is_file() or declared_file.read_bytes() != verified_file.read_bytes():
            raise SystemExit(f"declared extracted content differs from archive: {declared_file}")
    return extracted.resolve()


def cargo_project(root: Path, version: str, patches: str, source: str) -> None:
    (root / "Cargo.toml").write_text(
        "[package]\nname = \"bp2-runtime-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n"
        "\n[workspace]\n\n[dependencies]\n"
        f'sc-observability = "={version}"\nsc-observability-types = "={version}"\nserde_json = "1"\n\n{patches}\n'
    )
    (root / "src").mkdir()
    (root / "src/main.rs").write_text(source)


def baseline_source() -> str:
    return (
        "use std::path::PathBuf;\nuse sc_observability::{Logger, LoggerConfig};\n"
        "use sc_observability_types::ServiceName;\nfn main() {\n"
        " let config = LoggerConfig::default_for(ServiceName::new(\"bp2-baseline\").unwrap(), PathBuf::from(\"logs\"));\n"
        " let logger = Logger::new(config).unwrap(); let _ = logger.shutdown();\n}\n"
    )


def candidate_source() -> str:
    return (
        "use std::path::PathBuf;\nuse sc_observability::{Logger, LoggerConfig};\n"
        "use sc_observability_types::{ActionName, AdmissionOutcome, Level, LevelChange, LevelChangeError, LevelChangeSource, LevelFilter, LogEvent, LogFieldMatch, LogQuery, ProcessIdentity, SchemaVersion, ServiceName, TargetCategory, Timestamp};\n"
        "use serde_json::json;\n"
        "fn event(service: ServiceName, level: Level, request: &str) -> LogEvent { LogEvent { version: SchemaVersion::new(\"v1\").unwrap(), timestamp: Timestamp::now_utc(), level, service, target: TargetCategory::new(\"bp2.consumer\").unwrap(), action: ActionName::new(\"exercise\").unwrap(), message: Some(request.into()), identity: ProcessIdentity::default(), trace: None, request_id: None, correlation_id: None, outcome: None, diagnostic: None, state_transition: None, fields: serde_json::Map::from_iter([(\"request\".into(), json!(request))]) } }\n"
        "fn main() { let _ = std::fs::remove_dir_all(\"logs\"); let service = ServiceName::new(\"bp2-candidate\").unwrap(); let config = LoggerConfig::default_for(service.clone(), PathBuf::from(\"logs\")); let (logger, mut owner) = Logger::new_with_level_owner(config).unwrap();\n"
        " assert_eq!(logger.try_log_with_outcome(event(service.clone(), Level::Debug, \"filtered\")).unwrap(), AdmissionOutcome::Filtered);\n"
        " assert!(matches!(owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application).unwrap(), LevelChange::Changed { .. })); assert_eq!(logger.level_state().effective_level, LevelFilter::Debug);\n"
        " assert_eq!(logger.try_log_with_outcome(event(service.clone(), Level::Debug, \"accepted\")).unwrap(), AdmissionOutcome::Accepted); logger.flush().unwrap();\n"
        " let snapshot = logger.query(&LogQuery { field_matches: vec![LogFieldMatch::equals(\"request\", json!(\"accepted\"))], ..LogQuery::default() }).unwrap(); assert_eq!(snapshot.events.len(), 1);\n"
        " owner.reset_level(LevelChangeSource::Application).unwrap(); assert_eq!(logger.level_state().effective_level, LevelFilter::Info); let stopped = logger.shutdown(); assert_eq!(stopped.level_state().revision, 2); assert!(matches!(owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application), Err(LevelChangeError::Stopped))); }\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--mode", choices=("staged", "live"), default="staged")
    parser.add_argument("--stage", type=Path)
    parser.add_argument("--result-file", type=Path)
    parser.add_argument("--platform", choices=("macos", "ubuntu", "windows"))
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        raise SystemExit("--version must be exact; placeholders are rejected")
    if args.mode == "live" and args.stage:
        raise SystemExit("live mode rejects local stage overrides")
    if args.mode == "staged" and not args.stage:
        raise SystemExit("staged mode requires extracted --stage (use the staged-consumer wrapper for auto-staging)")

    provenance: dict[str, object] = {"candidate": {"version": args.version, "mode": args.mode}, "baseline": {"version": BASELINE_VERSION, "source_commit": BASELINE_SOURCE_SHA}, "assertions": list(PLATFORM_ASSERTIONS)}
    patches = ""
    verified = None
    if args.stage:
        manifest = args.stage / "stage-manifest.json"
        evidence = json.loads(manifest.read_text())
        if evidence.get("candidate_version") != args.version or evidence.get("schema_version") != 2:
            raise SystemExit("stage manifest does not match candidate version/schema")
        if tuple(item.get("name") for item in evidence.get("packages", [])) != EXPECTED_PACKAGES or any(item.get("version") != args.version for item in evidence["packages"]):
            raise SystemExit("stage package identities or versions are incomplete")
        verified = tempfile.TemporaryDirectory(prefix="bp2-verified-stage-")
        lines = ["[patch.crates-io]"]
        for package in evidence["packages"]:
            extracted = verified_package(args.stage.resolve(), package, Path(verified.name))
            lines.append(f'{package["name"]} = {{ path = "{extracted.as_posix()}" }}')
        patches = "\n".join(lines)
        provenance["candidate"] = {"version": args.version, "mode": args.mode, "stage_manifest": str(manifest.resolve()), "source_commit": evidence["source_commit"], "archives": {item["name"]: item["archive_sha256"] for item in evidence["packages"]}}

    with tempfile.TemporaryDirectory(prefix="bp2-runtime-baseline-") as temporary:
        baseline = Path(temporary)
        cargo_project(baseline, BASELINE_VERSION, "", baseline_source())
        run(["cargo", "run"], baseline)
        run(["cargo", "run", "--locked"], baseline)
    with tempfile.TemporaryDirectory(prefix="bp2-runtime-candidate-") as temporary:
        candidate = Path(temporary)
        cargo_project(candidate, args.version, patches, candidate_source())
        run(["cargo", "run"], candidate)
        run(["cargo", "run", "--locked"], candidate)
    if verified:
        verified.cleanup()
    if args.result_file:
        args.result_file.parent.mkdir(parents=True, exist_ok=True)
        args.result_file.write_text(json.dumps({"status": "passed", "platform": args.platform, **provenance}, indent=2) + "\n")
    print(json.dumps(provenance, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
