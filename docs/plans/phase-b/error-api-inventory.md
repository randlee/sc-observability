# B.1a legacy error production inventory

Checked against the synchronized `fix/phase-b-policy-sync` parent with:

```text
rg -n "(IdentityError|InitError|EventError|FlushError|ShutdownError|ProjectionError|SubscriberError|LogSinkError|ExportError)" crates --glob '*.rs'
```

This is an execution inventory for neutral preparation. It does not authorize
changing any of these boundaries; B.1b–B.1d own runtime adoption and B.1e owns
warning policy.

| Legacy family | Production and public/custom uses | Neutral disposition | Later owner |
| --- | --- | --- | --- |
| `IdentityError` | `sc-observability-types/src/process.rs` open `ProcessIdentityResolver`; type definition and diagnostics tests | Add `IdentityFailure` conversion/classification; retain resolver signature | B.1c/runtime integration |
| `InitError` | `sc-observability/src/builder.rs`, `runtime.rs`; `sc-observe/src/lib.rs` config/constructor/builder; `sc-observability-otlp/src/config.rs` builders/validation and `lib.rs` constructors; public `Result` signatures and tests | Add `InitFailure`; preserve all constructors and return types | B.1b–B.1d by source boundary |
| `EventError` | `sc-observability/src/runtime.rs` validation/admission/compatibility conversion; `sc-observability/src/lib.rs` `LogError`/`TryLogError` compatibility surface; `sc-observability-otlp/src/assembly.rs` span assembly | Add `EventFailure`; no bridge or runtime replacement here | B.1b and B.1d |
| `FlushError` | `sc-observability/src/maintenance.rs` and `runtime.rs`; `sc-observe/src/lib.rs`; `sc-observability-otlp/src/lib.rs` flush outcome | Add `FlushFailure`; preserve fail-open behavior and signatures | B.1b–B.1d |
| `ShutdownError` | `sc-observe/src/lib.rs` shutdown; `sc-observability-otlp/src/lib.rs` shutdown and flush/export conversion helpers | Add `ShutdownFailure`; preserve repeated-shutdown and source behavior | B.1c and B.1d |
| `ProjectionError` | `sc-observe/src/lib.rs` custom projector fixtures and routing; `sc-observability-otlp/src/projectors.rs` all three internal exporter projectors and telemetry conversion; public projector traits | Add `ProjectionFailure`; retain projector traits and registrations | B.1c and B.1d |
| `SubscriberError` | `sc-observe/src/lib.rs` custom subscriber callback type, subscriber implementations, and routing fixtures; public subscriber trait | Add `SubscriberFailure`; retain callback and registration APIs | B.1c |
| `LogSinkError` | `sc-observability/src/lib.rs` public `LogSink` trait and compatibility fixtures; `sinks.rs` write/flush/maintenance/fault paths; feature-gated `fault-injection` sink path | Add `LogSinkFailure`; retain old sink trait and health/source accounting | B.1b |
| `ExportError` | `sc-observability-otlp/src/lib.rs` open `Exporter` trait, built-in exporter implementations, export/health/shutdown paths, and exporter test fixtures | Add `ExportFailure`; retain exporter trait and telemetry emit boundary | B.1d |

## Contract coverage

The new module covers every nine-family mapping in
`error-api-contract.md`, including the feature-invariant `FaultInjected`
kind. Classification is family-local, so a code known to another family is
`Unclassified`. The legacy wrapper's `Box<ErrorContext>` is moved into and out
of the typed value; no diagnostic reconstruction occurs. The builder fixture
checks that kind remains unchanged while cause, docs, details, and source are
updated in place.

## Deferred dependency

The inventory intentionally records existing copied-bridge and runtime uses;
it does not claim their B.1 integration is complete. Full B.1a acceptance still
follows the accepted copied bridge and source integration in the owning layers.
