# 1.4.1 to 2.0 migration reference

The frozen compatibility baseline is 1.4.1. ADR-017 authorizes only enumerated,
reviewed breaks; it does not waive arbitrary source or wire changes. This
reference separates the currently observed DTO signature change from the
canonical error/config/signal contract whose complete activation is D18 work.
No publication or final baseline approval is implied.

## from_core_health returns the health value

`sc_observability_dto::from_core_health` now returns `LogHealthDto` directly,
instead of `Result<LogHealthDto, Failure>`. It projects already validated
core health and level state without inventing a possible conversion failure.

```rust,ignore
// 1.4.1
let health = sc_observability_dto::from_core_health(logging, level)?;
// 2.0
let health = sc_observability_dto::from_core_health(logging, level);
```

Remove `?`, `unwrap`, `expect`, `map_err`, and `Ok`/`Err` matching at this call.
A caller whose own interface remains fallible can explicitly return
`Ok(from_core_health(logging, level))`; do not reintroduce a fake conversion
failure. The DTO still has schema version 1, the actual logging/level state,
and `bridge: None` for an independent core logger. This signature change alone
does not change that wire payload or grant a schema-version bump.

The exact old/new public-API signatures and authority are recorded in
[`release/public-api-major-breaks.toml`](../release/public-api-major-breaks.toml).
The entry must remain present in the consumed manifest, even after all local
callers have migrated.

## Error and custom extension points

Follow [Error API migration](migrate-error-api.md) for all nine wrapper families,
named cause variants, source/context retention and custom sink/projector
migration. Import canonical same-name enums after activation, not permanent
parallel typed wrappers. Keep open trait implementations and host-owned logger
lifecycle semantics intact.

## Enumerated public 2.0 removals

The D18 manifest records every observed 1.4.1 removal rather than granting a
crate-wide waiver. `sc-observability-types` retires wrapper structs,
`ClassifiedError`, `*Failure` conversions, kind aliases, and `TelemetryError`
in favor of the ADR-017 named enums. `sc-observability` and `sc-observe` retire
legacy logging/routing errors and the duplicate `*_typed` surface; unsuffixed
2.0 methods use canonical errors where a same-method replacement exists.

`sc-observability-log` retires its local lifecycle error enums and their
variants in favor of the shared `InitError`, `FlushError`, and `ShutdownError`
families. `sc-observability-otlp` retires the old constants/error-code modules
and changes construction, export, lifecycle, and span assembly signatures to
the canonical configuration/error contracts. `sc-observability-dto` retires
the legacy `Failure` and generic `WireEnvelope` forms in favor of the canonical
DTO/schema projection. A removed item with no exact added public line has an
empty `new` field in the manifest; that is intentional and must not be
represented as a compatibility promise.

## OTLP configuration and defaults

Applications own environment/config translation. Construct validated
`TelemetryConfig` explicitly with backend and protocol; shared crates must
not override explicit values from ambient `OTEL_*` variables. SDK operation
requires the caller's Tokio runtime. Legacy HTTP/JSON uses its bounded plain
thread worker. Unsupported enabled combinations fail construction with a typed
error, rather than falling back to the disabled no-network implementation.
The exporter set and backend traits stay crate-private.

Use the D21 config contract in `crates/sc-observability-otlp/src/config.rs` and
`constants.rs`, not historical transport defaults. The declared defaults are
3,000 ms per request; 30,000 ms lifecycle flush/shutdown; queue capacity 1,024
records and 16 MiB aggregate bytes; 256-record log/trace/metric batches; and a
5,000 ms metric interval. Legacy retries default to three retries, 250 ms
initial backoff, 5,000 ms maximum backoff, a 30,000 ms sequence timeout,
5,000 ms Retry-After cap and 20 percent jitter. Defaults apply only when a
value is absent; a rejected explicit value is not silently replaced. Preserve
typed admission, deadline, flush and shutdown failures at the public boundary.

## Neutral signals and serialization

Use checked neutral metric/span constructors. Preserve integer signedness,
finite floating-point values, histogram buckets/bounds and temporal data,
resource/scope fields, status and trace/parent identities. Do not pass a lossy
JSON-shaped intermediate into an exporter.

Native `AttributeValue` serde uses a `kind` tag and `data` payload (omitted for
`null`), including distinct `int` and `uint` cases. This accepted tagged native
representation is distinct from OTLP JSON encoding and from any separately
versioned language DTO envelope. Do not infer one wire format from another;
use the existing schema/conformance fixtures and serializers. Canonical errors
use their documented named-kind representation. A source-compatible Rust type
change is not proof of wire compatibility.

## Qualification and rollout

Run the existing public-API semver gate against 1.4.1. It now matches exact
removed/changed API lines to enumerated manifest entries; a different
replacement signature, missing entry, or tool failure is an error. New D18
breaks need individual evidence before being added. No crate-wide or wildcard
waiver exists. The structural `cargo semver-checks` gate also remains required;
a manifest entry does not waive its failures, including breaking additions
that a text diff alone cannot classify.

```sh
python3 scripts/ci/validate_public_api_semver.py --crate sc-observability-dto
python3 scripts/ci/validate_public_api_semver.py
```

The first command is scoped evidence only. The second is required for the
whole release and may still fail on unenumerated changes while D18 is in
progress. To verify omission enforcement, copy the manifest in a scratch
checkout, remove the observed `dto-from-core-health-infallible` entry, run the
same scoped command and require nonzero status with `unlisted API break`;
restore it and require success. Never generate or approve the final 2.0
baseline from a scoped pass.

Finish with canonical migration fixture execution, real Python/TypeScript/
Tauri composition, schema checks and all-features workspace tests. Keep the
existing shared npm channel and Python `abi3-py310` / `cp310-abi3` /
`Requires-Python >=3.10` policy: six native platforms, 29 installed-suite cells,
with Windows ARM64 covering 3.11–3.14. Changing the floor, cap or matrix needs
a separate compatibility decision.
