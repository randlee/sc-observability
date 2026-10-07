//! Canonical log/resource projections for OTLP exporter records.

use std::collections::BTreeMap;

use crate::contracts::{ExportRecord, LogRecord};
use sc_observability_types::otlp::{OtlpInstrumentationScope, OtlpLogRecord, OtlpResource};
use sc_observability_types::{LogEvent, ServiceName};

pub(crate) fn resource(service: &ServiceName) -> OtlpResource {
    OtlpResource {
        attributes: BTreeMap::from_iter([(
            "service.name".to_owned(),
            sc_observability_types::v2::AttributeValue::String(service.as_str().to_owned()),
        )]),
        schema_url: None,
    }
}

pub(crate) fn log_record(event: &LogEvent) -> ExportRecord<LogRecord> {
    ExportRecord {
        resource: resource(&event.service),
        scope: OtlpInstrumentationScope::default(),
        record: OtlpLogRecord {
            event: event.clone(),
            trace_flags: sc_observability_types::v2::TraceFlags::default(),
            attributes: BTreeMap::new(),
        },
    }
}
