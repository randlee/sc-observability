# Phase B neutral DTO public API

## Scope

Initial public API of `sc-observability-dto` at version 1.4.0, wire schema v1.
The crate owns the wire declarations and checked conversions listed in
`bindings/API-COVERAGE.md`. This is a scoped TYP-030 exception for wire-only
projections and binding-owned diagnostic literals. Existing core types, native
errors, serialization and ownership remain in their current crates. Runtime-dependent
bridge conversion belongs to `sc-observability-binding-runtime`.

## Approval

Appointed lead aobs reviews the exact exported API digest during B.3 completeness.
The implementation author does not grant independent approval. The machine-readable
record is added only after that actual review; this document grants no publication,
native runtime acceptance, or release approval. B.7 remains the sole publication gate.

## Affected Artifacts

- `crates/sc-observability-dto/`: neutral public data and checked conversions.
- `bindings/schema-generator/`, `bindings/schema/`, `bindings/conformance/`.
- Schema-only generators and their committed TS/Python outputs.
- `release/public-api-policy.json`: explicit new-crate scope with no published baseline.
- Dependency, boundary and missing-docs validators include this crate.
