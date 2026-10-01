//! Transport-neutral OTLP signal envelopes.
//!
//! The types in this module retain the resource and instrumentation context
//! which OTLP transport requests require while keeping SDK and HTTP details in
//! `sc-observability-otlp`.

/// Full transport-neutral signal payloads.
pub mod signals;

use crate::LogEvent;
use crate::v2::{Attributes, MetricRecord, SpanEnded, SpanEvent, SpanRecord, TraceFlags};

/// Resource identity attached to an OTLP export request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OtlpResource {
    /// Resource attributes shared by the request's records.
    pub attributes: Attributes,
    /// Optional resource schema URL.
    pub schema_url: Option<String>,
}

/// Instrumentation scope attached to an OTLP export request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OtlpInstrumentationScope {
    /// Instrumentation library name.
    pub name: String,
    /// Optional instrumentation library version.
    pub version: Option<String>,
    /// Optional instrumentation schema URL.
    pub schema_url: Option<String>,
    /// Scope attributes.
    pub attributes: Attributes,
}

/// One neutral signal with lossless OTLP resource and scope context.
#[derive(Debug, Clone, PartialEq)]
pub struct OtlpRecord<T> {
    /// Resource associated with this record.
    pub resource: OtlpResource,
    /// Instrumentation scope associated with this record.
    pub scope: OtlpInstrumentationScope,
    /// Signal payload.
    pub record: T,
}

/// A log signal ready for OTLP wire projection.
#[derive(Debug, Clone, PartialEq)]
pub struct OtlpLogRecord {
    /// Existing neutral logging event.
    pub event: LogEvent,
    /// W3C trace flags associated with `event.trace`.
    pub trace_flags: TraceFlags,
    /// Additional OTLP log attributes.
    pub attributes: Attributes,
}

/// A completed span ready for OTLP wire projection.
#[derive(Debug, Clone, PartialEq)]
pub struct OtlpCompleteSpan {
    /// Ended span record.
    pub record: SpanRecord<SpanEnded>,
    /// Ordered span events.
    pub events: Vec<SpanEvent>,
}

/// Groups records by resource and then instrumentation scope without reordering.
///
/// Resource groups retain their first-seen order. Within each resource group,
/// scope groups retain their first-seen order and their records retain input
/// order. This permits a transport to form OTLP requests without assigning one
/// record another record's resource or scope.
#[must_use]
pub fn group_records_by_resource_and_scope<T: Clone>(
    records: &[OtlpRecord<T>],
) -> Vec<OtlpResourceGroup<T>> {
    let mut resources = Vec::new();
    for item in records {
        let resource_group = if let Some(group) = resources
            .iter_mut()
            .find(|group: &&mut OtlpResourceGroup<T>| group.resource == item.resource)
        {
            group
        } else {
            let index = resources.len();
            resources.push(OtlpResourceGroup {
                resource: item.resource.clone(),
                scopes: Vec::new(),
            });
            &mut resources[index]
        };

        if let Some(scope_group) = resource_group
            .scopes
            .iter_mut()
            .find(|group| group.scope == item.scope)
        {
            scope_group.records.push(item.record.clone());
        } else {
            resource_group.scopes.push(OtlpScopeGroup {
                scope: item.scope.clone(),
                records: vec![item.record.clone()],
            });
        }
    }
    resources
}

/// A resource-homogeneous ordered group of OTLP records.
#[derive(Debug, Clone, PartialEq)]
pub struct OtlpResourceGroup<T> {
    /// Shared resource for every scope group.
    pub resource: OtlpResource,
    /// Ordered scope subgroups for this resource.
    pub scopes: Vec<OtlpScopeGroup<T>>,
}

/// A scope-homogeneous ordered group of OTLP records.
#[derive(Debug, Clone, PartialEq)]
pub struct OtlpScopeGroup<T> {
    /// Shared instrumentation scope for every record.
    pub scope: OtlpInstrumentationScope,
    /// Records in original input order.
    pub records: Vec<T>,
}

/// Canonical OTLP metric envelope.
pub type OtlpMetricRecord = OtlpRecord<MetricRecord>;

#[cfg(test)]
mod tests {
    use super::{
        OtlpInstrumentationScope, OtlpRecord, OtlpResource, group_records_by_resource_and_scope,
    };

    #[test]
    fn grouping_preserves_resource_scope_and_record_order() {
        let first_resource = OtlpResource {
            schema_url: Some("https://example.test/resource/one".to_owned()),
            ..OtlpResource::default()
        };
        let second_resource = OtlpResource {
            schema_url: Some("https://example.test/resource/two".to_owned()),
            ..OtlpResource::default()
        };
        let first_scope = OtlpInstrumentationScope {
            name: "first".to_owned(),
            ..OtlpInstrumentationScope::default()
        };
        let second_scope = OtlpInstrumentationScope {
            name: "second".to_owned(),
            ..OtlpInstrumentationScope::default()
        };

        let groups = group_records_by_resource_and_scope(&[
            OtlpRecord {
                resource: first_resource.clone(),
                scope: first_scope.clone(),
                record: 1_u8,
            },
            OtlpRecord {
                resource: second_resource.clone(),
                scope: first_scope.clone(),
                record: 2_u8,
            },
            OtlpRecord {
                resource: first_resource.clone(),
                scope: second_scope.clone(),
                record: 3_u8,
            },
            OtlpRecord {
                resource: first_resource.clone(),
                scope: first_scope.clone(),
                record: 4_u8,
            },
        ]);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].resource, first_resource);
        assert_eq!(groups[0].scopes[0].scope, first_scope);
        assert_eq!(groups[0].scopes[0].records, vec![1, 4]);
        assert_eq!(groups[0].scopes[1].scope, second_scope);
        assert_eq!(groups[0].scopes[1].records, vec![3]);
        assert_eq!(groups[1].resource, second_resource);
        assert_eq!(groups[1].scopes[0].records, vec![2]);
    }
}
