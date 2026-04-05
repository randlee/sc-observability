use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{MetricName, ServiceName, Timestamp};

/// Supported metric aggregation shapes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MetricKind {
    /// Monotonic counter metric.
    Counter,
    /// Gauge metric representing the latest value.
    Gauge,
    /// Histogram metric representing a sampled distribution.
    Histogram,
}

/// Structured metric observation projected from routing or telemetry layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricRecord {
    /// UTC metric timestamp.
    pub timestamp: Timestamp,
    /// Service that emitted the metric.
    pub service: ServiceName,
    /// Stable metric name.
    pub name: MetricName,
    /// Aggregation shape for the metric.
    pub kind: MetricKind,
    /// Numeric metric value.
    pub value: f64,
    /// Optional UCUM unit string, for example `ms`, `By`, or `1`.
    pub unit: Option<String>,
    /// Structured metric attributes.
    pub attributes: Map<String, Value>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn service_name() -> ServiceName {
        ServiceName::new("sc-observability").expect("valid service name")
    }

    fn metric_name() -> MetricName {
        MetricName::new("obs.events_total").expect("valid metric name")
    }

    #[test]
    fn metric_record_round_trips_through_serde() {
        let metric = MetricRecord {
            timestamp: Timestamp::UNIX_EPOCH,
            service: service_name(),
            name: metric_name(),
            kind: MetricKind::Counter,
            value: 4.0,
            unit: Some("1".to_string()),
            attributes: Map::from_iter([("state".to_string(), json!("running"))]),
        };

        let encoded = serde_json::to_string(&metric).expect("serialize metric");
        let decoded: MetricRecord = serde_json::from_str(&encoded).expect("deserialize metric");
        assert_eq!(decoded, metric);
    }
}
