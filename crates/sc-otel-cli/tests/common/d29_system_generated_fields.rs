/// Canonical observation-time and correlated-ID fields generated from input.
pub const D29_SYSTEM_GENERATED_FIELDS: &[(&str, &str)] = &[
    ("logs", "observed_time"),
    ("logs", "trace_id"),
    ("logs", "span_id"),
    ("spans", "trace_id"),
    ("spans", "span_id"),
];
