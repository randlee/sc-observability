# Compatibility source accounting

This counts the additional hand-written types and callables that the
compatible 1.x release carries on top of the canonical implementation, by
crate. It reuses the one compatibility inventory,
[`docs/compatibility/registry.json`](../compatibility/registry.json), and its
four treatments as defined in
[`signatures.md`](../compatibility/signatures.md). It checks every row against
the declaration that actually serves it in source; it does not count
registry rows as wrappers.

**Measured revision:** `87d2a9734e7967bee3b20d2c78eb228cd0793a8c`, compared
with the released baseline v1.4.1
`c578912653233c7dc678fefe5af575118dbbaaa1`. This head contains the released
DTO Result restoration and the seven compatibility-method removal metadata
corrections. Its counted declarations are the same as at the earlier
measurement `69174764c6914db254fbea3cefc8de7fe8b6243e`; later commits only
moved the `sc-observe` compatibility methods and `bridge_backend_v2` to the
lines cited below. The registry records these counts under
`source_audit.additional_handwritten`.

## Counting rule

Each public item that a consumer can name is counted once, in the crate that
declares it:

- A **type** is a struct, enum, trait or type alias. A **callable** is a free
  function, an inherent method or a required trait method (trait slot).
- An item is **hand-written** when its declaration is written in this
  workspace. A public path that reaches an existing declaration through
  `pub use` or `pub type` is an alias, and adds nothing. A declaration
  produced by a macro invocation counts at that invocation.
- **Alias** (`unchanged_alias`): the released path is served by the canonical
  declaration itself.
- **Existing pair** (`existing_pair`): the released item and its canonical
  counterpart were both already public in 1.4.1 (for example `flush` and
  `flush_typed`). The released item is retained, possibly moved into a
  `compat` file, but it is not new code.
- **Restoration** (`restoration`): the canonical implementation no longer
  has the released item, so its released declaration is written again, for
  example the nine root error wrappers in `errors.rs`.
- **Added wrapper** (`new_adapter`, or an unregistered new item): a
  hand-written declaration that adapts one signature to the other, either a
  released signature over the canonical runtime or a `v2` facade over the
  shared root state.

**Additional hand-written** means restorations plus added wrappers. Aliases
and existing pairs add nothing. A trait counts once as a type, and each of its
trait slots follows its own registry row. Each registered trait
implementation (`trait_impl_contracts`) is listed separately.

Items outside the registry are included when they are hand-written in a
compatibility region (`compat.rs`, `compat/`, the released wrappers in
`crates/sc-observability-types/src/errors.rs`, or an inline `pub mod v2`
block), or when they are a canonical twin of a released function
(`bridge_backend_v2`). An unregistered item that was public in 1.4.1 counts
as a restoration. One that was not counts as an added wrapper.

Not counted:

- **New canonical API:** for example the OTLP error-code constants and the
  new DTO projections. ADR-020 treats this as additive API that needs no
  deprecated counterpart.
- **`v2` re-exports**, which are aliases.
- **Generated impls:** derives and auto-trait impls, and trait
  implementations such as `Deref` on the `v2` facades.
- **`sc-observability-log-consumer-check`:** it has `publish = false`, and its
  `compat`-named items are test fixtures included by path.
- **`sc_observability_types::ObservationError`:** declared in `errors.rs`, but
  the canonical `v2` observation facade uses it directly.

## Counts at the measured revision

Pairs are `types/callables`.

| Crate | Aliases (types/callables) | Existing pairs | Restorations | Added wrappers | Additional handwritten types | Additional handwritten callables | Trait impls | Pending |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `sc-observability-types` | 3/20 | 8/0 | 10/0 | 5/5 | 15 | 5 | 0 | 0 |
| `sc-observability` | 4/18 | 8/0 | 0/2 | 2/35 | 2 | 37 | 2 | 0 |
| `sc-observe` | 0/4 | 3/6 | 0/6 | 3/14 | 3 | 20 | 0 | 0 |
| `sc-observability-log` | 0/5 | 6/5 | 0/3 | 1/21 | 1 | 24 | 0 | 0 |
| `sc-observability-otlp` | 0/3 | 7/17 | 1/0 | 2/16 | 3 | 16 | 0 | 0 |
| `sc-observability-dto` | 0/0 | 0/0 | 0/1 | 0/0 | 0 | 1 | 0 | 0 |
| `sc-observability-binding-runtime` | 0/0 | 0/0 | 0/0 | 0/1 | 0 | 1 | 0 | 0 |
| `sc-observability-log-macros` | 0/0 | 0/0 | 0/0 | 0/0 | 0 | 0 | 0 | 0 |
| **Total** | 7/50 | 32/28 | 11/12 | 13/92 | 24 | 104 | 2 | 0 |

The `sc-observability-log-macros` crate changes no public API. Every one of the
215 registry rows (58 types, 143 callables, 12 trait slots and 2 trait
implementations) resolves to exactly one category above. The hand-written items outside
the registry are 4 types and 27 callables added (26 in compatibility regions
plus `bridge_backend_v2`), and one restored type,
`sc_observability_otlp::OtlpProtocol`.

## Pending, not implemented

None of the items in this source-accounting inventory remains unimplemented
at the measured revision. This is a declaration inventory, not a claim that
all Phase D acceptance findings or independent reviews are complete.

The released `sc_observability_dto::from_core_health` declaration in
`crates/sc-observability-dto/src/conversion.rs:14` now returns
`Result<LogHealthDto, Failure>`. Its thin `Ok` wrapper delegates to the
infallible canonical projection, `from_canonical_core_health`. The renamed
canonical implementation is not a second added compatibility callable.

### Historical measurement

PR743 (`4c108cf31a6bdb02a25d930e7bd4b4b9a52283a3`) recorded the PR740
source measurement at `f36dab934f2b4775a4e0280db7aac24d6d868560`: 24 added
handwritten types, 103 added handwritten callables (11 restored), and one
pending DTO callable. That measurement remains valid for that revision.
PR744 (`dc1a634ecca024a46f419b1fdcc949e531700296`) restored the released
Result-returning wrapper, adding exactly one restored callable. At PR752,
the resulting totals are 24 types and 104 callables (12 restored), with
zero pending inventory items. The seven PR752 removal-metadata corrections
change no source declarations or counts.

## Registry treatment compared with source

Seven rows labelled `unchanged_alias` are served by a retained 1.4.1
declaration that now lives in a `compat` file, alongside the `*_typed` method
it already had in 1.4.1. By the rule above, they are existing pairs:

| Row | Declaration |
| --- | --- |
| `sc_observe::ObservabilityConfig::default_for` | `crates/sc-observe/src/compat.rs:85` |
| `sc_observe::ObservabilityConfig::service_name` | `crates/sc-observe/src/compat.rs:94` |
| `sc_observe::Observability::new` | `crates/sc-observe/src/compat.rs:115` |
| `sc_observe::Observability::flush` | `crates/sc-observe/src/compat.rs:129` |
| `sc_observe::Observability::shutdown` | `crates/sc-observe/src/compat.rs:144` |
| `sc_observe::ObservabilityBuilder::build` | `crates/sc-observe/src/compat.rs:171` |
| `sc_observability_otlp::SpanAssembler::push` | `crates/sc-observability-otlp/src/compat.rs:701` |

These rows now list their containing `compat` file in `removable_paths`,
with a rationale limited to the released wrapper method, not deletion of the
whole file or its typed counterpart. The six observe `canonical_source`
paths identify the selected-head released declaration in `compat.rs`, not
the different `v2` signature in `lib.rs`; the OTLP pointer already identified
its released declaration. PR752 preserves their registry treatment and the
list under `source_audit.additional_handwritten`. They remain existing pairs
for source accounting and add no handwritten declarations.

## Removal points

A later major release deletes the counted restorations and added wrappers by
deleting:

- the `compat` files of `sc-observability`, `sc-observe`,
  `sc-observability-log` and `sc-observability-otlp`;
- the nine released wrappers in `crates/sc-observability-types/src/errors.rs`
  and their root re-export;
- the root re-exports recorded in `compat_root_reexport_exceptions`;
- each row's own `removable_paths`, including the private released-only
  admission in `crates/sc-observability-otlp/src/runtime.rs` and
  `crates/sc-observability-otlp/src/config/validation.rs` recorded on the
  `sc_observability_otlp::Telemetry` row. Those items are crate-private, so
  they are not counted as public declarations above.

The inline `v2` facades in `sc-observe` and `sc-observability-log`, and
`bridge_backend_v2`, become the canonical root names at that point. They
are wrappers only while the released names occupy the root.

## Reproducing the counts

1. List the public API of each crate at both revisions with the CI form,
   `cargo public-api --manifest-path crates/<crate>/Cargo.toml -sss`, and diff
   the listings.
2. Build rustdoc JSON at the measured revision:
   `RUSTDOCFLAGS="-Z unstable-options --output-format json" cargo +nightly doc --workspace --no-deps --all-features`.
3. Resolve every public path from each crate root through `pub use` to its
   declaration span, including inherent-impl methods and trait slots.
4. For each registry row, take the declaration of its `symbol` and apply the
   rule above. Then add hand-written items in compatibility regions that have
   no row, classified by whether the 1.4.1 listing contains the same path.

## Evidence

Every restoration, added wrapper, trait implementation and pending item, with
its declaration at the measured revision:

### `sc-observability-types`

| Category | Kind | Item | Declaration | Row |
| --- | --- | --- | --- | --- |
| added wrapper | trait slot | `sc_observability_types::LogProjector::project_logs` | `crates/sc-observability-types/src/projection.rs:67` | registry |
| added wrapper | trait slot | `sc_observability_types::MetricProjector::project_metrics` | `crates/sc-observability-types/src/projection.rs:98` | registry |
| added wrapper | trait slot | `sc_observability_types::ObservationSubscriber::observe` | `crates/sc-observability-types/src/projection.rs:44` | registry |
| added wrapper | trait slot | `sc_observability_types::ProcessIdentityResolver::resolve` | `crates/sc-observability-types/src/process.rs:82` | registry |
| added wrapper | trait slot | `sc_observability_types::SpanProjector::project_spans` | `crates/sc-observability-types/src/projection.rs:81` | registry |
| added wrapper | type | `sc_observability_types::LogProjector` | `crates/sc-observability-types/src/projection.rs:57` | registry |
| added wrapper | type | `sc_observability_types::MetricProjector` | `crates/sc-observability-types/src/projection.rs:88` | registry |
| added wrapper | type | `sc_observability_types::ObservationSubscriber` | `crates/sc-observability-types/src/projection.rs:34` | registry |
| added wrapper | type | `sc_observability_types::ProcessIdentityResolver` | `crates/sc-observability-types/src/process.rs:75` | registry |
| added wrapper | type | `sc_observability_types::SpanProjector` | `crates/sc-observability-types/src/projection.rs:71` | registry |
| restoration | type | `sc_observability_types::EventError` | `crates/sc-observability-types/src/errors.rs:65` | registry |
| restoration | type | `sc_observability_types::ExportError` | `crates/sc-observability-types/src/errors.rs:113` | registry |
| restoration | type | `sc_observability_types::FlushError` | `crates/sc-observability-types/src/errors.rs:73` | registry |
| restoration | type | `sc_observability_types::IdentityError` | `crates/sc-observability-types/src/errors.rs:18` | registry |
| restoration | type | `sc_observability_types::InitError` | `crates/sc-observability-types/src/errors.rs:57` | registry |
| restoration | type | `sc_observability_types::LogSinkError` | `crates/sc-observability-types/src/errors.rs:105` | registry |
| restoration | type | `sc_observability_types::ProjectionError` | `crates/sc-observability-types/src/errors.rs:89` | registry |
| restoration | type | `sc_observability_types::ShutdownError` | `crates/sc-observability-types/src/errors.rs:81` | registry |
| restoration | type | `sc_observability_types::SubscriberError` | `crates/sc-observability-types/src/errors.rs:97` | registry |
| restoration | type | `sc_observability_types::TelemetryError` | `crates/sc-observability-types/src/errors.rs:138` | registry |

### `sc-observability`

| Category | Kind | Item | Declaration | Row |
| --- | --- | --- | --- | --- |
| added wrapper | callable | `sc_observability::Logger::builder` | `crates/sc-observability/src/compat.rs:457` | registry |
| added wrapper | callable | `sc_observability::Logger::builder_typed` | `crates/sc-observability/src/compat.rs:462` | registry |
| added wrapper | callable | `sc_observability::Logger::emit` | `crates/sc-observability/src/compat.rs:553` | registry |
| added wrapper | callable | `sc_observability::Logger::flush` | `crates/sc-observability/src/compat.rs:568` | registry |
| added wrapper | callable | `sc_observability::Logger::flush_typed` | `crates/sc-observability/src/compat.rs:573` | registry |
| added wrapper | callable | `sc_observability::Logger::follow` | `crates/sc-observability/src/compat.rs:600` | registry |
| added wrapper | callable | `sc_observability::Logger::health` | `crates/sc-observability/src/compat.rs:593` | registry |
| added wrapper | callable | `sc_observability::Logger::level_state` | `crates/sc-observability/src/compat.rs:587` | registry |
| added wrapper | callable | `sc_observability::Logger::log` | `crates/sc-observability/src/compat.rs:505` | registry |
| added wrapper | callable | `sc_observability::Logger::log_typed` | `crates/sc-observability/src/compat.rs:511` | registry |
| added wrapper | callable | `sc_observability::Logger::new` | `crates/sc-observability/src/compat.rs:471` | registry |
| added wrapper | callable | `sc_observability::Logger::new_typed` | `crates/sc-observability/src/compat.rs:485` | registry |
| added wrapper | callable | `sc_observability::Logger::new_with_level_owner` | `crates/sc-observability/src/compat.rs:478` | registry |
| added wrapper | callable | `sc_observability::Logger::new_with_level_owner_typed` | `crates/sc-observability/src/compat.rs:492` | registry |
| added wrapper | callable | `sc_observability::Logger::query` | `crates/sc-observability/src/compat.rs:443` | registry |
| added wrapper | callable | `sc_observability::Logger::service_name` | `crates/sc-observability/src/compat.rs:581` | unregistered |
| added wrapper | callable | `sc_observability::Logger::shutdown` | `crates/sc-observability/src/compat.rs:448` | registry |
| added wrapper | callable | `sc_observability::Logger::try_log` | `crates/sc-observability/src/compat.rs:520` | registry |
| added wrapper | callable | `sc_observability::Logger::try_log_typed` | `crates/sc-observability/src/compat.rs:525` | registry |
| added wrapper | callable | `sc_observability::Logger::try_log_with_outcome` | `crates/sc-observability/src/compat.rs:534` | registry |
| added wrapper | callable | `sc_observability::Logger::try_log_with_outcome_typed` | `crates/sc-observability/src/compat.rs:541` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::build` | `crates/sc-observability/src/compat.rs:390` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::build_typed` | `crates/sc-observability/src/compat.rs:399` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::build_with_level_owner` | `crates/sc-observability/src/compat.rs:408` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::build_with_level_owner_typed` | `crates/sc-observability/src/compat.rs:416` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::new` | `crates/sc-observability/src/compat.rs:356` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::new_typed` | `crates/sc-observability/src/compat.rs:363` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::register_sink` | `crates/sc-observability/src/compat.rs:370` | registry |
| added wrapper | callable | `sc_observability::LoggerBuilder::register_typed_sink` | `crates/sc-observability/src/compat.rs:376` | unregistered |
| added wrapper | callable | `sc_observability::RetainedSinkFaultInjector::wrap` | `crates/sc-observability/src/compat.rs:248` | registry |
| added wrapper | callable | `sc_observability::SinkRegistration::new` | `crates/sc-observability/src/compat.rs:163` | registry |
| added wrapper | trait slot | `sc_observability::LogSink::flush` | `crates/sc-observability/src/compat.rs:50` | registry |
| added wrapper | trait slot | `sc_observability::LogSink::write` | `crates/sc-observability/src/compat.rs:47` | registry |
| added wrapper | trait slot | `sc_observability::typed::TypedLogSink::flush` | `crates/sc-observability/src/compat.rs:64` | registry |
| added wrapper | trait slot | `sc_observability::typed::TypedLogSink::write` | `crates/sc-observability/src/compat.rs:61` | registry |
| added wrapper | type | `sc_observability::LogSink` | `crates/sc-observability/src/compat.rs:45` | registry |
| added wrapper | type | `sc_observability::typed::TypedLogSink` | `crates/sc-observability/src/compat.rs:59` | registry |
| restoration | callable | `sc_observability::typed::legacy_sink` | `crates/sc-observability/src/compat.rs:74` | registry |
| restoration | callable | `sc_observability::typed::typed_sink` | `crates/sc-observability/src/compat.rs:80` | registry |
| restoration | trait impl | `sc_observability::typed::TypedLogSink for sc_observability::ConsoleSink` | `crates/sc-observability/src/compat.rs` | registry |
| restoration | trait impl | `sc_observability::typed::TypedLogSink for sc_observability::JsonlFileSink` | `crates/sc-observability/src/compat.rs` | registry |

### `sc-observe`

| Category | Kind | Item | Declaration | Row |
| --- | --- | --- | --- | --- |
| added wrapper | callable | `sc_observe::ObservabilityBuilder::register_projection` | `crates/sc-observe/src/lib.rs:770` | registry |
| added wrapper | callable | `sc_observe::ObservabilityBuilder::register_subscriber` | `crates/sc-observe/src/lib.rs:727` | registry |
| added wrapper | callable | `sc_observe::v2::Observability::builder` | `crates/sc-observe/src/lib.rs:124` | unregistered |
| added wrapper | callable | `sc_observe::v2::Observability::emit` | `crates/sc-observe/src/lib.rs:129` | unregistered |
| added wrapper | callable | `sc_observe::v2::Observability::flush` | `crates/sc-observe/src/lib.rs:137` | unregistered |
| added wrapper | callable | `sc_observe::v2::Observability::health` | `crates/sc-observe/src/lib.rs:147` | unregistered |
| added wrapper | callable | `sc_observe::v2::Observability::new` | `crates/sc-observe/src/lib.rs:119` | unregistered |
| added wrapper | callable | `sc_observe::v2::Observability::shutdown` | `crates/sc-observe/src/lib.rs:142` | unregistered |
| added wrapper | callable | `sc_observe::v2::ObservabilityBuilder::build` | `crates/sc-observe/src/lib.rs:190` | unregistered |
| added wrapper | callable | `sc_observe::v2::ObservabilityBuilder::register_projection` | `crates/sc-observe/src/lib.rs:182` | unregistered |
| added wrapper | callable | `sc_observe::v2::ObservabilityBuilder::register_subscriber` | `crates/sc-observe/src/lib.rs:174` | unregistered |
| added wrapper | callable | `sc_observe::v2::ObservabilityBuilder::with_observability_health_provider` | `crates/sc-observe/src/lib.rs:166` | unregistered |
| added wrapper | callable | `sc_observe::v2::ObservabilityConfig::default_for` | `crates/sc-observe/src/lib.rs:81` | unregistered |
| added wrapper | callable | `sc_observe::v2::ObservabilityConfig::service_name` | `crates/sc-observe/src/lib.rs:86` | unregistered |
| added wrapper | type | `sc_observe::v2::Observability` | `crates/sc-observe/src/lib.rs:107` | unregistered |
| added wrapper | type | `sc_observe::v2::ObservabilityBuilder` | `crates/sc-observe/src/lib.rs:154` | unregistered |
| added wrapper | type | `sc_observe::v2::ObservabilityConfig` | `crates/sc-observe/src/lib.rs:77` | unregistered |
| restoration | callable | `sc_observe::Observability::flush_typed` | `crates/sc-observe/src/compat.rs:154` | registry |
| restoration | callable | `sc_observe::Observability::new_typed` | `crates/sc-observe/src/compat.rs:149` | registry |
| restoration | callable | `sc_observe::Observability::shutdown_typed` | `crates/sc-observe/src/compat.rs:160` | registry |
| restoration | callable | `sc_observe::ObservabilityBuilder::build_typed` | `crates/sc-observe/src/compat.rs:176` | registry |
| restoration | callable | `sc_observe::ObservabilityConfig::default_for_typed` | `crates/sc-observe/src/compat.rs:99` | registry |
| restoration | callable | `sc_observe::ObservabilityConfig::service_name_typed` | `crates/sc-observe/src/compat.rs:104` | registry |

### `sc-observability-log`

| Category | Kind | Item | Declaration | Row |
| --- | --- | --- | --- | --- |
| added wrapper | callable | `sc_observability_log::LogControl::active_log_path` | `crates/sc-observability-log/src/compat/mod.rs:375` | registry |
| added wrapper | callable | `sc_observability_log::LogControl::dropped_events` | `crates/sc-observability-log/src/compat/mod.rs:381` | registry |
| added wrapper | callable | `sc_observability_log::LogControl::flush` | `crates/sc-observability-log/src/compat/mod.rs:355` | registry |
| added wrapper | callable | `sc_observability_log::LogControl::health` | `crates/sc-observability-log/src/compat/mod.rs:366` | registry |
| added wrapper | callable | `sc_observability_log::LogControl::into_v2` | `crates/sc-observability-log/src/compat/mod.rs:346` | unregistered |
| added wrapper | callable | `sc_observability_log::LogControl::query` | `crates/sc-observability-log/src/compat/mod.rs:411` | registry |
| added wrapper | callable | `sc_observability_log::LogControl::try_log` | `crates/sc-observability-log/src/compat/mod.rs:402` | registry |
| added wrapper | callable | `sc_observability_log::LogControl::wait_stopped` | `crates/sc-observability-log/src/compat/mod.rs:390` | registry |
| added wrapper | callable | `sc_observability_log::LogGuard::control` | `crates/sc-observability-log/src/lib.rs:417` | registry |
| added wrapper | callable | `sc_observability_log::LogGuard::flush` | `crates/sc-observability-log/src/lib.rs:460` | registry |
| added wrapper | callable | `sc_observability_log::LogGuard::shutdown` | `crates/sc-observability-log/src/lib.rs:476` | registry |
| added wrapper | callable | `sc_observability_log::init` | `crates/sc-observability-log/src/lib.rs:532` | registry |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::active_log_path` | `crates/sc-observability-log/src/lib.rs:235` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::control` | `crates/sc-observability-log/src/lib.rs:204` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::dropped_events` | `crates/sc-observability-log/src/lib.rs:229` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::elevate_level` | `crates/sc-observability-log/src/lib.rs:253` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::flush` | `crates/sc-observability-log/src/lib.rs:213` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::health` | `crates/sc-observability-log/src/lib.rs:244` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::reset_level` | `crates/sc-observability-log/src/lib.rs:266` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::LogGuard::shutdown` | `crates/sc-observability-log/src/lib.rs:222` | unregistered |
| added wrapper | callable | `sc_observability_log::v2::init` | `crates/sc-observability-log/src/lib.rs:194` | unregistered |
| added wrapper | type | `sc_observability_log::v2::LogGuard` | `crates/sc-observability-log/src/lib.rs:185` | unregistered |
| restoration | callable | `sc_observability_log::FlushError::remediation` | `crates/sc-observability-log/src/compat/mod.rs:230` | registry |
| restoration | callable | `sc_observability_log::InitError::remediation` | `crates/sc-observability-log/src/compat/mod.rs:192` | registry |
| restoration | callable | `sc_observability_log::ShutdownError::remediation` | `crates/sc-observability-log/src/compat/mod.rs:268` | registry |

### `sc-observability-otlp`

| Category | Kind | Item | Declaration | Row |
| --- | --- | --- | --- | --- |
| added wrapper | callable | `sc_observability_otlp::AuthHeader::new` | `crates/sc-observability-otlp/src/compat.rs:128` | registry |
| added wrapper | callable | `sc_observability_otlp::AuthHeader::new_typed` | `crates/sc-observability-otlp/src/compat.rs:133` | registry |
| added wrapper | callable | `sc_observability_otlp::OtlpEndpoint::new` | `crates/sc-observability-otlp/src/compat.rs:73` | registry |
| added wrapper | callable | `sc_observability_otlp::OtlpEndpoint::new_typed` | `crates/sc-observability-otlp/src/compat.rs:78` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::emit_log` | `crates/sc-observability-otlp/src/compat.rs:469` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::emit_metric` | `crates/sc-observability-otlp/src/compat.rs:483` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::emit_span` | `crates/sc-observability-otlp/src/compat.rs:476` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::flush` | `crates/sc-observability-otlp/src/compat.rs:495` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::flush_async_typed` | `crates/sc-observability-otlp/src/compat.rs:505` | unregistered |
| added wrapper | callable | `sc_observability_otlp::Telemetry::new` | `crates/sc-observability-otlp/src/compat.rs:446` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::new_typed` | `crates/sc-observability-otlp/src/compat.rs:451` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::shutdown` | `crates/sc-observability-otlp/src/compat.rs:515` | registry |
| added wrapper | callable | `sc_observability_otlp::Telemetry::shutdown_async_typed` | `crates/sc-observability-otlp/src/compat.rs:525` | unregistered |
| added wrapper | callable | `sc_observability_otlp::TelemetryConfigBuilder::build` | `crates/sc-observability-otlp/src/compat.rs:346` | registry |
| added wrapper | callable | `sc_observability_otlp::TelemetryConfigBuilder::build_typed` | `crates/sc-observability-otlp/src/compat.rs:351` | registry |
| added wrapper | callable | `sc_observability_otlp::TelemetryConfigBuilder::with_transport` | `crates/sc-observability-otlp/src/compat.rs:317` | registry |
| added wrapper | type | `sc_observability_otlp::OtelConfig` | `crates/sc-observability-otlp/src/compat.rs:168` | registry |
| added wrapper | type | `sc_observability_otlp::TelemetryConfig` | `crates/sc-observability-otlp/src/compat.rs:255` | registry |
| restoration | type | `sc_observability_otlp::OtlpProtocol` | `crates/sc-observability-otlp/src/compat.rs:43` | unregistered |

### `sc-observability-dto`

| Category | Kind | Item | Declaration | Row |
| --- | --- | --- | --- | --- |
| restoration | callable | `sc_observability_dto::from_core_health` | `crates/sc-observability-dto/src/conversion.rs:14` | registry |

### `sc-observability-binding-runtime`

| Category | Kind | Item | Declaration | Row |
| --- | --- | --- | --- | --- |
| added wrapper | callable | `sc_observability_binding_runtime::bridge_backend_v2` | `crates/sc-observability-binding-runtime/src/lib.rs:313` | unregistered |
