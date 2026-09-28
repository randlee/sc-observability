# Phase D accepted limitations

## DTO attribute projection

On 2026-09-27, Rand directed that this issue be documented and deferred rather
than block Phase D landing or unrelated work. Tracked as
`obs-dto-attribute-projection` in the backlog, outside the Phase D completion gates.
This is accepted scope reduction, not a fix or a QA PASS.

DTO-to-native conversion currently feeds generic JSON attribute values into a
parser expecting the native tagged `AttributeValue` representation. An attributed
metric can therefore fail conversion, including the fixture containing an unsigned
64-bit maximum and a string label. This is a representation mismatch, not merely
JSON numeric precision loss. Round trips for nonempty attributes are not guaranteed;
use empty attributes for the currently verified metric DTO path. Native Rust
histogram use is unaffected by this DTO conversion issue.

The active `histogram_conversion_is_lossless` test retains count, bucket count,
bound, sum, temporality, timestamp, and DTO round-trip assertions using empty
attributes. Full-width counts remain decimal strings on the wire and bounds/sums
remain f64. The separate `metric_attributes_round_trip` reproduction is explicitly
ignored pending follow-up; CI reports it as ignored, not passed.

To reproduce the deferred issue:

```sh
cargo test --locked -p sc-observability-dto --test canonical_contracts \
  metric_attributes_round_trip -- --ignored --exact
```

A future change must define and implement attribute projection in both directions,
including native signed/unsigned integer distinctions, before enabling that test.
No converter redesign is required by this Phase D disposition.

## Future non-exhaustive native error variants

`sc-observability-types` exposes the native error enums as `#[non_exhaustive]`.
At this revision, all 12 `ExportError` variants have explicit DTO mappings, so
downstream or integration tests cannot safely construct a future variant to
exercise the local wildcard-to-`Internal` fallback. The fallback remains in the
production match arms for variants added in a later revision; executable
coverage for such a variant must be added with that variant's producer. This
limitation does not affect decoded unknown wire discriminants, which retain the
separate `UnknownRemote` result and are covered by the existing contract test.
