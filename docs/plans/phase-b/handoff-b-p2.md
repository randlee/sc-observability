# B.P2 staged runtime-level release handoff

Candidate version: `1.3.0` (the next minor after the current `1.2.0` release).
The candidate is staged only: no tag, GitHub release, `cargo publish`, npm
publish, or PyPI upload is authorized before B.7 phase-end release work.

## Immutable package inventory and consumer evidence

```sh
python3 scripts/ci/validate_runtime_level_staged_consumer.py --version 1.3.0
```

On 2026-09-16 (America/Los_Angeles), implementation retained an actual stage at
`/Users/randlee/.config/atm/share/sc-obs/bp2-evidence/5dded-stage/`. Its source
commit is `487a3597230120f4e8317d4b11cfef5b1d6e306f`; stage-manifest SHA-256 is
`7f6def0156b9a23bc47f0b2de471f0d3605949c3c15b36c44b2d6d8e6feb8141`.

| Candidate archive | SHA-256 |
| --- | --- |
| `sc-observability-types-1.3.0.crate` | `352a993e75e7f0bf4fc2c5349d72eeb5792b1544e220d694e189e784a960aa8f` |
| `sc-observability-1.3.0.crate` | `10fd9b7f0b02a666dc8400c818904a19818c88fddccebda41eaa3e0d77cda932` |
| `sc-observe-1.3.0.crate` | `6834769a32ee2c741a6ee050cc10a9b1a4a20f5cc0284a158b522ca22d6d581b` |
| `sc-observability-otlp-1.3.0.crate` | `56bfd2a95dca677d4b173559070c17a1cdab6875a57904fe10e329cba4198a59` |

The preparer rejects dirty Git provenance, writes deterministic archive bytes,
checks each archive's normalized `Cargo.toml` and full package inventory, and
the validator rejects any candidate path that is not an extracted archive.
`local.json` (SHA-256
`762c8da6d027707d9fd5b507b9f723136a234ccc4f2975382a2c44048057c3a5`)
records separate exact `1.2.0` registry-baseline and `1.3.0` extracted-candidate
legs. The baseline source is release commit
`dcc52685fd845c8d1bddde29199e799ae921cf5c`; the candidate asserts threshold
filtering, logging/querying, coherent state, reset, shutdown, and stale owner.
The documented command alone was also run; its retained result SHA-256 is
`2d8ccb371611872253a1a4264c2a1942b6cc4342e0b5ea398a69c2c34d847690`.

`scripts/ci/validate_runtime_level_platform_evidence.py` requires passing,
non-skipped `macos.json`, `ubuntu.json`, and `windows.json`. The
`B.P2 staged consumer` workflow produces and retains these artifacts before the
aggregate check. They have not yet been produced by that workflow, so this is
an explicit **platform gate unavailable/pending**, not an implementation or QA
approval. `quality-mgr` owns independent QA and any PASS verdict.

Contract revision: B.P1 runtime-level contract revision `2` (elevate then reset)
is asserted by the candidate fixture. Approval: staged implementation evidence
only; registry publication and release approval remain deferred to B.7.

For B.7 only, live mode has no `--stage` argument and rejects local overrides:

```sh
python3 scripts/ci/validate_runtime_level_registry_consumer.py \
  --mode live --version 1.3.0
```

Live mode still requires B.7 to retain registry retrieval/index availability and
macOS/Linux/Windows results. No live-mode result is claimed here.
