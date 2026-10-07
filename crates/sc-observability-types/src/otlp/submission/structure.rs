//! Typed structural validation of canonical envelopes.
//!
//! Every `AnyValue`, `AttributeKey` and attribute collection is reached by
//! destructuring the neutral records, so adding a field or variant is a
//! compile error here instead of a silently skipped rule. Paths are kept as a
//! borrowed parent chain and rendered only when a rule fails.
use super::{ProfilesSubmission, SubmissionError, error_codes, errors::context};
use crate::constants;
use crate::otlp::signals::{
    AnyValue, AttributeKey, Exemplar, ExponentialHistogramDataPoint, HistogramDataPoint,
    InstrumentationScope, KeyValueAndUnit, KeyValues, LogPoint, MetricData, MetricStream,
    NumberPoint, Resource, ResourceRecord, SpanEventPoint, SpanLinkPoint, SpanPoint,
    SummaryDataPoint,
};
use std::fmt::{self, Write};

/// One step of a payload path.
#[derive(Clone, Copy)]
enum Segment<'a> {
    Field(&'a str),
    Index(usize),
}

/// Borrowed path from the envelope root to the value being checked.
#[derive(Clone, Copy)]
pub(super) struct Path<'a> {
    parent: Option<&'a Path<'a>>,
    segment: Segment<'a>,
}
impl<'a> Path<'a> {
    pub(super) const fn root(family: &'a str) -> Self {
        Self {
            parent: None,
            segment: Segment::Field(family),
        }
    }
    fn field(&'a self, name: &'a str) -> Self {
        Self {
            parent: Some(self),
            segment: Segment::Field(name),
        }
    }
    fn index(&'a self, index: usize) -> Self {
        Self {
            parent: Some(self),
            segment: Segment::Index(index),
        }
    }
    fn render(&self) -> String {
        self.to_string()
    }
}
impl fmt::Display for Path<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(parent) = self.parent {
            fmt::Display::fmt(parent, f)?;
            if let Segment::Field(_) = self.segment {
                f.write_char('.')?;
            }
        }
        match self.segment {
            Segment::Field(name) => f.write_str(name),
            Segment::Index(index) => write!(f, "[{index}]"),
        }
    }
}

fn too_many(path: &Path<'_>, found: usize, limit: usize) -> SubmissionError {
    SubmissionError::validation(
        path.render(),
        format!("{found} entries exceed the limit of {limit}"),
    )
}

/// Rejects a collection longer than `limit`.
pub(super) fn check_len(
    path: &Path<'_>,
    found: usize,
    limit: usize,
) -> Result<(), SubmissionError> {
    if found > limit {
        return Err(too_many(path, found, limit));
    }
    Ok(())
}

/// Applies the value rules: profiles-only dictionary references, the OTLP
/// int64 range for unsigned input, nesting depth and size limits.
#[derive(Clone, Copy)]
pub(super) struct Rules {
    /// Whether dictionary indices are permitted (profiles payloads only).
    pub(super) profiles: bool,
}
impl Rules {
    fn string(path: &Path<'_>, bytes: usize) -> Result<(), SubmissionError> {
        if bytes > constants::SUBMISSION_MAX_STRING_BYTES {
            return Err(SubmissionError::validation(
                path.render(),
                format!(
                    "{bytes} bytes exceed the limit of {}",
                    constants::SUBMISSION_MAX_STRING_BYTES
                ),
            ));
        }
        Ok(())
    }
    pub(super) fn key_values(
        self,
        values: &KeyValues,
        path: &Path<'_>,
        depth: usize,
    ) -> Result<(), SubmissionError> {
        check_len(
            path,
            values.entries().len(),
            constants::SUBMISSION_MAX_COLLECTION_ENTRIES,
        )?;
        for (i, (key, value)) in values.entries().iter().enumerate() {
            let entry = path.index(i);
            match key {
                AttributeKey::Name(name) => Self::string(&entry.field("key"), name.len())?,
                AttributeKey::Index(_) if !self.profiles => {
                    return Err(SubmissionError::validation(
                        entry.field("key").render(),
                        "indexed attribute keys are profiles-only",
                    ));
                }
                AttributeKey::Index(_) => {}
            }
            self.any_value(value, &entry.field("value"), depth)?;
        }
        Ok(())
    }
    pub(super) fn any_value(
        self,
        value: &AnyValue,
        path: &Path<'_>,
        depth: usize,
    ) -> Result<(), SubmissionError> {
        if depth > constants::ANY_VALUE_MAX_DEPTH {
            return Err(SubmissionError::validation(
                path.render(),
                format!(
                    "value nesting exceeds {} levels",
                    constants::ANY_VALUE_MAX_DEPTH
                ),
            ));
        }
        match value {
            AnyValue::Bool(_) | AnyValue::Int(_) | AnyValue::Double(_) => {}
            AnyValue::String(text) => Self::string(path, text.len())?,
            AnyValue::Bytes(bytes) => Self::string(path, bytes.len())?,
            AnyValue::UInt(unsigned) => {
                if i64::try_from(*unsigned).is_err() {
                    return Err(SubmissionError::ValueOutOfRange {
                        path: path.render(),
                        context: context(
                            error_codes::SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE,
                            format!("unsigned value {unsigned} exceeds OTLP int64"),
                        ),
                    });
                }
            }
            AnyValue::StringIndex(_) => {
                if !self.profiles {
                    return Err(SubmissionError::validation(
                        path.render(),
                        "dictionary indices are profiles-only",
                    ));
                }
            }
            AnyValue::Array(values) => {
                check_len(
                    path,
                    values.len(),
                    constants::SUBMISSION_MAX_COLLECTION_ENTRIES,
                )?;
                for (i, nested) in values.iter().enumerate() {
                    self.any_value(nested, &path.index(i), depth + 1)?;
                }
            }
            AnyValue::KvList(values) => self.key_values(values, path, depth + 1)?,
        }
        Ok(())
    }
    fn resource(self, resource: &Resource, path: &Path<'_>) -> Result<(), SubmissionError> {
        let Resource {
            attributes,
            dropped_attributes_count: _,
            entity_refs: _,
            schema_url: _,
        } = resource;
        self.key_values(attributes, &path.field("attributes"), 1)
    }
    fn scope(self, scope: &InstrumentationScope, path: &Path<'_>) -> Result<(), SubmissionError> {
        let InstrumentationScope {
            name: _,
            version: _,
            attributes,
            dropped_attributes_count: _,
            schema_url: _,
        } = scope;
        self.key_values(attributes, &path.field("attributes"), 1)
    }
    /// Checks a family's records, each with its resource and scope.
    pub(super) fn records<T>(
        self,
        records: &[ResourceRecord<T>],
        path: &Path<'_>,
        record: impl Fn(Self, &T, &Path<'_>) -> Result<(), SubmissionError>,
    ) -> Result<(), SubmissionError> {
        check_len(
            path,
            records.len(),
            constants::SUBMISSION_MAX_RECORDS_PER_SIGNAL,
        )?;
        for (i, item) in records.iter().enumerate() {
            let at = path.index(i);
            self.resource(&item.resource, &at.field("resource"))?;
            self.scope(&item.scope, &at.field("scope"))?;
            record(self, &item.record, &at.field("record"))?;
        }
        Ok(())
    }
    pub(super) fn log(self, log: &LogPoint, path: &Path<'_>) -> Result<(), SubmissionError> {
        let LogPoint {
            time: _,
            observed_time: _,
            severity_number: _,
            severity_text: _,
            event_name: _,
            body,
            attributes,
            dropped_attributes_count: _,
            flags: _,
            trace_id: _,
            span_id: _,
        } = log;
        if let Some(body) = body {
            self.any_value(body, &path.field("body"), 1)?;
        }
        self.key_values(attributes, &path.field("attributes"), 1)
    }
    pub(super) fn span(self, span: &SpanPoint, path: &Path<'_>) -> Result<(), SubmissionError> {
        let SpanPoint {
            trace_id: _,
            span_id: _,
            trace_state: _,
            parent_span_id: _,
            flags: _,
            name: _,
            kind: _,
            start_time: _,
            end_time: _,
            attributes,
            dropped_attributes_count: _,
            events,
            dropped_events_count: _,
            links,
            dropped_links_count: _,
            status: _,
        } = span;
        self.key_values(attributes, &path.field("attributes"), 1)?;
        let events_path = path.field("events");
        check_len(
            &events_path,
            events.len(),
            constants::SUBMISSION_MAX_COLLECTION_ENTRIES,
        )?;
        for (i, event) in events.iter().enumerate() {
            let SpanEventPoint {
                time: _,
                name: _,
                attributes,
                dropped_attributes_count: _,
            } = event;
            self.key_values(attributes, &events_path.index(i).field("attributes"), 1)?;
        }
        let links_path = path.field("links");
        check_len(
            &links_path,
            links.len(),
            constants::SUBMISSION_MAX_COLLECTION_ENTRIES,
        )?;
        for (i, link) in links.iter().enumerate() {
            let SpanLinkPoint {
                trace_id: _,
                span_id: _,
                trace_state: _,
                attributes,
                dropped_attributes_count: _,
                flags: _,
            } = link;
            self.key_values(attributes, &links_path.index(i).field("attributes"), 1)?;
        }
        Ok(())
    }
    fn exemplars(self, exemplars: &[Exemplar], path: &Path<'_>) -> Result<(), SubmissionError> {
        check_len(
            path,
            exemplars.len(),
            constants::SUBMISSION_MAX_COLLECTION_ENTRIES,
        )?;
        for (i, exemplar) in exemplars.iter().enumerate() {
            let Exemplar {
                filtered_attributes,
                time: _,
                value: _,
                trace_id: _,
                span_id: _,
            } = exemplar;
            self.key_values(
                filtered_attributes,
                &path.index(i).field("filtered_attributes"),
                1,
            )?;
        }
        Ok(())
    }
    fn number_points(self, points: &[NumberPoint], path: &Path<'_>) -> Result<(), SubmissionError> {
        for (i, point) in points.iter().enumerate() {
            let NumberPoint {
                attributes,
                start_time: _,
                time: _,
                value: _,
                exemplars,
                flags: _,
            } = point;
            let at = path.index(i);
            self.key_values(attributes, &at.field("attributes"), 1)?;
            self.exemplars(exemplars, &at.field("exemplars"))?;
        }
        Ok(())
    }
    pub(super) fn metric(
        self,
        metric: &MetricStream,
        path: &Path<'_>,
    ) -> Result<(), SubmissionError> {
        let MetricStream {
            name: _,
            description: _,
            unit: _,
            metadata,
            data,
        } = metric;
        self.key_values(metadata, &path.field("metadata"), 1)?;
        let data_path = path.field("data");
        let points = data_path.field("points");
        let count = match data {
            MetricData::Gauge { points: p } | MetricData::Sum { points: p, .. } => p.len(),
            MetricData::Histogram { points: p, .. } => p.len(),
            MetricData::ExponentialHistogram { points: p, .. } => p.len(),
            MetricData::Summary { points: p } => p.len(),
        };
        check_len(&points, count, constants::SUBMISSION_MAX_RECORDS_PER_SIGNAL)?;
        match data {
            MetricData::Gauge { points: p } | MetricData::Sum { points: p, .. } => {
                self.number_points(p, &points)
            }
            MetricData::Histogram { points: p, .. } => {
                for (i, point) in p.iter().enumerate() {
                    let HistogramDataPoint {
                        attributes,
                        exemplars,
                        start_time: _,
                        time: _,
                        count: _,
                        sum: _,
                        bucket_counts: _,
                        explicit_bounds: _,
                        flags: _,
                        min: _,
                        max: _,
                    } = point;
                    let at = points.index(i);
                    self.key_values(attributes, &at.field("attributes"), 1)?;
                    self.exemplars(exemplars, &at.field("exemplars"))?;
                }
                Ok(())
            }
            MetricData::ExponentialHistogram { points: p, .. } => {
                for (i, point) in p.iter().enumerate() {
                    let ExponentialHistogramDataPoint {
                        attributes,
                        exemplars,
                        start_time: _,
                        time: _,
                        count: _,
                        sum: _,
                        scale: _,
                        zero_count: _,
                        zero_threshold: _,
                        positive: _,
                        negative: _,
                        flags: _,
                        min: _,
                        max: _,
                    } = point;
                    let at = points.index(i);
                    self.key_values(attributes, &at.field("attributes"), 1)?;
                    self.exemplars(exemplars, &at.field("exemplars"))?;
                }
                Ok(())
            }
            MetricData::Summary { points: p } => {
                for (i, point) in p.iter().enumerate() {
                    let SummaryDataPoint {
                        attributes,
                        start_time: _,
                        time: _,
                        count: _,
                        sum: _,
                        quantile_values: _,
                        flags: _,
                    } = point;
                    self.key_values(attributes, &points.index(i).field("attributes"), 1)?;
                }
                Ok(())
            }
        }
    }
    /// Checks profile metadata and the dictionary's attribute values.
    pub(super) fn profiles(
        self,
        profiles: &ProfilesSubmission,
        path: &Path<'_>,
    ) -> Result<(), SubmissionError> {
        let ProfilesSubmission {
            dictionary,
            profiles,
        } = profiles;
        let dictionary_path = path.field("dictionary");
        let table = dictionary_path.field("attribute_table");
        check_len(
            &table,
            dictionary.attribute_table.len(),
            constants::SUBMISSION_MAX_RECORDS_PER_SIGNAL,
        )?;
        for (i, entry) in dictionary.attribute_table.iter().enumerate() {
            let KeyValueAndUnit {
                key_strindex: _,
                value,
                unit_strindex: _,
            } = entry;
            if let Some(value) = value {
                self.any_value(value, &table.index(i).field("value"), 1)?;
            }
        }
        // Profile bodies hold only dictionary indices, which
        // `validate_references` checks against the tables.
        self.records(profiles, &path.field("profiles"), |_, _, _| Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::{Path, Rules};
    use crate::otlp::signals::{AnyValue, AttributeKey, KeyValues, StringIndex};
    use crate::{constants, error_codes};

    fn nested(depth: usize) -> AnyValue {
        (0..depth).fold(AnyValue::Int(1), |inner, _| AnyValue::Array(vec![inner]))
    }

    #[test]
    fn paths_render_fields_and_indices() {
        let root = Path::root("logs");
        let entry = root.index(2);
        let record = entry.field("record");
        assert_eq!(record.field("body").render(), "logs[2].record.body");
    }

    #[test]
    fn depth_limit_is_inclusive() {
        let rules = Rules { profiles: false };
        let root = Path::root("body");
        assert!(
            rules
                .any_value(&nested(constants::ANY_VALUE_MAX_DEPTH - 1), &root, 1)
                .is_ok()
        );
        let error = rules
            .any_value(&nested(constants::ANY_VALUE_MAX_DEPTH), &root, 1)
            .unwrap_err();
        assert_eq!(
            error.code(),
            &error_codes::SC_OBSERVABILITY_SUBMIT_VALIDATION
        );
    }

    #[test]
    fn collection_and_string_limits_reject_oversized_values() {
        let rules = Rules { profiles: false };
        let root = Path::root("body");
        let long = AnyValue::String("x".repeat(constants::SUBMISSION_MAX_STRING_BYTES + 1));
        assert!(rules.any_value(&long, &root, 1).is_err());
        let wide = AnyValue::Array(vec![
            AnyValue::Bool(true);
            constants::SUBMISSION_MAX_COLLECTION_ENTRIES + 1
        ]);
        assert!(rules.any_value(&wide, &root, 1).is_err());
        let attributes = KeyValues::try_from_iter(
            (0..=constants::SUBMISSION_MAX_COLLECTION_ENTRIES)
                .map(|i| (AttributeKey::Name(i.to_string()), AnyValue::Int(0))),
        )
        .unwrap();
        assert!(rules.key_values(&attributes, &root, 1).is_err());
    }

    #[test]
    fn typed_rules_report_exact_paths() {
        let rules = Rules { profiles: false };
        let root = Path::root("attributes");
        let values = KeyValues::try_from_iter([
            (AttributeKey::Name("ok".into()), AnyValue::Int(1)),
            (
                AttributeKey::Name("big".into()),
                AnyValue::Array(vec![AnyValue::UInt(u64::MAX)]),
            ),
        ])
        .unwrap();
        let error = rules.key_values(&values, &root, 1).unwrap_err();
        assert_eq!(
            error.code(),
            &error_codes::SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE
        );
        assert!(matches!(
            error,
            crate::otlp::submission::SubmissionError::ValueOutOfRange { ref path, .. }
                if path == "attributes[1].value[0]"
        ));
        let indexed = KeyValues::try_from_iter([(
            AttributeKey::Index(StringIndex::try_new(0).unwrap()),
            AnyValue::StringIndex(StringIndex::try_new(0).unwrap()),
        )])
        .unwrap();
        assert!(matches!(
            rules.key_values(&indexed, &root, 1).unwrap_err(),
            crate::otlp::submission::SubmissionError::Validation { ref path, .. }
                if path == "attributes[0].key"
        ));
        assert!(
            Rules { profiles: true }
                .key_values(&indexed, &root, 1)
                .is_ok()
        );
    }
}
