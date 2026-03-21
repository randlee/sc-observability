use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtelConfig {
    pub endpoint: Option<String>,
    pub service_name: String,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            service_name: "unknown-service".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRecord {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricRecord {
    pub name: String,
}
