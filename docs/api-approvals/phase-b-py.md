# Phase B Python embedding native API

## Scope

Default-feature Rust embedding surface of `sc-observability-py` at version
1.4.0: the six required native type re-exports (`BridgeControlBackend`,
`CoreLoggerBackend`, `CoreLoggerOwner`, `HostLoggingBackend`, `Operation`,
`OperationState`), the `install_host_logger` `Result` API, and the PyO3
module initializer. Independently generated from the refreshed source after
the DTO internal module split, recorded verbatim at
`docs/plans/phase-b/evidence/b4-api/sc-observability-py.txt`.

## Approval

Appointed lead aobs approved the exact exported API digest
The original approval remains dated 2026-09-17T11:00:48Z. The lead-authorized
evidence refresh records digest
`5b41508de3fee743e7ae694754e0d108fb1b630639b88a098838978b5a0bdc36`
at verified source `b868147a20471ff9d65ca010fb9053c6254e57f6`.
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
