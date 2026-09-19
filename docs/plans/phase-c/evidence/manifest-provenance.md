# Phase C manifest provenance repair

The metadata correction `aec16d67ccf6fd9c85f3de75ceeb825da6ddbdf1`
adds 22 inherited package metadata fields and workspace authors, and changes the
private consumer-check dependency to inherit the existing versioned bridge edge.
Its parent is `280f39555c18ed41723c179242fdaa6f6e15d23d`.

The separate `../manifest-metadata-adaptations.json` pins exact Git blobs for
all 12 changed manifests at both commits. The validator checks adjacent commits,
exact changed-file scope, actual Git blob bytes, exact live after bytes, and the
narrow allowed lines and parsed TOML delta. Only authors/homepage/repository
inheritance and the consumer dependency conversion are permitted. The consumer
must resolve to the same bridge path, retain dependency options/features, and
inherit exactly the existing workspace version; the bridge must still inherit
that version. No version train change is authorized.

The existing release proof receives verified pre-Phase-C manifest bytes and
still checks the original false-to-true publish transformation against its
original before/after hashes. The new stage then checks that those proven hashes
match its before snapshots before advancing the three imported manifest hashes.
The original import provenance, post-import adaptations, and B.2 release JSON
are unchanged. Final full inventory comparison still rejects runtime drift and
extra files. No runtime exemptions or publication approval are introduced.

The canonical record is detected when release validation runs, so both the
existing import command and package-staging release check use the same chain.
Without the record, the existing exact B.2 checks still reject the later metadata.
A future manifest change needs separately reviewed evidence; this proof does not
automatically accept arbitrary additions. Git history must contain both pins.

## Verification

- Pass 1: 12 new structural/evidence tests, 60 existing import tests, and 19
  staging tests pass. The pre-existing import test expected four QA records;
  it now asserts the exact five already present in the frozen Phase B record,
  including `bridge_queue_full.rs`. No provenance was rewritten to satisfy it.
- Pass 2: the opt-in real historical test clones an isolated local Git view,
  copies actual manifests/imported files, verifies the accepted BTIT history,
  rejects runtime, unrelated manifest, consumer option, workspace version and
  forged-before-blob mutations independently, then passes after restoration.
  Total: 92 tests, including this real historical case and its five mutations.
- `cargo check --locked -p sc-observability-log-consumer-check` passes.
- The exact reported BTIT command passes. The standalone release adaptation
  check also passes. CI runs these regressions in the existing full-history
  import job with its accepted BTIT clone; hosted execution remains pending.

Commands:

```sh
python3 scripts/ci/validate_log_import.py --source-repo /Users/randlee/github/beads-task-issue-tracker --post-import-adaptations docs/plans/phase-b/post-import-adaptations.json --release-adaptations docs/plans/phase-b/release-adaptations-b-2.json
SC_LOG_IMPORT_SOURCE=/Users/randlee/github/beads-task-issue-tracker python3 -m unittest scripts.ci.tests.test_validate_log_import scripts.ci.tests.test_log_metadata_adaptations scripts.ci.tests.test_log_metadata_real_import scripts.ci.tests.test_log_staging
python3 scripts/ci/_log_release_adaptations.py
cargo check --locked -p sc-observability-log-consumer-check
```

Raw stdout/stderr is retained in `manifest-provenance/`. These are local results;
they do not claim release preflight, lint policy, or independent QA acceptance.
