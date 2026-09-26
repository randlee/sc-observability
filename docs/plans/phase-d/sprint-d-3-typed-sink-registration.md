# d-3: Typed sink registration ergonomics (#203)

## Plan metadata

- Wave: 6
- Branch: `sprint/d-3-typed-sink-registration`
- PR target: `sprint/d-2-host-logger-bridge`
- Blocked by: `obs-d-13-sanity`
- Owned paths:
  - `crates/sc-observability/src/builder.rs`
  - `crates/sc-observability/src/sinks.rs`
  - `crates/sc-observability/tests/typed_registration.rs`
  - `docs/logging/d-3-typed-sink-registration.md`
  - `docs/plans/phase-d/sprint-d-3-typed-sink-registration.md`

## Goal

Implement canonical 2.0 typed sink registration using obs-d-13 contracts.

## Deliverables

1. [REQ: LOG-004, LOG-013, LOG-037] Implement `SinkRegistration::typed` in `builder.rs` against
   the canonical open-sink contract, preserving registration metadata and using
   the D.13 signature.

2. [REQ: LOG-004, LOG-013, LOG-037, NFR-012] Implement `LoggerBuilder::register_typed_sink` with
   chaining, typed registration errors, one write/flush per operation, and
   preserved sink health/source diagnostics.

3. [REQ: LOG-004, LOG-013, LOG-037, NFR-012] Own the complete `sinks.rs`
   migration, including existing `LogSink` implementors and all eleven
   error-type call sites, to the canonical 2.0 `LogSinkError` signature; update
   the typed-registration consumer fixtures and documentation.
   `examples/custom-sink-example` remains D.17's owned example.

## This Sprint Does Not Close

D.13 owns staged `typed.rs` definitions. D.18 alone retires transitional
wrappers/adapters and owns public approvals, migration guides, and full logging
integration. D.4 owns the remaining core facade/sink construction-site migration.


## Design

## Implementation contract

Consume obs-d-13 Typed sink signatures and ownership decisions. Both inherent
registration implementations live in `builder.rs`; do not edit `typed.rs` or
invent a competing opaque-error API. D.3 owns the complete `sinks.rs` file for
the canonical `LogSinkError` migration, including existing `LogSink`
implementors and all eleven error-type call sites; D.4 neither owns nor edits
that file. `examples/custom-sink-example` remains D.17's fence. D.18 alone
removes transitional wrappers/adapters during canonical-export activation.

The only file fence is metadata.owned_paths; paths mentioned as dependencies
are read-only unless that metadata grants ownership.

## Handoff from obs-d-13 (wave 1)

Created by obs-d-13, owned here from wave 2. Consume its completed sanity-gated
artifact and preserve the contract while implementing the consumer-side
registration paths. This serial handoff is why relation is must_follow; no
same-wave sibling shares these paths.

- `crates/sc-observability/src/builder.rs`

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-3, owned by obs-d-18 from wave 3; after this bead closes
it makes no further edits. The receiver owns production completion and final
compatibility retirement.

- `crates/sc-observability/src/builder.rs`


## Acceptance criteria

- [ ] `cargo test -p sc-observability --test typed_registration --locked` runs
  both registration entry points with a custom `LogSink` implementation using
  the canonical error signature (D1/D3).
- [ ] boundary:sc-observability — registration metadata, chaining,
  duplicate/invalid/closed failure, one write/flush, health and source identity
  are asserted against the D.13 contract (D2).
- [ ] This sprint does not close the 1.4.1-to-2.0 release migration; obs-d-18
  owns that gate.
