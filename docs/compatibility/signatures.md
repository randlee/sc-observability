# Compatibility contract registry

`registry.json` is the one machine-readable continuation of the D18 recovery
audit. It is pinned to `v1.4.1` / `c578912653233c7dc678fefe5af575118dbbaaa1`
and contains an individual record for each of the 58 affected nominal public
contracts, 141 affected inherent/free callables, and 12 public trait slots.
It is not an estimate of forwarding bodies or a replacement API ledger.

## Row contract

Every nominal, callable, and trait-slot record carries these fields:

| Field | Meaning |
| --- | --- |
| `baseline_signature` | Exact released declaration extracted from the pinned baseline; nominal rows retain the released public identity. |
| `canonical_signature` | Exact declaration at the selected head, or `null` when the released method no longer exists and must be restored by its named facade owner. |
| `obligations` | Identity, open-trait/Send+Sync, source/code, or wire-shape constraint that cannot be lost in conversion. |
| `treatment` | One of `unchanged_alias`, `existing_pair`, `new_adapter`, or `restoration`; this is a four-way implementation decision, not a progress label. |
| `conversion` | The permitted conversion direction. Canonical implementation remains independent of compat. |
| `baseline_source` / `canonical_source` | Pinned source evidence for the declaration. |
| `removable_paths` | Current root/compat source path only when there is code to remove; an empty list means the future facade is not invented here. |

`null` canonical signatures are intentional evidence: a D22 record may name a
removed released method, but D22 must not fabricate the D23–D26 facade wrapper
that will restore it. The record's baseline signature, obligations, conversion
direction, and future owner make that gap explicit.

## Audited callable coverage

The registry has one record for every member below. The count is the pinned
source audit: 139 inherent methods plus the two affected free functions. Exact
declarations and source references are in `method_contracts`, not summarized
or inferred by this table.

| Area | Owner | Methods | Count |
| --- | --- | --- | ---: |
| shared | `SpanRecord` | `new`, `with_diagnostic`, `end`, `timestamp`, `service`, `name`, `trace`, `status`, `diagnostic`, `attributes`, `duration_ms` | 11 |
| shared | `SubscriberRegistration` / `ProjectionRegistration` | registration constructors, filters, projectors, and `into_parts` | 9 |
| core | `Logger` / `LoggerBuilder` | construction, admission, typed helpers, query/follow, lifecycle | 27 |
| core | config, registration, reader/follow, fault injector, concrete sinks | all released inherent methods | 17 |
| bridge | `LogGuard`, `LogControl`, local errors, `EmitError` | control/lifecycle and error helpers | 23 |
| observe | config, facade, builder | configuration, registration, lifecycle | 18 |
| OTLP | telemetry, config builder, endpoint/header, assembler, projectors | transport, projection, and lifecycle | 34 |
| free | `sc_observability_log::init`, `sc_observability_dto::from_core_health` | one function each | 2 |

## Trait-slot coverage

The 12 `trait_slot_contracts` are separate from the inherent/free count:
`ProcessIdentityResolver::resolve`, the four subscriber/projector slots,
`LogSink::{write,flush,health}`, `TypedLogSink::{write,flush,health}`, and
`LogFilter::accepts`. Their rows preserve public implementability and
`Send + Sync` obligations; error-bearing slots identify their v2 canonical
counterpart without turning the canonical trait implementation into compat.

## Compatibility decisions frozen by D22

- Shared legacy errors are a **restoration** at the released root; canonical
  errors remain under `types::v2`, with source/code identity converted only at
  a later facade boundary.
- Existing signal definitions and source-compatible owners are
  **unchanged aliases** or **existing pairs**. In particular, no duplicate
  event/query graph is invented for `StateTransition.entity_id` validation.
- Signature-changing root methods and open traits are **new-adapter** records:
  D23–D26 own the actual released facade implementation, while D22 freezes its
  exact baseline/canonical declaration and conversion obligation.
- Absent released methods and local bridge errors are **restoration** records.
  Their `canonical_signature: null` makes the required facade work visible;
  it never claims that a missing wrapper already works.
