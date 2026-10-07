# sc-observability-dto

Neutral schema-v1 wire values and checked conversions for sc-observability bindings.
Use `decode_event` / `decode_query` before native conversion; `EventStamp` belongs to
the host. Runtime ownership, queue operations, and trusted provenance stamping live
in the native binding runtime. The default dependency closure contains only shared
native types, serde and serde_json. The optional `schema-gen` feature enables the
exactly pinned Schemars tooling dependency.

This crate has been published since 1.4.1. In 1.5.0 it also supplies canonical
v2 typed failure projections while retaining schema-v1 wire envelopes. See the
[Phase F migration guide](../../docs/migration/phase-f.md).
