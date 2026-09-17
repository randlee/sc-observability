# B.P2 staged runtime-level release handoff

Candidate version: `1.3.0` (the next minor after the current `1.2.0` release).
The candidate is staged only: no tag, GitHub release, `cargo publish`, npm
publish, or PyPI upload is authorized before B.7 phase-end release work.

## Package and consumer commands

```sh
python3 scripts/ci/prepare_runtime_level_staged_packages.py \
  --version 1.3.0 --output "$STAGE_DIR"
python3 scripts/ci/validate_runtime_level_registry_consumer.py \
  --mode staged --version 1.3.0 --stage "$STAGE_DIR"
```

`stage-manifest.json` records the source SHA, candidate version, staged package
roots, tree SHA-256 checksums, and Cargo package file lists. The staged consumer
uses explicit `[patch.crates-io]` entries to those isolated package roots; that provenance is
printed by the validator and must be retained with platform runs. It proves the
candidate package set, not live registry availability.

For B.7 only, live mode has no `--stage` argument and rejects local overrides:

```sh
python3 scripts/ci/validate_runtime_level_registry_consumer.py \
  --mode live --version 1.3.0
```

Live mode still requires B.7 to retain registry retrieval/index availability and
macOS/Linux/Windows results. `quality-mgr` owns the independent QA/platform
record; this handoff records implementation entry points only.
