# d-21: OTLP contract

Generated projection of `obs-d-21`; the bead is authoritative.

## Plan metadata

- Wave: 1
- Layer: 2
- Assignee / model: lobs / luna
- Relation: `must_follow`
- Closure: `contract`
- Target boundary: sc-observability-otlp contract and workspace registration
- Branch: `sprint/d-21-otlp-contract`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-21-otlp-contract`
- PR target (merge order only): `sprint/d-12-types-and-otlp-contract`
- Blocked by: `obs-d-12-sanity`
- Requirements: LAY-001, LAY-002, LAY-003, LAY-004, LAY-005, LAY-006, LAY-007, LOG-018, LOG-019, NFR-003, NFR-004, NFR-005, NFR-008, NFR-009, NFR-010, NFR-011, NFR-012, OBS-015, OBS-018, OTLP-004, OTLP-005, OTLP-006, OTLP-007, OTLP-008, OTLP-009, OTLP-010, OTLP-011, OTLP-013, OTLP-014, OTLP-016, OTLP-018, OTLP-019, OTLP-020, OTLP-021, OTLP-022, OTLP-023, PHD-003, PHD-004, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-002, TYP-023, TYP-024, TYP-030
- ADRs: ADR-002, ADR-003, ADR-004, ADR-005, ADR-006, ADR-009, ADR-011, ADR-017, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `Cargo.lock`
  - `Cargo.toml`
  - `bindings/python/sc-observability-py/Cargo.toml`
  - `bindings/schema-generator/Cargo.lock`
  - `bindings/schema-generator/Cargo.toml`
  - `bindings/tauri/Cargo.lock`
  - `bindings/tauri/Cargo.toml`
  - `boundaries/**`
  - `crates/sc-observability-binding-runtime/Cargo.toml`
  - `crates/sc-observability-dto/Cargo.toml`
  - `crates/sc-observability-log-consumer-check/Cargo.toml`
  - `crates/sc-observability-log-macros/Cargo.toml`
  - `crates/sc-observability-log/Cargo.toml`
  - `crates/sc-observability-otlp/Cargo.toml`
  - `crates/sc-observability-otlp/src/assembly.rs`
  - `crates/sc-observability-otlp/src/config.rs`
  - `crates/sc-observability-otlp/src/constants.rs`
  - `crates/sc-observability-otlp/src/contract_tests.rs`
  - `crates/sc-observability-otlp/src/contracts.rs`
  - `crates/sc-observability-otlp/src/error_codes.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/implementation.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/mod.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/tests.rs`
  - `crates/sc-observability-otlp/src/lib.rs`
  - `crates/sc-observability-otlp/src/lifecycle.rs`
  - `crates/sc-observability-otlp/src/lifecycle_tests.rs`
  - `crates/sc-observability-otlp/src/projectors.rs`
  - `crates/sc-observability-otlp/src/sdk/implementation.rs`
  - `crates/sc-observability-otlp/src/sdk/mod.rs`
  - `crates/sc-observability-otlp/src/sdk/tests.rs`
  - `crates/sc-observability-otlp/src/testing.rs`
  - `crates/sc-observability/Cargo.toml`
  - `crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-baseline/Cargo.toml`
  - `crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-consumer/Cargo.toml`
  - `crates/sc-observe/Cargo.toml`
  - `docs/plans/phase-b/evidence/b3-final/Cargo.toml`
  - `examples/atm-adapter-example/Cargo.toml`
  - `examples/custom-sink-example/Cargo.toml`
  - `examples/log-settings/Cargo.toml`
  - `examples/otlp-legacy/Cargo.toml`
  - `examples/rust-python-logging/Cargo.toml`
  - `examples/tauri-logging/src-tauri/Cargo.toml`
  - `scripts/ci/fixtures/error-migration/legacy/Cargo.toml`
  - `scripts/ci/fixtures/error-migration/migrated/Cargo.toml`
  - `scripts/ci/fixtures/error-migration/partial/Cargo.toml`
  - `scripts/ci/validate_dependency_bans.sh`
  - `scripts/ci/validate_repo_boundaries.sh`
  - `docs/plans/phase-d/sprint-d-21-otlp-contract.md`

## Goal

Close the OTLP contract after obs-d-12's types sanity gate, releasing obs-d-5–8 without delaying types-only implementation work. This is the second stage within numbered wave 1; it is not an independent root.

## Deliverables

1. Define crate-private exporter/lifecycle traits, ExporterSet and factory/test doubles; checked durations, dual record/byte admission bounds, defaults, validation order and conversion to the types-owned ConfigFailure/ExportError variants. Own contract_tests.rs and run types-owned neutral_contracts.rs read-only. [OTLP-004/005/011/018–022; PHD-003/004; NFR-004/005]

2. Register all OTLP module roots/stubs (including contracts.rs, lifecycle.rs, sdk/mod.rs, legacy_http_json/mod.rs and facade declarations) and create every declared placeholder. Own root Cargo.toml, Cargo.lock, workspace members/features, remaining non-types Cargo manifests and dependency/boundary allowlists. Consume architecture section6 and ADR-019 normative allowlist read-only; record exact reviewed pins in owned manifests/lock/boundaries. Hand off implementation/test files to obs-d-5–8; module roots remain read-only. The SDK example Cargo.toml stays with obs-d-7. [LAY-002–005; NFR-003/004/009; OTLP-014/016/023]

3. Apply the atomic 2.0 workspace/package/dependency bump and resolve all owned locks. The explicit obs-d-12 handoff authorizes changing only the types Cargo.toml version literal even though its fence stays with obs-d-12. Verify the already reviewed shared requirements/architecture/API-design contract (OTLP-005/020/021, PHD-003/004 and section6) through the read-only handoff; keep release-note/inventory alignment in obs-d-18. [NFR-008/010/011/012; PHD-003/004]

## This Sprint Does Not Close

obs-d-12 alone defines shared errors/signals and normative documentation; this bead consumes them. obs-d-5–8 implement real projector/lifecycle/backend behavior. obs-d-18 owns public facade composition, compatibility removal and release/API gates. No collector or publication claim is made.

## Design

## Backend and trait contract

```rust
#[non_exhaustive]
pub enum ExporterBackend {
    OpenTelemetrySdk,
    LegacyHttpJson, // reserved; D.8 makes this backend operational
}

#[non_exhaustive]
pub struct OtelConfig {
    pub backend: ExporterBackend,
    pub protocol: OtlpProtocol,
    // endpoint/auth/TLS and timeout fields remain explicit;
    pub legacy_retry: Option<LegacyRetryPolicy>,
}

#[non_exhaustive]
pub struct LegacyRetryPolicy { /* legacy-only retry wire fields below */ }

impl OtelConfig {
    pub fn new(backend: ExporterBackend, protocol: OtlpProtocol) -> Self;
}

type LifecycleFuture = Pin<
    Box<dyn Future<Output = Result<(), ExportError>> + Send + 'static>
>;

pub(crate) trait ExporterLifecycle: Send + Sync {
    fn blocking_preflight(&self) -> Result<(), ExportError>;
    fn flush_async(&self) -> LifecycleFuture;
    fn shutdown_async(&self) -> LifecycleFuture;
    fn flush_blocking(&self) -> Result<(), ExportError>;
    fn shutdown_blocking(&self) -> Result<(), ExportError>;
}

pub(crate) trait LogExporter: Send + Sync {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
}

pub(crate) trait TraceExporter: Send + Sync {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
}

pub(crate) trait MetricExporter: Send + Sync {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError>;
}

pub(crate) struct ExporterSet {
    logs: Arc<dyn LogExporter>,
    traces: Arc<dyn TraceExporter>,
    metrics: Arc<dyn MetricExporter>,
    lifecycle: Arc<dyn ExporterLifecycle>,
}
```

Both backends construct the same `ExporterSet`; `Telemetry` stores only these
trait objects. `ExporterBackend` is consumed by construction/injection and is
never branched on by emit, flush, or shutdown. Selecting `LegacyHttpJson`
before D.8 returns a stable typed unsupported-backend error. An enabled
configuration never silently installs a no-op exporter.

The factory validates this closed matrix before allocating providers/workers:

| Backend | Valid protocol | Required feature/runtime | Invalid result |
| --- | --- | --- | --- |
| disabled (transport disabled) | none | none | the sole no-network disabled implementation |
| `OpenTelemetrySdk` | SDK-supported gRPC or HTTP/protobuf | `otlp-sdk`; entered caller Tokio runtime | stable unsupported-protocol/runtime error |
| `LegacyHttpJson` | `HttpJson` only | `legacy-http-json`; plain-thread construction | reserved typed error until D.8 |

Delete public/production `Noop*Exporter` fallbacks; disabled construction is an
explicit private disabled set and an enabled selection can never reach it.
Every existing `OtelConfig` field receives one disposition: endpoint,
headers/auth, CA/TLS and `timeout_ms` map to the SDK/legacy builders;
`debug_local_export` is a separate diagnostic mirror outside exporter
selection; `insecure_skip_verify` is either implemented by the backend with an
explicit security warning or rejected at construction—never ignored.
`timeout_ms` covers the entire legacy HTTP request, including connect, TLS,
request write, response headers, and response read. Endpoint and header/auth
values are validated before provider/worker construction; malformed endpoints,
invalid header syntax, and forbidden credential placement return named
construction failures without retaining secret values.

### Validated transport contract

D.21 is the sole contract owner of every transport-bound field, default, validation,
and configuration error. The 2.0 wire surface uses direct shared transport
fields plus a grouped `legacy_retry` object:

| Field | Applicability | Default when absent |
| --- | --- | --- |
| `timeout_ms` | both backends; maps to request/export timeout | `3_000` |
| `lifecycle_flush_timeout_ms` | both backends | `30_000` |
| `lifecycle_shutdown_timeout_ms` | both backends | `30_000` |
| `queue_capacity` | both backends; bounded admission queue | `1_024` |
| `legacy_retry.max_retries` | legacy only, optional on wire | `3` |
| `legacy_retry.initial_backoff_ms` | legacy only, optional on wire | `250` |
| `legacy_retry.max_backoff_ms` | legacy only, optional on wire | `5_000` |
| `legacy_retry.retry_sequence_timeout_ms` | legacy only, optional on wire | `30_000` |
| `legacy_retry.retry_after_cap_ms` | legacy only, optional on wire | `5_000` |
| `legacy_retry.retry_jitter_percent` | legacy only, optional on wire | `20` |

`queue_capacity` counts admitted records, not batches, and is validated as `1..=65_536`. A separate checked `queue_byte_capacity` defaults to 16 MiB, has a hard 64 MiB maximum, and bounds the serialized payload bytes held by all queued/in-flight batches. Admission reserves both record and byte credits atomically; either exhausted budget returns QueueFull. Records larger than 1 MiB are rejected before enqueue; batches split at 512 records or 1 MiB. The queue cannot retain 65,536 one-MiB batches. A 413 is terminal for that split batch,
which is counted once as failed/dropped rather than retried as a larger batch.
`shutdown_async_typed` has one drain budget: it starts at shutdown entry and
covers cancellation, the in-flight request, barrier, and worker join. On
expiry it returns `LifecycleTimeout` with remaining admitted work accounted.

```rust
#[non_exhaustive]
pub struct TelemetryHealth {
    pub queue_depth: usize,
    pub queue_capacity: usize,
    pub worker_state: WorkerState,
    pub last_terminal_failure: Option<Diagnostic>,
    pub last_success: Option<Timestamp>,
}
```

For `OpenTelemetrySdk`, the three shared timeout fields map to SDK lifecycle /
export construction. Any explicit legacy-only field—including the pre-existing
`max_retries`, `initial_backoff_ms`, and `max_backoff_ms`—returns
`ConfigFieldNotApplicable`. Nothing is ignored. This 2.0 optional-field change
and its migration from the former unconditional retry defaults are documented.

Defaults are resolved **before** validation. Each resolved value retains
`ValueOrigin::{Default, Explicit}` so an error identifies both the offending
field and whether a conflicting peer was defaulted. Partial overrides are
therefore deterministic and reviewable.

All raw serialized millisecond/percent fields are converted exactly once:

```rust
#[non_exhaustive]
pub enum OtlpConfigField {
    Endpoint,
    Header,
    Timeout,
    LifecycleFlushTimeout,
    LifecycleShutdownTimeout,
    QueueCapacity,
    QueueByteCapacity,
    MaxRetries,
    InitialBackoff,
    MaxBackoff,
    RetrySequenceTimeout,
    RetryAfterCap,
    RetryJitterPercent,
}

#[non_exhaustive]
pub enum ValueOrigin { Default, Explicit }

#[non_exhaustive]
pub struct ResolvedField<T> {
    pub field: OtlpConfigField,
    pub value: T,
    pub origin: ValueOrigin,
}

#[non_exhaustive]
pub enum OtlpConfigTarget { Disabled, Backend(ExporterBackend) }

pub(crate) struct PositiveDuration(Duration);

impl PositiveDuration {
    fn try_from_millis(field: OtlpConfigField, value: u64)
        -> Result<Self, ConfigFailure>;
}

pub(crate) struct LifecycleBounds {
    flush: PositiveDuration,
    shutdown: PositiveDuration,
}

pub(crate) struct BoundedPercent(u8); // checked 0..=100

pub(crate) struct RetryPolicy {
    max_retries: u32,
    initial_backoff: PositiveDuration,
    max_backoff: PositiveDuration,
    sequence_timeout: PositiveDuration,
    retry_after_cap: PositiveDuration,
    jitter: BoundedPercent,
}

pub(crate) struct ValidatedTransportBounds {
    queue_capacity: QueueCapacity,
    queue_byte_capacity: QueueByteCapacity,
    request_timeout: PositiveDuration,
    lifecycle: LifecycleBounds,
    backend: BackendTransportBounds,
}

pub(crate) enum BackendTransportBounds {
    Disabled,
    Sdk,
    Legacy(RetryPolicy),
}

impl ValidatedTransportBounds {
    fn try_from_config(config: &OtelConfig) -> Result<Self, ConfigFailure>;
}
```

`TelemetryHealth`, `OtlpConfigField`, `ValueOrigin`, `ResolvedField`, and
`OtlpConfigTarget` are `#[non_exhaustive]` public types so their 2.0 contracts
can add fields or variants without a further breaking release.

The constructor derives `Disabled` from `config.enabled == false`; otherwise
it derives the backend only from `config.backend`.
`LifecycleBounds` holds checked positive flush/shutdown durations;
`RetryPolicy` holds `max_retries`, checked initial/max/sequence/Retry-After
durations, and `BoundedPercent(0..=100)`. `BackendTransportBounds` makes legacy
retry state unrepresentable for SDK. Validation, using checked arithmetic, is:

- every millisecond duration is positive and convertible to `Duration`;
- `lifecycle_shutdown_timeout_ms >= timeout_ms`;
- `lifecycle_flush_timeout_ms >= timeout_ms`;
- `queue_capacity` is in `1..=65_536`, otherwise `InvalidQueueCapacity`;
- for legacy, `max_backoff_ms >= initial_backoff_ms`;
- for legacy, `retry_sequence_timeout_ms >= timeout_ms`;
- for legacy, `0 < retry_after_cap_ms <= retry_sequence_timeout_ms`;
- for legacy, `retry_jitter_percent <= 100`;
- reject every explicit field inapplicable to disabled transport or the
  selected backend with `ConfigFieldNotApplicable`;
- reject a requested insecure verification override when the selected backend
  does not explicitly support it with `InsecureTransportRejected`.
- validate endpoint URL syntax, header/auth syntax, and credential placement;
  otherwise return `InvalidEndpoint` or `InvalidHeader` with a redacted,
  field-only payload.

Checks execute in exactly this listed order and return the first failure; they
are not aggregated. Within the first bullet, fields are checked in the wire
table's top-to-bottom order. This makes every multi-violation diagnostic
deterministic.

Both factories receive only `ValidatedTransportBounds` and cannot inspect or
reparse raw fields. Deadlines use a monotonic injectable clock and start when
the public operation is admitted.

Construction order is fixed: resolve defaults and create `ResolvedField`
values; run the ordered validation list above; build
`ValidatedTransportBounds`; then check feature/backend/protocol availability.
Thus a malformed legacy config fails deterministically before D.6's reserved
`UnsupportedBackend`. Disabled transport still validates explicitly supplied
shared fields, rejects every explicit legacy-only retry field with
`ConfigFieldNotApplicable { target: OtlpConfigTarget::Disabled, .. }`, yields
`BackendTransportBounds::Disabled`, and never constructs a network
provider/worker. SDK inapplicability instead records
`OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk)`.

Configuration variants carry reviewable payloads:

```rust
ZeroDuration { field: OtlpConfigField, origin: ValueOrigin }
DurationOverflow { field: OtlpConfigField, raw: u64, origin: ValueOrigin }
InvalidBoundOrdering {
    lower: ResolvedField<u64>,
    upper: ResolvedField<u64>,
}
InvalidJitterPercent { field: OtlpConfigField, raw: u8, origin: ValueOrigin }
ConfigFieldNotApplicable { field: OtlpConfigField, target: OtlpConfigTarget }
InsecureTransportRejected { backend: ExporterBackend }
InvalidEndpoint { field: OtlpConfigField }
InvalidHeader { field: OtlpConfigField }
TransportConstructionFailed { backend: ExporterBackend, source: Diagnostic }
```


### Stable failure inventory owner

Use obs-d-12's complete stable failure inventory verbatim as the read-only source for ConfigFailure/ExportError codes and mappings. obs-d-21 implements validation/conversion to those types; it does not define error variants or registry string values.

## OTLP crate registration contract

`crates/sc-observability-otlp/src/lib.rs` declares the implementation modules once for all consumers and `Cargo.toml` declares the features that select them:

```rust
mod config;
mod contracts;
mod lifecycle;
mod testing;
#[cfg(test)] mod contract_tests;
mod constants;
mod assembly;
mod error_codes;
mod projectors;
#[cfg(feature = "otlp-sdk")] mod sdk;
#[cfg(feature = "legacy-http-json")] mod legacy_http_json;
```

```toml
[features]
otlp-sdk = []
legacy-http-json = []
```

## Dependency and module declaration contract

D.21 records the optional otlp-sdk feature's reviewed opentelemetry family pins and the legacy-http-json feature's reqwest =0.12.28 (blocking/json/rustls-tls, default features off), httpdate =1.0.3, getrandom and tokio rt/sync allowlist in owned manifests/lock/boundaries against the read-only architecture section6/ADR-019 amendment supplied by obs-d-12. Legacy requires no caller runtime and no SDK/tonic dependency; reqwest's internal Tokio graph is explicit. Existing boundary validators and Cargo feature tests enforce this; no new parallel validator framework.

The only exporter traits and ExporterSet are pub(crate), as OTLP-011 and ADR-018 require. No public Exporter, undefined Signal, public ExporterSet or facade re-export is added. D.21's factory contract and recording test doubles compile without real SDK/legacy implementations. Module declarations expose separate contract and implementation files so D.7 and D.8 are siblings. obs-d-21 registers future log-settings/legacy examples and workspace members; obs-d-7 owns examples/otlp-sdk/Cargo.toml as the explicit exception.

Contract tests are self-contained and do not require completed production adapters. Any compatibility scaffolding needed for the existing workspace is explicitly transitional contract glue, not a claimed production implementation; each retiring consumer is named above, and D.18's migration gate rejects any leftover obsolete wrapper. Contract closure requires workspace compilation, not production collector behavior.

## Module roots and standalone contract closure

obs-d-21 owns and stubs sdk/mod.rs and legacy_http_json/mod.rs, including their module declaration lines; obs-d-7/8 own only their respective implementation.rs and tests.rs after the staged-file handoff; examples/otlp-sdk/Cargo.toml is owned by obs-d-7. Each module root contains `mod implementation;` and `#[cfg(test)] mod tests;`. obs-d-21 creates compilable placeholders for those declared files and creates assembly.rs/projectors.rs placeholders before declaring them in lib.rs. obs-d-5 replaces its assembly/projector placeholders after contract sanity. Module roots remain read-only to implementation consumers. Contract closure requires obs-d-12-sanity but remains independent of obs-d-13; it cannot rely on the logging contract landing first.

## Boundary enforcement scope

Delete the proposed six additional mechanical boundary rules. Rust visibility, types and Cargo feature/dependency graphs enforce private exporter ownership and crate edges; do not duplicate them with textual signature gates. Keep the existing source/provenance and ATM-specific env/import checks, which Cargo cannot infer. If a missing provenance/env check is demonstrated, extend validate_repo_boundaries.sh or its existing underlying consumer, with the failing case, rather than create a new validator. No speculative rule or new incident claim is authorized.



## Handoff from obs-d-12

After obs-d-12-sanity, consume its canonical types v2 error/model surface, complete stable registry and neutral_contracts.rs tests. All ConfigFailure and ExportError variants remain defined in sc-observability-types. Shared docs/requirements.md, docs/architecture.md and docs/api-design.md, including section6, remain owned by obs-d-12 and are read-only normative inputs. Record exact transport dependency pins in owned Cargo.lock/boundary manifests; do not create a second allowlist definition.

Handoff from obs-d-12: version literal in crates/sc-observability-types/Cargo.toml. Lead ruling 01M3E7Z46AA9DNNMC73BDE37JG authorizes changing only this version literal atomically with the workspace version/dependency pins. The file fence stays with obs-d-12; no other types-manifest edit is authorized here. This is the sole named exception to the otherwise disjoint same-wave fences.

## Types-owned errors and OTLP-owned checked bounds

obs-d-12 owns ErrorContext and every error variant/code. obs-d-21 owns ValidatedTransportBounds, QueueCapacity and QueueByteCapacity; arithmetic is checked and each terminal admission releases both credits once. Zero/overflow/above-64MiB byte capacities map to the already defined InvalidQueueByteCapacity/OTLP_CONFIG_QUEUE_BYTE_CAPACITY; record capacity outside 1..=65536 maps to InvalidQueueCapacity/OTLP_CONFIG_QUEUE_CAPACITY. Defaults and limits live in the OTLP constants module; accessors never bypass checked construction. OTLP error_codes.rs only re-exports types error_codes.rs::otlp constants.
## Handoff to obs-d-8 (wave 2)

Created/staged by obs-d-21, owned by obs-d-8 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/legacy_http_json/implementation.rs`
- `crates/sc-observability-otlp/src/legacy_http_json/tests.rs`

## Handoff to obs-d-6 (wave 2)

Created/staged by obs-d-21, owned by obs-d-6 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/lifecycle.rs`
- `crates/sc-observability-otlp/src/lifecycle_tests.rs`

## Handoff to obs-d-7 (wave 2)

Created/staged by obs-d-21, owned by obs-d-7 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/sdk/implementation.rs`
- `crates/sc-observability-otlp/src/sdk/tests.rs`


## Handoff to obs-d-5

Handoff to obs-d-5: obs-d-21 consumes obs-d-12 validated neutral signal/MetricModelError contracts and stages OTLP module scaffolding; obs-d-5 consumes those artifacts and owns only OTLP assembly/projector behavior, with D.21-owned files read-only after handoff. assembly.rs and projectors.rs are created by obs-d-21 and owned by obs-d-5 from wave 2; the normative types/config/module roots stay read-only.



## Handoff to obs-d-18 (wave 3)

obs-d-18 consumes the completed OTLP contract and atomic workspace-version artifact after obs-d-21-sanity, then owns facade composition and compatibility activation in these files:

- `crates/sc-observability-otlp/src/config.rs`
- `crates/sc-observability-otlp/src/contracts.rs`
- `crates/sc-observability-otlp/src/lib.rs`

## Release gate

## Complete module registration and byte-capacity closure

The authoritative `lib.rs` registration list is `mod config;`, `mod contracts;`,
`mod lifecycle;`, `#[cfg(test)] mod lifecycle_tests;`, `mod testing;`,
`#[cfg(test)] mod contract_tests;`, `mod constants;`, `mod assembly;`,
`mod error_codes;`, `mod projectors;`, `#[cfg(feature = "otlp-sdk")] mod sdk;`,
and `#[cfg(feature = "legacy-http-json")] mod legacy_http_json;`.

The wire table also contains `queue_byte_capacity` for both backends, default
`16 MiB`, checked in `1..=64 MiB`. Ordered validation checks `queue_capacity`
first and then `queue_byte_capacity`, returning `InvalidQueueCapacity` or
`InvalidQueueByteCapacity` before backend-specific retry bounds.

This contract releases obs-d-5–8 after obs-d-21-sanity; obs-d-18 additionally waits on that gate. Final production transports, collector equivalence and release approvals remain their named downstream gates. The root workspace invariant applies.

## Acceptance criteria

- [ ] #1: cargo test -p sc-observability-otlp --lib contract_tests --all-features --locked runs nonzero validation_order, resolved_defaults, stable_failure_codes, record_and_byte_capacity and fake_exporter_contract cases; byte/record zero/overflow/upper bounds use the distinct types-owned variants/codes.
- [ ] #1: cargo test -p sc-observability-types --test neutral_contracts --locked passes against obs-d-12's unchanged contract test suite; no types test file is edited.
- [ ] #2: every declared module has a compiling stub; cargo check --workspace --all-features --locked and bash scripts/ci/validate_repo_boundaries.sh pass without a real backend. sdk/mod.rs and legacy_http_json/mod.rs contain mod implementation and cfg(test) mod tests.
- [ ] #2–3: Cargo metadata, all owned dependency pins and resolved lockfiles report a consistent 2.0 graph after the atomic bump, including the authorized types-manifest version literal. Exact transport pins match the consumed section6/ADR-019 allowlist and remain in the owned boundary record.
- [ ] #3: OTLP-005/020/021 and PHD-003/004 config/lifecycle semantics match the read-only normative specification; no environment override or competing failure registry is introduced. Release documentation closes in obs-d-18. Root workspace invariant passes.
