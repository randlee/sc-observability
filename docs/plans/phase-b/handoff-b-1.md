# B.1 copy execution evidence

## Source and acceptance

Copied the accepted BTIT commit `396a9d9f77ca1950eeb92d4f88c0eecadb5ef00b`
(`docs/plans/phase-b/handoff-b-p3.md` at `27ce64dc553c95de3699c4fe9afa26b0a060642e`),
covering `crates/sc-observability-log/`, `crates/sc-observability-log-macros/`,
and `crates/sc-observability-log-consumer-check/` in full (Rust source, docs,
tests, and UI fixtures) -- 85 files. Every file's destination Git blob ID was
independently re-derived and compared against the source repository's blob ID
for the same path at the accepted commit: 0 mismatches before any adaptation
was applied.

## Mechanical adaptations

Recorded in `docs/plans/phase-b/import-provenance.json`, verified by
`scripts/ci/validate_log_import.py --source-repo`:

- `package_metadata` (3 files, one per copied crate's `Cargo.toml`): converted
  hard-coded `version`/`edition`/`rust-version`/`license` to this workspace's
  `.workspace = true` inherited form and added `repository.workspace = true`,
  per `sprint-b-1-copy.md`'s boundary sample. `name`, `description`, and
  `publish = false` are unchanged.
- `trybuild_diagnostic_text` (4 files, all under
  `crates/sc-observability-log/tests/ui/*.stderr`): BTIT's
  `rust-toolchain.toml` pins channel `1.98.1`; this workspace pins `1.94.1`.
  Regenerated once with `TRYBUILD=overwrite` against the destination-pinned
  toolchain. In every case the diagnostic code and `-->` source location are
  unchanged; only rustc's pretty-printer wording, type-path qualification, or
  amount of surrounding source context differs between the two rustc
  releases. The underlying `.rs` UI-test sources are byte-identical to the
  accepted source. See the "Recorded disposition" section of
  `sprint-b-1-copy.md` for the full accepted disposition.

No other content adaptation was needed: the macros crate's dependency on the
bridge (`sc-observability-log-macros.workspace = true`) and the
consumer-check crate's relative path dependency
(`sc-observability-log = { path = "../sc-observability-log" }`) already
resolve correctly under the destination's identical relative directory
layout, so neither required a `dependency_path` relocation.

## Workspace/CI wiring

- Root `Cargo.toml`: added the three crates to `[workspace.members]` and to
  `[workspace.dependencies]` (`sc-observability-log-macros` pinned
  `=1.2.0`, an exact pin against this workspace's shared version, per the
  sprint doc's "exact `=V` version plus workspace path" requirement -- this is
  a hard-coded literal, not self-maintaining: nothing in this repo's
  automation currently re-syncs it when `workspace.package.version` bumps, so
  a future version bump must update this pin by hand); added the crates' third-party
  dependencies (`log`, `hostname`, `tempfile`, `trybuild`, `tracing`, `tokio`,
  `syn`, `quote`, `proc-macro2`) to `[workspace.dependencies]` at the exact
  version/feature sets BTIT's own workspace used.
- `scripts/ci/validate_docs_consistency.sh`: extended the rustdoc
  missing-docs loop to `sc-observability-log` and `sc-observability-log-macros`
  (both already pass `-D missing-docs` with no source changes;
  `sc-observability-log-consumer-check` is a private compile-only crate with
  no documented public surface, intentionally excluded).
- `scripts/ci/validate_repo_boundaries.sh`: extended the ATM-coupling/
  home-dir-discovery `shared_crate_roots` scan to the three new crates; added
  explicit checks that `sc-observability-log-macros` does not depend back on
  `sc-observability-log`, and that none of the four existing core crates
  depends on `log` or on any of the three copied crates (sprint doc AC3).
- `docs/architecture.md`: added Crate Boundary Table rows for the three
  copied crates, including the scoped TYP-030 companion exception
  (`InitError`/`FlushError`/`ShutdownError`, already accepted in the locked
  `target-bridge-api.md` and `docs/requirements.md` PHB-002/ADR-011 -- no
  further requirements.md edit was needed since that exception was already
  documented and accepted as part of the target contract entry gate).
- `docs/api-approvals/phase-b-log-import.md`: created with
  Scope/Approval/Affected Artifacts, recording that all three crates retain
  `publish = false` so they contribute zero lines to
  `validate_public_api_diff.sh`/`validate_public_api_docs.sh`. Running the
  whole command is expected to, and does, exit 1 with a nonempty additive
  diff against the four published core crates, inherited from the
  already-merged B.1a/B.1b/B.1c typed-API preparation layers underneath this
  branch -- not introduced by this copy, and not a "no diff"/PASS state.
  `validate_public_api_docs.sh` separately passes (rustdoc coverage only).

## Validation (clean at HEAD)

```text
python3 scripts/ci/validate_log_import.py --source-repo /Users/randlee/github/beads-task-issue-tracker
  -> B.1 import provenance, BTIT acceptance handoff, and copied inventory are coherent
python3 scripts/ci/tests/test_validate_log_import.py -v
  -> Ran 47 tests in 8.3s, OK (41 pre-existing + 6 new: package_metadata workspace-inheritance
     acceptance, trybuild_diagnostic_text acceptance, non-.stderr-file rejection,
     wrong-error-code rejection, wrong-location rejection, empty-profile rejection)
cargo fmt --all -- --check                                        -> clean
cargo test --locked --workspace --all-targets                     -> all green, 0 failed
cargo test --locked --workspace --doc                              -> all green, 0 failed
cargo clippy --locked --workspace --all-targets -- -D warnings     -> clean
cargo check --locked -p sc-observability-log-consumer-check        -> clean
bash scripts/ci/validate_dependency_bans.sh                        -> dependency ban validation passed
bash scripts/ci/validate_repo_boundaries.sh                        -> repo boundary validation passed
bash scripts/ci/validate_docs_consistency.sh                       -> docs consistency validation passed;
                                                                       rustdoc missing-docs validation passed
bash scripts/ci/validate_public_api_diff.sh                        -> exit 1: additive-only diff for the
                                                                       four published core crates, inherited
                                                                       from already-merged B.1a/B.1b/B.1c
                                                                       typed-API prep (not introduced by
                                                                       this copy; the three new crates stay
                                                                       publish=false and contribute nothing)
bash scripts/ci/validate_public_api_docs.sh                        -> public API docs validation passed
cargo test --locked -p sc-observability-log --all-targets \
  --features test_hooks                                            -> all green, 0 failed (55+ tests
                                                                       across unit/doc/UI-trybuild targets)
cargo test --locked -p sc-observability-log --release \
  --features static_level_cap_test --test static_level_cap         -> 1 passed, 0 failed
  (capped_release_rejects_trace_before_install_then_allows_info)
cargo test --locked -p sc-observability-log --release \
  --test runtime_level_bridge                                      -> 1 passed, 0 failed
  (direct_facade_and_macro_admission_share_the_core_level_owner)
python3 scripts/ci/validate_runtime_level_qualification_metadata.py -> B.P2 qualification metadata and
                                                                       roster are coherent
python3 scripts/ci/tests/test_validate_runtime_level_qualification_metadata.py -v
  -> Ran 10 tests, OK (3 pre-existing + 7 new WorkspaceMemberRosterTests)
```

All three of the above extra commands were run under this workspace's pinned
1.94.1 toolchain, with no Rust source changes, in response to aobs
completeness findings B1-C02 (feature-gated regression coverage) and B1-C03
(CI qualification workspace-member roster) on `phase-b-b1-copy`.

## Imported regression gate scope (B1-C02)

- `sc-observability-log`'s two opt-in Cargo features (`test_hooks`,
  `static_level_cap_test`) are exercised above under the destination's pinned
  toolchain; `static_level_cap_test` requires `--release` since it enables
  `log/release_max_level_info`, which only takes effect in the release
  profile. Neither feature was previously exercised in this worktree's
  evidence before this fix round.
- Three-platform CI proof requirement: `.github/workflows/ci.yml`'s `test`
  job (matrix `ubuntu-latest`/`macos-latest`/`windows-latest`) now runs three
  additional steps after `cargo test --workspace`: `cargo test -p
  sc-observability-log --all-targets --features test_hooks`, `cargo test -p
  sc-observability-log --release --features static_level_cap_test --test
  static_level_cap`, and `cargo test --locked -p sc-observability-log
  --release --test runtime_level_bridge`, so the opt-in features and the
  release-mode runtime-level fixture are all exercised on all three
  platforms, not just locally. This closes the previously-open CI-coverage
  gap for these commands.
- Exported API/impl inventory reconciliation against the approved
  `target-bridge-api.md` disposition matrix: see
  `docs/plans/phase-b/log-import-export-report.md`, generated from
  `cargo public-api` against both copied library crates and checked
  row-by-row against both of `target-bridge-api.md`'s disposition tables.

The final parent merge-forward is `0c31d7a133bf85b20be123261b23020722c7ccaf`
(`feature/phase-b-1c-observation-prep`'s
closeout of B.1c preparation, itself built on the tested `5530b37` checkpoint
and the `77d28c7` logger-prep final handoff); no lower layer was edited.

## B.P2 staged package build fix (B1-C03)

PR124 run `35197298387` failed inside `prepare_runtime_level_staged_packages.py`
at `cargo package --list -p sc-observability-types` (exit 101). Reproduced
locally: `prepare_runtime_level_staged_packages.py` advances every workspace
dependency's pinned version to the candidate version by literal
`version = "1.2.0", path =` text substitution in the copied root
`Cargo.toml`, but `sc-observability-log-macros`'s dependency is an exact
`=1.2.0` pin (`version = "=1.2.0", path =`), which that substitution did not
match. The staged `sc-observability-log-macros` crate itself advanced to the
candidate version (via `version.workspace = true`), while
`sc-observability-log`'s dependency on it stayed pinned at exactly `1.2.0`
-- a version that no longer existed in the staged tree, so Cargo's resolver
failed with "candidate versions found which didn't match". Fixed by also
advancing the exact-pin form to `version = "={candidate}", path =`,
preserving the pin's exactness. `checked_run()` in the same script also
previously relied on `subprocess.run(check=True)`, whose `CalledProcessError`
swallows captured stderr in the propagating traceback; it now checks the
return code explicitly and raises with the command and its stderr attached,
so this diagnosis did not require re-running cargo by hand.

Raw local evidence at `0d1da6e4ba36b643169067ff60599ed35a7fdd7a` (this fix
round's final commit before this doc update), candidate version `1.3.0`
(current `runtime-level-qualification.toml` candidate):

```text
python3 scripts/ci/prepare_runtime_level_staged_packages.py --version 1.3.0 --output /tmp/bp2-stage-final
  -> /private/tmp/bp2-stage-final/stage-manifest.json (exit 0)
  stage-manifest.json source_commit: 0d1da6e4ba36b643169067ff60599ed35a7fdd7a
  stage-manifest.json packages: sc-observability-types, sc-observability, sc-observe, sc-observability-otlp
  (exactly the 4 B.P2 roster archives; no unpublished companion crate present)

python3 scripts/ci/validate_runtime_level_staged_consumer.py --version 1.3.0 \
  --stage /tmp/bp2-stage-final --platform macos --result-file /tmp/bp2-evidence-final/macos.json
  -> exit 0
  result: {"assertions": ["baseline_exact_resolution", "candidate_extracted_archive_resolution",
    "threshold_filtering", "log_and_query", "level_state_and_reset", "stale_owner_after_shutdown"],
    "baseline": {"source_commit": "dcc52685fd845c8d1bddde29199e799ae921cf5c", "version": "1.2.0"},
    "candidate": {"mode": "staged", "source_commit": "0d1da6e4ba36b643169067ff60599ed35a7fdd7a",
    "version": "1.3.0", "stage_manifest": "/private/tmp/bp2-stage-final/stage-manifest.json"}}
  (all 6 declared assertions present; ubuntu/windows platform runs require
  their own CI runners and are not reproducible on this macOS worktree, but
  the underlying staged-consumer logic is platform-independent Rust/cargo
  invoked identically per platform by the workflow matrix)
```

`release/publish-artifacts.toml` and the staged four-package roster/order are
unchanged; no unpublished companion crate is added to that roster.

## Not in scope here

No publish, no BTIT dependency switch, no runtime-contract closure (Phase B's
runtime-level contract remains owner-deferred), and no claim that this copy
resolves any BTIT-side review finding. Independent QA and API-approval
reviewer sign-off remain pending.
