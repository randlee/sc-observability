# 1.4.0 qualification candidate

Publication remains separately unauthorized. Phase B stages the six core
packages from one committed qualification source and checks the same archive
checksums on three platforms; its binding inventory covers the additional
DTO/runtime/Tauri/Python/npm artifacts. Phase C must migrate and preflight the
shared `sc-publish` pipeline before separately authorized publication; this
candidate SHA is not a release tag or publication attestation.

## Compatibility

The six core packages add typed error contracts while retaining deprecated
legacy adapters. The log bridge and proc macros establish their initial public
baseline and remain pinned in lockstep (`=1.4.0`). Use explicit bounded flush
and shutdown to establish durability; queue admission alone does not do that.
See `docs/migration-guide.md` and `docs/plans/phase-b/handoff-b-2.md`.

## Public API reference

These versioned links are reserved for the later published artifacts:

- `sc-observability-types`: <https://docs.rs/sc-observability-types/1.4.0/sc_observability_types/>
- `sc-observability`: <https://docs.rs/sc-observability/1.4.0/sc_observability/>
- `sc-observe`: <https://docs.rs/sc-observe/1.4.0/sc_observe/>
- `sc-observability-otlp`: <https://docs.rs/sc-observability-otlp/1.4.0/sc_observability_otlp/>
- `sc-observability-log-macros`: <https://docs.rs/sc-observability-log-macros/1.4.0/sc_observability_log_macros/>
- `sc-observability-log`: <https://docs.rs/sc-observability-log/1.4.0/sc_observability_log/>
