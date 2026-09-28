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
