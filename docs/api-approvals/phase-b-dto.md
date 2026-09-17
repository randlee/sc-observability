# Phase B neutral DTO public API

## Scope

Initial public API of `sc-observability-dto` at version 1.4.0, wire schema v1.
The crate owns the wire declarations and checked conversions listed in
`bindings/API-COVERAGE.md`. This is a scoped TYP-030 exception for wire-only
projections and binding-owned diagnostic literals. Existing core types, native
errors, serialization and ownership remain in their current crates. Runtime-dependent
bridge conversion belongs to `sc-observability-binding-runtime`.

## Approval

Appointed lead aobs approved the exact exported API digest
`c03027422ae3ff9f49e5b425523c9c5eab1909e34bfac5dc39ac54295f89cc9a`
on 2026-09-17 at source `6481ac7ea4b321cab76f27a246ed84c5ab6af75f`.
The actual lead-authored record is retained verbatim in `phase-b-dto.json`.
This grants no behavior QA, native runtime acceptance, or publication approval.
B.7 remains the sole publication gate.

## Affected Artifacts

- `crates/sc-observability-dto/`: neutral public data and checked conversions.
- `bindings/schema-generator/`, `bindings/schema/`, `bindings/conformance/`.
- Schema-only generators and their committed TS/Python outputs.
- `release/public-api-policy.json`: explicit new-crate scope with no published baseline.
- Dependency, boundary and missing-docs validators include this crate.
