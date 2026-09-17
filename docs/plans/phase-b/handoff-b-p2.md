# B.P2 staged runtime-level release handoff

Candidate version: `1.3.0` (the next minor after the current `1.2.0` release).
The candidate is staged only: no tag, GitHub release, `cargo publish`, npm
publish, or PyPI upload is authorized before B.7 phase-end release work.

## Immutable package inventory and consumer evidence

```sh
python3 scripts/ci/validate_runtime_level_staged_consumer.py --version 1.3.0
```

On 2026-09-16 (America/Los_Angeles), implementation retained an actual stage at
`/Users/randlee/.config/atm/share/sc-obs/bp2-evidence/5347c56-stage/`. Its source
commit is `5347c56efb1d1f62ab0253350ea60078b498ccc8`; stage-manifest SHA-256 is
`45df9d04f55fe1db5dfa385466b8d03b13f875cf8ceac211898be02f60fe624f`.

| Candidate archive | SHA-256 |
| --- | --- |
| `sc-observability-types-1.3.0.crate` | `352a993e75e7f0bf4fc2c5349d72eeb5792b1544e220d694e189e784a960aa8f` |
| `sc-observability-1.3.0.crate` | `10fd9b7f0b02a666dc8400c818904a19818c88fddccebda41eaa3e0d77cda932` |
| `sc-observe-1.3.0.crate` | `6834769a32ee2c741a6ee050cc10a9b1a4a20f5cc0284a158b522ca22d6d581b` |
| `sc-observability-otlp-1.3.0.crate` | `56bfd2a95dca677d4b173559070c17a1cdab6875a57904fe10e329cba4198a59` |

The preparer rejects dirty Git provenance, writes deterministic archive bytes,
checks each archive's normalized `Cargo.toml` and full package inventory, and
the validator rejects any candidate path that is not an extracted archive.
The integrity fix layer additionally verifies each archive SHA-256 and member
inventory against the stage manifest, freshly extracts verified bytes to a
temporary consumer-only directory, and byte-compares the declared extraction
before Cargo resolves it. It rejects altered archives, altered/missing extracted
content, and archive paths escaping the stage root.
`local-macos.json` (SHA-256
`c05d6d581e5ee582e3262f9c7536369bf9677cf4ee78ee7b3e8138f841fa705c`)
records separate exact `1.2.0` registry-baseline and `1.3.0` extracted-candidate
legs. The baseline source is release commit
`dcc52685fd845c8d1bddde29199e799ae921cf5c`; the candidate asserts threshold
filtering, logging/querying, coherent state, reset, shutdown, and stale owner.
The documented command alone was also run; its retained result SHA-256 is
`2d8ccb371611872253a1a4264c2a1942b6cc4342e0b5ea398a69c2c34d847690`.

`scripts/ci/validate_runtime_level_platform_evidence.py` requires passing,
non-skipped `macos.json`, `ubuntu.json`, and `windows.json` with identical
candidate version, source commit, and four archive hashes. The retained shared
`bp2-candidate-stage` artifact and three platform-result artifacts from
[run 35174729184](https://github.com/randlee/sc-observability/actions/runs/35174729184)
passed on macOS, Ubuntu, and Windows; its aggregate-platform-evidence job also
passed. The stage is built once, then the exact bytes are downloaded by every
platform job. This is implementation evidence, not an independent QA approval;
`quality-mgr` owns that verdict.

Contract approval is the `runtime-level-contract.md` blob
`e566d7eba4947abe935c1a0e1a20f4c4fab35864` from commit
`12991c6521d064b733501b9b49a3188e3ce71e1c`, scoped to implementation by aobs
in [`phase-b-runtime-level.md`](../../api-approvals/phase-b-runtime-level.md).
The fixture's state revision `2` is behavior evidence, not a contract revision.
Registry publication and release approval remain deferred to B.7.

For B.7 only, live mode has no `--stage` argument and rejects local overrides:

```sh
python3 scripts/ci/validate_runtime_level_registry_consumer.py \
  --mode live --version 1.3.0
```

Live mode still requires B.7 to retain registry retrieval/index availability and
macOS/Linux/Windows results. No live-mode result is claimed here.
