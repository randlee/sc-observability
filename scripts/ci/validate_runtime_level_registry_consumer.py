#!/usr/bin/env python3
"""Validate B.P1 runtime behavior against a staged package set or live crates.io."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tempfile
from pathlib import Path


def run(args: list[str], cwd: Path) -> None:
    subprocess.run(args, cwd=cwd, check=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--mode", choices=("staged", "live"), required=True)
    parser.add_argument("--stage", type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        raise SystemExit("--version must be exact; placeholders are rejected")
    if args.mode == "live" and args.stage:
        raise SystemExit("live mode rejects local stage overrides")
    if args.mode == "staged" and not args.stage:
        raise SystemExit("staged mode requires --stage")

    patches = ""
    provenance = {"mode": args.mode, "version": args.version}
    if args.stage:
        manifest = args.stage / "stage-manifest.json"
        evidence = json.loads(manifest.read_text())
        if evidence["candidate_version"] != args.version:
            raise SystemExit("stage candidate version does not match --version")
        patch_lines = ["[patch.crates-io]"]
        for package in evidence["packages"]:
            patch_lines.append(f'{package["name"]} = {{ path = "{(args.stage / package["staged_root"]).resolve()}" }}')
        patches = "\n".join(patch_lines)
        provenance["stage_manifest"] = str(manifest.resolve())
        provenance["source_commit"] = evidence["source_commit"]

    with tempfile.TemporaryDirectory(prefix="bp2-runtime-consumer-") as temporary:
        root = Path(temporary)
        (root / "Cargo.toml").write_text(
            "[package]\nname = \"bp2-runtime-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n"
            "\n[workspace]\n\n[dependencies]\n"
            f'sc-observability = "={args.version}"\nsc-observability-types = "={args.version}"\n\n{patches}\n'
        )
        (root / "src").mkdir()
        (root / "src/main.rs").write_text(
            "use std::path::PathBuf;\n"
            "use sc_observability::{Logger, LoggerConfig};\n"
            "use sc_observability_types::{LevelChangeSource, LevelFilter, ServiceName};\n"
            "fn main() {\n"
            " let config = LoggerConfig::default_for(ServiceName::new(\"bp2-consumer\").unwrap(), PathBuf::from(\"logs\"));\n"
            " let (logger, mut owner) = Logger::new_with_level_owner(config).unwrap();\n"
            " let changed = owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application).unwrap();\n"
            " assert!(matches!(changed, sc_observability_types::LevelChange::Changed { .. }));\n"
            " owner.reset_level(LevelChangeSource::Application).unwrap();\n"
            " let stopped = logger.shutdown();\n"
            " assert!(matches!(owner.elevate_level(LevelFilter::Debug, LevelChangeSource::Application), Err(sc_observability_types::LevelChangeError::Stopped)));\n"
            " assert_eq!(stopped.level_state().revision, 2);\n}\n"
        )
        # The first isolated resolution creates the fixture lockfile from the
        # declared staged provenance; the second run proves that lock is usable.
        run(["cargo", "run"], root)
        run(["cargo", "run", "--locked"], root)
    print(json.dumps(provenance, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
