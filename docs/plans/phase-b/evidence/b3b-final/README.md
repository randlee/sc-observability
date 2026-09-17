# B.3b developer qualification evidence

`index.json` names the source and retained raw artifact hashes. Platform reports
and logs were downloaded from CI run35206736995 at94d891d. Each contains all26
process-isolated cases in debug/release plus public integration and authority
compile-fail fixtures. `ci-run.json` retains the actual job outcomes.

`binding-runtime-consumer.json` contains sandbox policy/probes, dependency and
archive provenance, consumer output, frozen-lock proof and six negative bundle
results. `binding-runtime-bundle-manifest.json` inventories the actual archives
and source files. No publication occurred. `full-runtime-gate.log` is a separate
successful execution of the default script using retained other-platform cells.

`checks.json` and `check-*.log` retain full workspace and structural checks at
296df34 (the same native source, before the source-hash encoding-only CI fix).
`docs-final.log` validates the final handoff. `aggregation-negatives.json` proves
missing/stale/skipped/altered evidence and unconfined consumers are rejected.
`api-export.log` is the actual re-export execution; the approved bytes and digest
are retained in the sibling `b3b-api` directory and scoped approval record.

These are developer qualification artifacts. Independent consolidated QA,
owner-deferred governance and B.7 publication gates remain separate.
