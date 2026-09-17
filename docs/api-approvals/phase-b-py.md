# Phase B Python embedding native API

## Scope

Default-feature Rust embedding surface of `sc-observability-py` at version
1.4.0: the six required native type re-exports (`BridgeControlBackend`,
`CoreLoggerBackend`, `CoreLoggerOwner`, `HostLoggingBackend`, `Operation`,
`OperationState`), the `install_host_logger` `Result` API, and the PyO3
module initializer. Independently generated from a pinned git archive at
source commit `a69a54a5abb750f07f21ad79bcb9cdb5562f6b01`, recorded verbatim
at `docs/plans/phase-b/evidence/b4-api/sc-observability-py.txt`.

## Approval

Appointed lead aobs approved the exact exported API digest
`d4d01640af4a8a7a980fd86df7b4a844ad65c1fa66f5c364a4ce930f91bd12fd`
on 2026-09-17T11:00:48Z at source `a69a54a5abb750f07f21ad79bcb9cdb5562f6b01`.
The actual lead-authored record is retained verbatim in `phase-b-py.json`.
This grants no runtime behavior QA, deferred owner runtime-contract
acceptance, or publication approval. B.7 remains the sole publication gate,
and this approval is recheckable: any subsequent change to
`sc-observability-py`'s public API produces a different digest and requires
renewed review before `scripts/ci/validate_public_api_docs.sh` will pass
again for this crate.

## Affected Artifacts

- `bindings/python/sc-observability-py/src/lib.rs`: the PyO3 native module
  (`_native`) and its public re-exports.
- `crates/sc-observability-binding-runtime/`, `crates/sc-observability-dto/`:
  the types re-exported/consumed by this surface.
- `release/public-api-policy.json`: `sc-observability-py` entry (initial
  public API, no published baseline).
- `release/bindings-artifacts.toml`: the `sc-observability-py` crate entry.
