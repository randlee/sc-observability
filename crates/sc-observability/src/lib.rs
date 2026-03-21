pub use sc_observability_types::{MetricRecord, OtelConfig, TraceRecord};

pub fn current_otel_health() -> &'static str {
    "unknown"
}
