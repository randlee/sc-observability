# sc-observability-dto

Neutral schema-v1 wire values and checked conversions for sc-observability bindings.
Use `decode_event` / `decode_query` before native conversion; `EventStamp` belongs to
the host. Runtime ownership, queue operations, and trusted provenance stamping live
in the native binding runtime. The default dependency closure contains only shared
native types, serde and serde_json. The optional `schema-gen` feature enables the
exactly pinned Schemars tooling dependency.

These APIs are staged for Phase B and are not published before the B.7 release gate.
