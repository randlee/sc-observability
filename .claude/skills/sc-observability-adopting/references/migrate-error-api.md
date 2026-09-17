# Error API migration

This reference is for an adopter who is ready to move from the retained
diagnostic wrappers to the additive typed failure APIs. B.1e implements and
validates the warning-only migration after the B.P2 prerequisite; B.2
qualifies that result and B.7 publishes it. The current workspace implements
the typed counterparts, but this preparation branch deliberately does not add
new deprecation attributes.

## Prerequisite and release policy

Use the typed APIs only after the consumer has verified the replacement at the
same version as its existing `sc-observability` dependencies. The B.P2 staged
prerequisite is `1.3.0`; B.1e selects the next-minor warning candidate
`1.4.0` and validates it. This scoped preparation leaves activation pending;
B.2 qualifies the B.1e result and B.7 publishes it. There is no removal
schedule and no planned major release. A legacy consumer continues
to build and run with default lints; `-D warnings` or `-D deprecated` may fail
because deprecation is the explicit upgrade mechanism.

## Exact method mapping

Replace an old method with the same owner's `_typed` method. Arguments,
success values, ownership and lifecycle semantics stay the same.

| Old | Typed replacement | Returned failure |
| --- | --- | --- |
| `LoggerBuilder::new` | `LoggerBuilder::new_typed` | `InitFailure` |
| `Logger::builder` | `Logger::builder_typed` | `InitFailure` |
| `Logger::new` | `Logger::new_typed` | `InitFailure` |
| `Logger::log` | `Logger::log_typed` | `LogFailure` |
| `Logger::try_log` | `Logger::try_log_typed` | `TryLogFailure` |
| `Logger::try_log_with_outcome` | `Logger::try_log_with_outcome_typed` | `TryLogFailure` |
| `Logger::flush` | `Logger::flush_typed` | `FlushFailure` |
| `ObservabilityConfig::default_for` | `default_for_typed` | `InitFailure` |
| `ObservabilityConfig::service_name` | `service_name_typed` | `InitFailure` |
| `Observability::new` | `Observability::new_typed` | `InitFailure` |
| `Observability::flush` | `flush_typed` | `FlushFailure` |
| `Observability::shutdown` | `shutdown_typed` | `ShutdownFailure` |
| `ObservabilityBuilder::build` | `build_typed` | `InitFailure` |
| `OtlpEndpoint::new(impl Into<String>)` | `OtlpEndpoint::new_typed(impl Into<String>)` | `InitFailure` |
| `AuthHeader::new(impl Into<String>)` | `AuthHeader::new_typed(impl Into<String>)` | `InitFailure` |
| `TelemetryConfigBuilder::build` | `build_typed` | `InitFailure` |
| `SpanAssembler::push` | `push_typed` | `EventFailure` |
| `Telemetry::new` | `Telemetry::new_typed` | `InitFailure` |
| `Telemetry::flush` | `flush_typed` | `FlushFailure` |
| `Telemetry::shutdown` | `shutdown_typed` | `ShutdownFailure` |

The following are intentionally not in this table: `LoggerBuilder::build`,
`Logger::new_with_level_owner`, and `LoggerBuilder::build_with_level_owner`.
They remain supported without method-level deprecation. Their additive typed
owner counterparts are `new_with_level_owner_typed` and
`build_with_level_owner_typed`. `Logger::emit` retains its existing 1.2.0
deprecation and behavior; new migration code should use `log_typed` for
blocking admission or `try_log_typed` for nonblocking admission.
`Observability::emit`, `Telemetry::emit_log`,
`Telemetry::emit_span`, and `Telemetry::emit_metric` also remain supported
because no typed counterpart exists for those public boundaries.

`LoggerBuilder::build` remains supported and infallible; use `build_typed` for
recoverable startup errors.

The corrected telemetry projector helper has no `with_typed_*` builders. Keep
the unchanged registration methods and adapt typed implementations explicitly:

```rust
use std::sync::Arc;

use sc_observability_types::typed::{ProjectionFailure, TypedLogProjector, legacy_log_projector};
use sc_observability_types::{LogEvent, Observation, ProjectionRegistration};

struct TypedNoop;

impl TypedLogProjector<String> for TypedNoop {
    fn project_logs(
        &self,
        _observation: &Observation<String>,
    ) -> Result<Vec<LogEvent>, ProjectionFailure> {
        Ok(Vec::new())
    }
}

fn main() {
    let typed_projector: Arc<dyn TypedLogProjector<String>> = Arc::new(TypedNoop);
    let _registration = ProjectionRegistration::<String>::new()
        .with_log_projector(legacy_log_projector(typed_projector));
}
```

Use `legacy_span_projector` and `legacy_metric_projector` similarly. The
`Observation<String>` supplies the concrete observable payload so this sample
is a complete downstream consumer. These functions live in the explicit `sc_observability_types::typed` module; do not
add a root glob import or a new required method to an old open trait.

## Nine wrapper families

| Old wrapper | Typed failure | Match kind |
| --- | --- | --- |
| `IdentityError` | `IdentityFailure` | `IdentityFailureKind` |
| `InitError` | `InitFailure` | `InitFailureKind` |
| `EventError` | `EventFailure` | `EventFailureKind` |
| `FlushError` | `FlushFailure` | `FlushFailureKind` |
| `ShutdownError` | `ShutdownFailure` | `ShutdownFailureKind` |
| `ProjectionError` | `ProjectionFailure` | `ProjectionFailureKind` |
| `SubscriberError` | `SubscriberFailure` | `SubscriberFailureKind` |
| `LogSinkError` | `LogSinkFailure` | `LogSinkFailureKind` |
| `ExportError` | `ExportFailure` | `ExportFailureKind` |

Each typed failure has `kind()` and `context()` through
`sc_observability_types::typed::ClassifiedError`. Match the stable kinds and
retain `_` for the `#[non_exhaustive]` enum and `Unclassified` custom values:

```rust
use sc_observability_types::typed::{ClassifiedError, InitFailureKind};

fn explain(error: &sc_observability_types::typed::InitFailure) -> &'static str {
    match error.kind() {
        InitFailureKind::LoggerInitialization => "logger setup",
        InitFailureKind::ObservationInitialization => "observation setup",
        InitFailureKind::InvalidTelemetryConfig => "telemetry configuration",
        InitFailureKind::InvalidProtocol => "telemetry protocol",
        InitFailureKind::ExporterInitialization => "exporter setup",
        InitFailureKind::IdentityResolution => "identity resolution",
        _ => "unclassified initialization failure",
    }
}
```

Do not classify by message text or assume a code from another family is a kind
in this family. `from_context` and the `From` conversions retain the original
diagnostic, cause, remediation, docs, details, and source chain. The typed
failure is not `Clone` or serialized; existing wrappers and wire fixtures stay
unchanged.

## Success and failure checks

The smallest current-API failure example validates input and matches its kind:

```rust
use sc_observability_otlp::OtlpEndpoint;
use sc_observability_types::typed::{ClassifiedError, InitFailureKind};

let error = OtlpEndpoint::new_typed("").expect_err("empty endpoint must fail");
assert_eq!(error.kind(), InitFailureKind::InvalidTelemetryConfig);
assert!(error.context().diagnostic().message.contains("empty"));
```

The corresponding success path is:

```rust
use sc_observability_otlp::OtlpEndpoint;

let endpoint = OtlpEndpoint::new_typed("https://collector.example/v1").expect("valid endpoint");
assert_eq!(endpoint.as_str(), "https://collector.example/v1");
```

Run these examples against the checked-out workspace with:

```text
cargo test -p sc-observability-otlp --test full_stack_integration typed_projector_inputs_forward_through_retained_registration
cargo test -p sc-observe --test typed_observation typed_and_legacy_construction_failures_classify_consistently
cargo test -p sc-observability-types --test neutral_contracts typed_to_legacy_adapters_preserve_errors_and_invoke_once
```

The integration tests execute success and failure paths rather than merely
importing symbols. The complete downstream fixture/JSON-diagnostic validator
is a separate B.1e/CI gate and is recorded as pending in the migration handoff.

## Custom traits and adapters

Keep existing `ProcessIdentityResolver`, `ObservationSubscriber`,
`LogProjector`, `SpanProjector`, `MetricProjector`, and `LogSink`
implementations source-compatible. New implementations may use the explicit
typed traits:

- `sc_observability_types::typed::TypedProcessIdentityResolver`
- `TypedObservationSubscriber`
- `TypedLogProjector`, `TypedSpanProjector`, `TypedMetricProjector`
- `sc_observability::typed::TypedLogSink`

At a retained legacy registration boundary, call the matching
`legacy_identity`, `legacy_subscriber`, `legacy_log_projector`,
`legacy_span_projector`, `legacy_metric_projector`, or `legacy_sink` adapter
when a new typed implementation must be reused by an old caller. Conversely,
when a new typed caller must reuse an existing legacy implementation, use
`typed_identity`, `typed_subscriber`, `typed_log_projector`,
`typed_span_projector`, `typed_metric_projector`, or
`sc_observability::typed::typed_sink`. Both directions call the underlying
implementation once and move the context; neither requires new legacy trait
methods, registrations or enum variants.

## Incremental rollback

Migrate one owner/boundary at a time. If a typed path fails validation, revert
that call site to the retained old method while leaving already-validated typed
paths in place. Do not remove the typed API, change a public signature, or
rewrite serialized values. Keep the failure's source/context in diagnostic
reports while investigating.

## Narrow warning handling

After warning activation, let ordinary legacy code show the warning. If a
retained constructor signature or compatibility adapter must mention a warned
wrapper, put a local `#[allow(deprecated)]` on the smallest item, name the
symbol/module and state its compatibility reason. Never use a crate-root,
workspace-wide or blanket allow. A typed consumer should compile under
`#![deny(deprecated)]` without any allowance. Explicitly naming `InitError` can
still warn even though the two owner-constructor methods are exempt; that is a
wrapper warning, not a method warning. Under `-D deprecated`, a legacy caller
is allowed to fail by caller policy.
