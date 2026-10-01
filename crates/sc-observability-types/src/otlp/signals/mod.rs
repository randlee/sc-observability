//! Full neutral signal payloads pinned to OpenTelemetry proto v1.10.0.
//!
//! Pin: Rust crate `opentelemetry-proto =0.33.0`, upstream tag `v1.10.0`.
//! Profiles are `opentelemetry.proto.profiles.v1development`, exported to
//! `/v1development/profiles`.
//!
//! | Proto message (v1.10.0) | Fields | Neutral type |
//! | --- | --- | --- |
//! | `common.AnyValue` | `string_value`, `bool_value`, `int_value`, `double_value`, `array_value`, `kvlist_value`, `bytes_value` | `AnyValue::{String, Bool, Int, Double, Array, KvList, Bytes}` (plus `UInt`; see the policy below) |
//! | `common.AnyValue` | `string_value_strindex` | `AnyValue::StringIndex(StringIndex)`: an index into `ProfilesDictionary.string_table`, valid only inside a profiles payload |
//! | `common.KeyValue` | `key`, `value` | `KeyValues` entry `(String, AnyValue)`; duplicate keys rejected |
//! | `common.KeyValue` | `key_strindex` | `AttributeKey::Index(StringIndex)`, under the same profiles-only rule |
//! | `common.InstrumentationScope` | `name`, `version`, `attributes`, `dropped_attributes_count` | `InstrumentationScope` |
//! | `common.EntityRef` | `schema_url`, `type`, `id_keys`, `description_keys` | `EntityRef` |
//! | `resource.Resource` | `attributes`, `dropped_attributes_count`, `entity_refs` | `Resource` |
//! | `Resource*`/`Scope*` wrappers | `schema_url` | `Resource.schema_url`, `InstrumentationScope.schema_url` |
//! | `logs.LogRecord` | `time_unix_nano`, `observed_time_unix_nano`, `severity_number`, `severity_text`, `body`, `attributes`, `dropped_attributes_count`, `flags`, `trace_id`, `span_id`, `event_name` | `LogPoint` |
//! | `trace.Span` | `trace_id`, `span_id`, `trace_state`, `parent_span_id`, `flags`, `name`, `kind`, `start_time_unix_nano`, `end_time_unix_nano`, `attributes`, `dropped_attributes_count`, `events`, `dropped_events_count`, `links`, `dropped_links_count`, `status` | `SpanPoint` |
//! | `trace.Span.Event` | `time_unix_nano`, `name`, `attributes`, `dropped_attributes_count` | `SpanEventPoint` |
//! | `trace.Span.Link` | `trace_id`, `span_id`, `trace_state`, `attributes`, `dropped_attributes_count`, `flags` | `SpanLinkPoint` |
//! | `trace.Status` | `message`, `code` | `SpanStatusPoint` |
//! | `metrics.Metric` | `name`, `description`, `unit`, `metadata`, `data` | `MetricStream` |
//! | `metrics.Gauge` / `Sum` / `Histogram` / `ExponentialHistogram` / `Summary` | `data_points`, `aggregation_temporality`, `is_monotonic` | `MetricData::{Gauge, Sum, Histogram, ExponentialHistogram, Summary}` |
//! | `metrics.NumberDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `as_double`/`as_int`, `exemplars`, `flags` | `NumberPoint` |
//! | `metrics.HistogramDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `bucket_counts`, `explicit_bounds`, `exemplars`, `flags`, `min`, `max` | `HistogramDataPoint` |
//! | `metrics.ExponentialHistogramDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `scale`, `zero_count`, `positive`, `negative`, `flags`, `exemplars`, `min`, `max`, `zero_threshold` | `ExponentialHistogramDataPoint`, `ExponentialBuckets` |
//! | `metrics.SummaryDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `quantile_values`, `flags` | `SummaryDataPoint`, `ValueAtQuantile` |
//! | `metrics.Exemplar` | `filtered_attributes`, `time_unix_nano`, `as_double`/`as_int`, `span_id`, `trace_id` | `Exemplar` |
//! | `profiles.ProfilesDictionary` | `mapping_table`, `location_table`, `function_table`, `link_table`, `string_table`, `attribute_table`, `stack_table` | `ProfilesDictionary` |
//! | `profiles.Profile` | `sample_type`, `samples`, `time_unix_nano`, `duration_nano`, `period_type`, `period`, `profile_id`, `dropped_attributes_count`, `original_payload_format`, `original_payload`, `attribute_indices` | `Profile` |
//! | `profiles.Sample`, `ValueType`, `Mapping`, `Stack`, `Location`, `Line`, `Function`, `Link`, `KeyValueAndUnit` | all fields | same-named neutral structs |
//! | any `double` field | NaN / ±Inf | `OtlpDouble` carries every IEEE-754 value, including NaN and ±Inf; see the non-finite policy |
//! | `tracez.proto` | all | excluded — out of scope (lead ruling P5, 2026-10-01): zPages, not an OTLP payload |
//! | `collector.*.Export*ServiceRequest/Response` | all | not neutral: built and parsed by the d-34 sync-http encoder |
//!
//! Enumerations: `SeverityNumber` 0–24, `SpanKind` including `Unspecified`, and
//! `StatusCode::{Unset, Ok, Error}`. `AggregationTemporality::Unspecified` is
//! rejected as invalid input, because the spec forbids it on Sum and Histogram.
//! `DataPointFlags(u32)` keeps `NO_RECORDED_VALUE`. Span and log flags are kept
//! as `u32`.
//!

mod values;
#[doc(inline)]
pub use values::{
    AnyValue, AttributeKey, KeyValues, OtlpDouble, SignalValidationError, StringIndex, TraceState,
};

mod metrics;
mod records;
#[doc(inline)]
pub use metrics::{AggregationTemporality, DataPointFlags, MetricData, NumberValue};
#[doc(inline)]
pub use records::{
    EntityRef, Exemplar, ExponentialBuckets, ExponentialHistogramDataPoint, HistogramDataPoint,
    InstrumentationScope, LogPoint, MetricStream, NumberPoint, Resource, ResourceRecord,
    SeverityNumber, SpanEventPoint, SpanKindPoint, SpanLinkPoint, SpanPoint, SpanStatusPoint,
    StatusCode, SummaryDataPoint, ValueAtQuantile,
};

mod profiles;
#[doc(inline)]
pub use profiles::{
    Function, KeyValueAndUnit, Line, Location, Mapping, Profile, ProfileLink, ProfilesDictionary,
    Sample, Stack, ValueType,
};
