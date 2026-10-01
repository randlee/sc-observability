//! Shared input validation, correlation and deterministic serialization.
use super::{
    EnvelopeVersion, ProfilesSubmission, Signal, SignalSet, SubmissionEnvelope, SubmissionError,
    SubmissionInput, error_codes, errors::context,
};
use crate::otlp::signals::{self, ResourceRecord, SignalValidationError};
use crate::{SpanId, Timestamp, TraceId};
use std::{
    collections::{BTreeMap, hash_map::RandomState},
    hash::{BuildHasher, Hasher},
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};

/// Injectable identifiers and observation time for canonicalization.
pub trait IdSource {
    /// Generates a nonzero trace identifier.
    fn trace_id(&mut self) -> TraceId;
    /// Generates a nonzero span identifier.
    fn span_id(&mut self) -> SpanId;
    /// Returns current UTC time.
    fn now(&mut self) -> Timestamp;
}
/// Process-local collision-resistant identifiers and the real UTC clock.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct SystemIds;
impl SystemIds {
    /// Creates an identifier source backed by the process random seed and UTC clock.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static ID_HASH: LazyLock<RandomState> = LazyLock::new(RandomState::new);
fn random_part() -> u64 {
    let mut h = ID_HASH.build_hasher();
    h.write_u64(ID_COUNTER.fetch_add(1, Ordering::Relaxed));
    h.write_u32(std::process::id());
    h.finish().max(1)
}
impl IdSource for SystemIds {
    fn trace_id(&mut self) -> TraceId {
        TraceId::new(format!("{:016x}{:016x}", random_part(), random_part()))
            .expect("formatted nonzero hexadecimal trace identifier")
    }
    fn span_id(&mut self) -> SpanId {
        SpanId::new(format!("{:016x}", random_part()))
            .expect("formatted nonzero hexadecimal span identifier")
    }
    fn now(&mut self) -> Timestamp {
        Timestamp::now_utc()
    }
}
fn signal_error(error: SignalValidationError) -> SubmissionError {
    let SignalValidationError::Validation {
        path,
        context: source,
    } = error;
    SubmissionError::validation(path, source.to_string())
}
fn conflict() -> SubmissionError {
    SubmissionError::CorrelationConflict {
        context: context(
            error_codes::SC_OBSERVABILITY_SUBMIT_CORRELATION_CONFLICT,
            "correlated records provide inconsistent identifiers",
        ),
    }
}
fn timing() -> SubmissionError {
    SubmissionError::TimingConflict {
        context: context(
            error_codes::SC_OBSERVABILITY_SUBMIT_TIMING_CONFLICT,
            "span end and duration disagree",
        ),
    }
}
type Correlation = (Option<TraceId>, Option<SpanId>);
fn merge(
    target: &mut Correlation,
    trace: Option<&TraceId>,
    span: Option<&SpanId>,
) -> Result<(), SubmissionError> {
    if trace.zip(target.0.as_ref()).is_some_and(|(a, b)| a != b)
        || span.zip(target.1.as_ref()).is_some_and(|(a, b)| a != b)
    {
        return Err(conflict());
    }
    if trace.is_some() {
        target.0 = trace.cloned();
    }
    if span.is_some() {
        target.1 = span.cloned();
    }
    Ok(())
}
fn canonical_span(
    i: usize,
    span: super::SpanInput,
    correlations: &BTreeMap<String, Correlation>,
    ids: &mut dyn IdSource,
    resource: &signals::Resource,
    scope: &signals::InstrumentationScope,
) -> Result<ResourceRecord<signals::SpanPoint>, SubmissionError> {
    let mut pair = (span.trace_id, span.span_id);
    if let Some(key) = &span.correlation_id
        && let Some(shared) = correlations.get(key)
    {
        pair = shared.clone();
    }
    let trace = pair.0.unwrap_or_else(|| ids.trace_id());
    let span_id = pair.1.unwrap_or_else(|| ids.span_id());
    let duration_end = span
        .duration_nanos
        .map(|n| {
            let duration = time::Duration::seconds(
                i64::try_from(n / 1_000_000_000).expect("u64 nanoseconds fit i64 seconds"),
            ) + time::Duration::nanoseconds(
                i64::try_from(n % 1_000_000_000).expect("subsecond nanoseconds fit i64"),
            );
            span.start_time
                .into_inner()
                .checked_add(duration)
                .map(Timestamp::from)
                .ok_or_else(|| {
                    SubmissionError::validation(
                        format!("spans[{i}].duration_nanos"),
                        "timestamp overflow",
                    )
                })
        })
        .transpose()?;
    let end = match (span.end_time, duration_end) {
        (Some(a), Some(b)) if a != b => return Err(timing()),
        (Some(a), _) | (_, Some(a)) => a,
        _ => {
            return Err(SubmissionError::validation(
                format!("spans[{i}].end_time"),
                "actual end or duration required",
            ));
        }
    };
    let record = signals::SpanPoint::try_new(
        trace,
        span_id,
        span.trace_state
            .map(signals::TraceState::try_new)
            .transpose()
            .map_err(signal_error)?,
        span.parent_span_id,
        span.flags,
        span.name,
        span.kind.unwrap_or_default(),
        span.start_time,
        end,
        span.attributes,
        span.dropped_attributes_count,
        span.events,
        span.dropped_events_count,
        span.links,
        span.dropped_links_count,
        span.status.unwrap_or_default(),
    )
    .map_err(signal_error)?;
    Ok(ResourceRecord::new(
        span.resource.unwrap_or_else(|| resource.clone()),
        span.scope.unwrap_or_else(|| scope.clone()),
        record,
    ))
}
fn check_version(version: EnvelopeVersion) -> Result<(), SubmissionError> {
    if version != EnvelopeVersion::CURRENT {
        return Err(SubmissionError::UnsupportedVersion {
            found: version,
            context: context(
                error_codes::SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION,
                "unsupported envelope version",
            ),
        });
    }
    Ok(())
}
impl SubmissionEnvelope {
    /// Canonicalizes all four signal families without front-end-specific behavior.
    /// # Errors
    /// Rejects unsupported versions, invalid values/timing, conflicting correlation, and invalid profile references.
    pub fn from_input(
        input: SubmissionInput,
        ids: &mut dyn IdSource,
    ) -> Result<Self, SubmissionError> {
        check_version(input.version)?;
        let resource = input.resource.unwrap_or_default();
        let scope = input.scope.unwrap_or_default();
        let mut correlations = BTreeMap::<String, Correlation>::new();
        for span in &input.spans {
            if let Some(key) = &span.correlation_id {
                merge(
                    correlations.entry(key.clone()).or_default(),
                    span.trace_id.as_ref(),
                    span.span_id.as_ref(),
                )?;
            }
        }
        for log in &input.logs {
            if let Some(key) = &log.correlation_id
                && let Some(pair) = correlations.get_mut(key)
            {
                merge(pair, log.trace_id.as_ref(), log.span_id.as_ref())?;
            }
        }
        for pair in correlations.values_mut() {
            pair.0.get_or_insert_with(|| ids.trace_id());
            pair.1.get_or_insert_with(|| ids.span_id());
        }
        let mut spans = Vec::new();
        for (i, span) in input.spans.into_iter().enumerate() {
            spans.push(canonical_span(
                i,
                span,
                &correlations,
                ids,
                &resource,
                &scope,
            )?);
        }
        let mut logs = Vec::new();
        for log in input.logs {
            let pair = log
                .correlation_id
                .as_ref()
                .and_then(|k| correlations.get(k))
                .cloned()
                .unwrap_or((log.trace_id, log.span_id));
            let record = signals::LogPoint::new(
                log.time,
                log.observed_time.unwrap_or_else(|| ids.now()),
                log.severity_number.unwrap_or_default(),
                log.severity_text,
                log.event_name,
                log.body,
                log.attributes,
                log.dropped_attributes_count,
                log.flags,
                pair.0,
                pair.1,
            );
            logs.push(ResourceRecord::new(
                log.resource.unwrap_or_else(|| resource.clone()),
                log.scope.unwrap_or_else(|| scope.clone()),
                record,
            ));
        }
        let metrics = input
            .metrics
            .into_iter()
            .map(|m| {
                ResourceRecord::new(
                    m.resource.unwrap_or_else(|| resource.clone()),
                    m.scope.unwrap_or_else(|| scope.clone()),
                    m.stream,
                )
            })
            .collect();
        let profiles = input.profiles.map(|p| {
            let r = p.resource.unwrap_or_else(|| resource.clone());
            let s = p.scope.unwrap_or_else(|| scope.clone());
            ProfilesSubmission {
                dictionary: p.dictionary,
                profiles: p
                    .profiles
                    .into_iter()
                    .map(|p| ResourceRecord::new(r.clone(), s.clone(), p))
                    .collect(),
            }
        });
        let envelope = Self {
            version: input.version,
            record_key: input.record_key,
            logs,
            spans,
            metrics,
            profiles,
        };
        envelope.validate()?;
        Ok(envelope)
    }
    /// Parses caller JSON and uses the shared canonicalization path.
    /// # Errors
    /// Syntax/EOF errors return `InvalidJson`; data and model errors return Validation with line/column.
    pub fn from_json(json: &str, ids: &mut dyn IdSource) -> Result<Self, SubmissionError> {
        let input = serde_json::from_str(json).map_err(|e| {
            if e.is_syntax() || e.is_eof() {
                SubmissionError::InvalidJson {
                    context: context(
                        error_codes::SC_OBSERVABILITY_SUBMIT_INVALID_JSON,
                        e.to_string(),
                    ),
                }
            } else {
                SubmissionError::validation(format!("{}:{}", e.line(), e.column()), e.to_string())
            }
        })?;
        Self::from_input(input, ids)
    }
    /// Revalidates a canonical envelope before admission, including mutated public fields.
    /// # Errors
    /// Rejects unsupported/empty data, out-of-range values, misplaced indices and invalid records.
    pub fn validate(&self) -> Result<(), SubmissionError> {
        if self.version != EnvelopeVersion::CURRENT {
            return Err(SubmissionError::UnsupportedVersion {
                found: self.version,
                context: context(
                    error_codes::SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION,
                    "unsupported envelope version",
                ),
            });
        }
        if self.signals().iter().next().is_none() {
            return Err(SubmissionError::EmptySubmission {
                context: context(
                    error_codes::SC_OBSERVABILITY_SUBMIT_EMPTY,
                    "at least one signal is required",
                ),
            });
        }
        // Every AnyValue/AttributeKey is visited through the typed neutral serde
        // shape, including metric exemplars and resource/scope metadata.
        for (family, records) in [
            ("logs", serde_json::to_value(&self.logs)),
            ("spans", serde_json::to_value(&self.spans)),
            ("metrics", serde_json::to_value(&self.metrics)),
        ] {
            validate_tree(
                &records.map_err(|e| SubmissionError::validation(family, e.to_string()))?,
                family,
                false,
            )?;
        }
        for span in &self.spans {
            span.record.validate().map_err(signal_error)?;
        }
        for metric in &self.metrics {
            metric.record.validate().map_err(signal_error)?;
        }
        if let Some(profiles) = &self.profiles {
            validate_tree(
                &serde_json::to_value(profiles)
                    .map_err(|e| SubmissionError::validation("profiles", e.to_string()))?,
                "profiles",
                true,
            )?;
            profiles
                .dictionary
                .validate_references(&profiles.profiles)
                .map_err(|e| {
                    let SignalValidationError::Validation {
                        path,
                        context: source,
                    } = e;
                    SubmissionError::DictionaryReference {
                        path,
                        context: context(
                            error_codes::SC_OBSERVABILITY_SUBMIT_DICTIONARY_REFERENCE,
                            source.to_string(),
                        ),
                    }
                })?;
        }
        Ok(())
    }
    /// Serializes canonical ordered keys with UTC timestamps.
    /// # Panics
    /// Panics if a neutral record violates its total JSON serialization invariant.
    #[must_use]
    pub fn to_canonical_json(&self) -> String {
        let value =
            serde_json::to_value(self).expect("neutral values have total JSON serialization");
        serde_json::to_string(&value).expect("a JSON value is serializable")
    }
    /// Returns the families that contain records.
    #[must_use]
    pub fn signals(&self) -> SignalSet {
        SignalSet::new(
            [
                (Signal::Logs, !self.logs.is_empty()),
                (Signal::Traces, !self.spans.is_empty()),
                (Signal::Metrics, !self.metrics.is_empty()),
                (
                    Signal::Profiles,
                    self.profiles
                        .as_ref()
                        .is_some_and(|p| !p.profiles.is_empty()),
                ),
            ]
            .into_iter()
            .filter_map(|(s, has)| has.then_some(s)),
        )
    }
}
fn validate_tree(
    value: &serde_json::Value,
    path: &str,
    profiles: bool,
) -> Result<(), SubmissionError> {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            if map.get("kind").and_then(Value::as_str) == Some("uint")
                && map
                    .get("data")
                    .and_then(Value::as_u64)
                    .is_some_and(|v| v > i64::MAX as u64)
            {
                return Err(SubmissionError::ValueOutOfRange {
                    path: path.into(),
                    context: context(
                        error_codes::SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE,
                        "unsigned value exceeds OTLP int64",
                    ),
                });
            }
            if !profiles && map.get("kind").and_then(Value::as_str) == Some("string_index") {
                return Err(SubmissionError::validation(
                    path,
                    "dictionary indices are profiles-only",
                ));
            }
            for (key, v) in map {
                validate_tree(v, &format!("{path}.{key}"), profiles)?;
            }
        }
        Value::Array(values) => {
            if !profiles && values.len() == 2 && values[0].is_number() && values[1].is_object() {
                return Err(SubmissionError::validation(
                    path,
                    "indexed attribute keys are profiles-only",
                ));
            }
            for (i, v) in values.iter().enumerate() {
                validate_tree(v, &format!("{path}[{i}]"), profiles)?;
            }
        }
        _ => {}
    }
    Ok(())
}
