# Compatible 1.x migration reference

The Phase F migration rule is
[ADR-020](architecture.md#adr-020-compatible-1x-adoption-of-phase-d): ship a
deprecated `v1` item before removing it. The stock public-API check records
accepted 1.5.0 surfaces. Start with
[Adopting the compatible 1.x release](migration/compatible-1x.md).

The later sections describe the canonical error, configuration and signal
contract. Phase F may remove only the items allowed by PHF-002; no major
version bump is implied.

## from_core_health keeps its released signature

`sc_observability_dto::from_core_health` keeps its released 1.4.1 signature
and returns `Result<LogHealthDto, Failure>`. It is a thin wrapper that always
returns `Ok` from the canonical projection, so existing callers need no
change:

```rust,ignore
// 1.4.1 and compatible 1.x
let health = sc_observability_dto::from_core_health(logging, level)?;
```

The infallible canonical projection is the new
`sc_observability_dto::from_canonical_core_health`, which returns
`LogHealthDto` directly. Call it when you want the projection without a
`Result`:

```rust,ignore
let health = sc_observability_dto::from_canonical_core_health(logging, level);
```

Do not wrap it in a fake conversion failure; a caller whose own interface
remains fallible can return `Ok(from_canonical_core_health(logging, level))`.
Both functions produce the same DTO: schema version 1, the actual logging and
level state, and `bridge: None` for an independent core logger. Neither changes
the wire payload or grants a schema-version bump.

## Error and custom extension points

Follow [Error API migration](migrate-error-api.md) for all nine wrapper families,
named cause variants, source/context retention and custom sink/projector
migration. Import canonical same-name enums after activation, not permanent
parallel typed wrappers. Keep open trait implementations and host-owned logger
lifecycle semantics intact.

## Phase F deprecate-before-remove path

PHF-002 governs the 1.x migration path. The next release remains 1.5.0: every
released 1.x public item that continues to be exposed ships behind its
default-on `v1` feature with a deprecation note naming its v2 replacement (or
the Phase F migration guide when it has none). This transition does not require
a major-version bump.

The release after that deprecation release may remove the `v1` modules and
features. Deprecated items need not retain compatibility adapters, behavioral
tests, or baseline comparison. Phase F may delete items already deprecated in
published 1.4.1, never-released items, and internal plumbing now; canonical
code must not use a `v1` item. Consumers should use the named canonical
replacement in the deprecation note rather than treating the retained legacy
path as a compatibility promise.

### EmitError pattern matching

`sc_observability_log::EmitError` is non-exhaustive in 2.0. Downstream code
matching it must include a wildcard arm for future variants. `NotInstalled`
now follows the eight 1.x variants, preserving their discriminants; use the
typed code, remediation, or drop cause instead of relying on numeric casts.

## OTLP configuration and defaults

Applications own environment/config translation. Construct validated
`TelemetryConfig` explicitly with backend and protocol; shared crates must
not override explicit values from ambient `OTEL_*` variables. SDK operation
requires the caller's Tokio runtime. Synchronous HTTP/JSON uses its bounded plain
thread worker. Unsupported enabled combinations fail construction with a typed
error, rather than falling back to the disabled no-network implementation.
The exporter set and backend traits stay crate-private.

Use the D21 config contract in `crates/sc-observability-otlp/src/config.rs` and
`constants.rs`, not historical transport defaults. The declared defaults are
3,000 ms per request; 30,000 ms lifecycle flush/shutdown; queue capacity 1,024
records and 16 MiB aggregate bytes; 256-record log/trace/metric batches; and a
5,000 ms metric interval. Synchronous HTTP retries default to three retries, 250 ms
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

Run the stock public-API check against the committed 1.5.0 baselines. Candidate
setup writes only under a caller-owned target directory; the check renders the
already-created rustdoc JSON with the stock tool and rejects every removed or
changed accepted line. Additions pass. A tool failure is an error.

```sh
python3 scripts/ci/stock_public_api.py setup --target-dir /tmp/sc-observability-api
python3 scripts/ci/stock_public_api.py check --target-dir /tmp/sc-observability-api
```

The first command prepares the candidate data; the second checks every
published crate. Only the explicit `release-cut` action may update a committed
baseline.

Finish with canonical migration fixture execution, real Python/TypeScript/
Tauri composition, schema checks and all-features workspace tests. Keep the
existing shared npm channel and Python `abi3-py310` / `cp310-abi3` /
`Requires-Python >=3.10` policy: six native platforms, 29 installed-suite cells,
with Windows ARM64 covering 3.11–3.14. Changing the floor, cap or matrix needs
a separate compatibility decision.
