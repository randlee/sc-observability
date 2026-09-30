# Compatibility signature registry

`registry.json` is the machine-readable source-audit record. It covers the 58
released nominal owners identified against `v1.4.1` and records whether each
identity is restored, has been routed to the canonical implementation, or
still requires the D23 released-surface wrapper.

The audit records 141 released inherent/free callables and 12 separate trait
slots. The counts are obligations, not a claim that 141 forwarding bodies are
needed: concrete sinks and existing typed adapters can share implementations.

## Canonical routing completed by D22

| Contract cluster | Current implementation signature | Compatibility owner |
| --- | --- | --- |
| Core construction | `Logger::{builder,new,new_with_level_owner}` and `LoggerBuilder::{new,build,build_with_level_owner}` return `sc_observability_types::v2::InitError` | D23 facade |
| Core admission | `Logger::{log,try_log,try_log_with_outcome,emit}` returns `v2::EventError`; `flush` returns `v2::FlushError` | D23 facade |
| Open extension traits | resolver/subscriber/projector/sink traits retain their names but return their `v2` error family | D23 legacy traits/adapters |
| Observe | configuration, builder, flush and shutdown use `v2::{InitError,FlushError,ShutdownError}` | D23 facade |
| Log bridge | bridge re-exports `v2::{InitError,FlushError,ShutdownError}` | D23 local error wrappers |
| OTLP | config/assembly/projectors/telemetry use the canonical errors and shared backend | D23 config and released error facade |

## Evidence and guardrails

- Baseline source: `v1.4.1` / `c578912653233c7dc678fefe5af575118dbbaaa1`.
- Recovery audit: `compatibility-inventory/types.md` and `methods.md`; its
  counted scope is reproduced in `registry.json`, rather than copied as an
  unstructured release claim.
- D22 restores the released root wrappers in `sc-observability-types`,
  including the missing `TelemetryError` unit-`Shutdown` and
  `ExportFailure(Box<ErrorContext>)` layout.
- D22 does not claim the pending facade rows are compatible yet. D23 must add
  wrappers/adapters without adapting current runtime implementations to legacy
  traits.
