//! Shared input validation, correlation and deterministic serialization.
use super::{
    EnvelopeVersion, ProfilesSubmission, Signal, SignalSet, SubmissionEnvelope, SubmissionError,
    SubmissionInput, error_codes,
    errors::{context, context_with_source},
    structure::{Path, Rules, check_len},
};
use crate::otlp::signals::{self, ResourceRecord, SignalValidationError};
use crate::{SpanId, Timestamp, TraceId, constants};
use std::{
    collections::{BTreeMap, hash_map::RandomState},
    hash::{BuildHasher, Hasher},
    num::{NonZeroU64, NonZeroU128},
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
fn random_part() -> NonZeroU64 {
    let mut h = ID_HASH.build_hasher();
    h.write_u64(ID_COUNTER.fetch_add(1, Ordering::Relaxed));
    h.write_u32(std::process::id());
    NonZeroU64::new(h.finish()).unwrap_or(NonZeroU64::MIN)
}
impl IdSource for SystemIds {
    fn trace_id(&mut self) -> TraceId {
        let high = u128::from(random_part().get()) << u64::BITS;
        TraceId::from_nonzero(NonZeroU128::from(random_part()) | high)
    }
    fn span_id(&mut self) -> SpanId {
        SpanId::from_nonzero(random_part())
    }
    fn now(&mut self) -> Timestamp {
        Timestamp::now_utc()
    }
}
/// Wraps a signal failure, keeping its path and its own diagnostic as source.
fn signal_error(error: SignalValidationError) -> SubmissionError {
    let SignalValidationError::Validation { path, .. } = &error;
    let path = path.clone();
    SubmissionError::Validation {
        context: context_with_source(
            error_codes::SC_OBSERVABILITY_SUBMIT_VALIDATION,
            format!("signal field {path} failed validation"),
            error,
        ),
        path,
    }
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
            let overflow = || {
                SubmissionError::validation(
                    format!("spans[{i}].duration_nanos"),
                    format!("start_time plus {n} ns overflows the timestamp range"),
                )
            };
            let duration = time::Duration::try_from(std::time::Duration::from_nanos(n))
                .map_err(|_| overflow())?;
            span.start_time
                .into_inner()
                .checked_add(duration)
                .map(Timestamp::from)
                .ok_or_else(overflow)
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
/// The single envelope version rule shared by canonicalization and revalidation.
fn check_version(version: EnvelopeVersion) -> Result<(), SubmissionError> {
    if version != EnvelopeVersion::CURRENT {
        return Err(SubmissionError::UnsupportedVersion {
            found: version,
            context: context(
                error_codes::SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION,
                format!(
                    "envelope version {} is unsupported; expected {}",
                    version.get(),
                    EnvelopeVersion::CURRENT.get()
                ),
            ),
        });
    }
    Ok(())
}
/// Rejects oversized families before any canonicalization work.
fn check_input_counts(input: &SubmissionInput) -> Result<(), SubmissionError> {
    let limit = constants::SUBMISSION_MAX_RECORDS_PER_SIGNAL;
    check_len(&Path::root("logs"), input.logs.len(), limit)?;
    check_len(&Path::root("spans"), input.spans.len(), limit)?;
    check_len(&Path::root("metrics"), input.metrics.len(), limit)?;
    if let Some(profiles) = &input.profiles {
        check_len(&Path::root("profiles"), profiles.profiles.len(), limit)?;
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
        check_input_counts(&input)?;
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
        // The version was checked on entry; only the content rules remain.
        envelope.validate_contents()?;
        Ok(envelope)
    }
    /// Parses caller JSON and uses the shared canonicalization path.
    /// # Errors
    /// Oversized input returns Validation; syntax/EOF errors return `InvalidJson`;
    /// data and model errors return Validation with line/column.
    pub fn from_json(json: &str, ids: &mut dyn IdSource) -> Result<Self, SubmissionError> {
        if json.len() > constants::SUBMISSION_MAX_INPUT_BYTES {
            return Err(SubmissionError::validation(
                "input",
                format!(
                    "{} bytes exceed the submission limit of {}",
                    json.len(),
                    constants::SUBMISSION_MAX_INPUT_BYTES
                ),
            ));
        }
        let input = serde_json::from_str(json).map_err(|e| {
            let at = format!("{}:{}", e.line(), e.column());
            if e.is_syntax() || e.is_eof() {
                SubmissionError::InvalidJson {
                    context: context_with_source(
                        error_codes::SC_OBSERVABILITY_SUBMIT_INVALID_JSON,
                        format!("malformed JSON at line:column {at}"),
                        e,
                    ),
                }
            } else {
                let message = e.to_string();
                SubmissionError::Validation {
                    path: at,
                    context: context_with_source(
                        error_codes::SC_OBSERVABILITY_SUBMIT_VALIDATION,
                        message,
                        e,
                    ),
                }
            }
        })?;
        Self::from_input(input, ids)
    }
    /// Revalidates a canonical envelope before admission, including mutated public fields.
    /// # Errors
    /// Rejects unsupported/empty data, out-of-range values, misplaced indices,
    /// oversized collections and invalid records.
    pub fn validate(&self) -> Result<(), SubmissionError> {
        check_version(self.version)?;
        self.validate_contents()
    }
    /// Every rule except the version, which callers check first.
    fn validate_contents(&self) -> Result<(), SubmissionError> {
        if self.signals().iter().next().is_none() {
            return Err(SubmissionError::EmptySubmission {
                context: context(
                    error_codes::SC_OBSERVABILITY_SUBMIT_EMPTY,
                    "at least one signal is required",
                ),
            });
        }
        let rules = Rules { profiles: false };
        rules.records(&self.logs, &Path::root("logs"), Rules::log)?;
        rules.records(&self.spans, &Path::root("spans"), Rules::span)?;
        rules.records(&self.metrics, &Path::root("metrics"), Rules::metric)?;
        for span in &self.spans {
            span.record.validate().map_err(signal_error)?;
        }
        for metric in &self.metrics {
            metric.record.validate().map_err(signal_error)?;
        }
        if let Some(profiles) = &self.profiles {
            Rules { profiles: true }.profiles(profiles, &Path::root("profiles"))?;
            profiles
                .dictionary
                .validate_references(&profiles.profiles)
                .map_err(|error| {
                    let SignalValidationError::Validation { path, .. } = &error;
                    let path = path.clone();
                    SubmissionError::DictionaryReference {
                        context: context_with_source(
                            error_codes::SC_OBSERVABILITY_SUBMIT_DICTIONARY_REFERENCE,
                            format!("profiles {path} does not address a dictionary entry"),
                            error,
                        ),
                        path,
                    }
                })?;
        }
        Ok(())
    }
    /// Serializes canonical ordered keys with UTC timestamps.
    ///
    /// Neutral serialization is total, the policy `validate` shares: every map
    /// key is a string, timestamps render for any year, non-finite doubles use
    /// their proto-JSON spellings and every custom serializer is infallible.
    /// Should a future type break that invariant, the result is the JSON
    /// `null`, which deserialization and `from_json` reject with a typed error;
    /// this method never panics.
    #[must_use]
    pub fn to_canonical_json(&self) -> String {
        // `Value` objects sort their keys, which makes the output canonical.
        let null = || String::from("null");
        serde_json::to_value(self)
            .ok()
            .and_then(|value| serde_json::to_string(&value).ok())
            .unwrap_or_else(null)
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

#[cfg(test)]
mod tests {
    use super::{IdSource, SystemIds};
    use crate::otlp::submission::{
        LogInput, SpanInput, SubmissionEnvelope, SubmissionError, SubmissionInput,
    };
    use crate::{SpanId, Timestamp, TraceId, constants, error_codes};

    #[test]
    fn system_ids_are_always_valid_without_panicking() {
        let mut ids = SystemIds::new();
        for _ in 0..256 {
            let trace = ids.trace_id();
            assert_eq!(TraceId::new(trace.as_str()).as_ref(), Ok(&trace));
            let span = ids.span_id();
            assert_eq!(SpanId::new(span.as_str()).as_ref(), Ok(&span));
        }
    }

    #[test]
    fn duration_overflow_is_a_typed_error() {
        let latest = time::Date::from_calendar_date(9999, time::Month::December, 31)
            .map(|date| date.midnight().assume_utc())
            .map(Timestamp::from)
            .unwrap();
        let mut span = SpanInput::new("late".into(), latest);
        span.duration_nanos = Some(u64::MAX);
        let mut input = SubmissionInput::new();
        input.spans.push(span);
        match SubmissionEnvelope::from_input(input, &mut SystemIds::new()) {
            Err(SubmissionError::Validation { path, .. }) => {
                assert_eq!(path, "spans[0].duration_nanos");
            }
            other => panic!("expected duration overflow, got {other:?}"),
        }
        let mut span = SpanInput::new("long".into(), Timestamp::UNIX_EPOCH);
        span.duration_nanos = Some(u64::MAX);
        let mut input = SubmissionInput::new();
        input.spans.push(span);
        let envelope = SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap();
        assert_eq!(
            envelope.spans[0]
                .record
                .end_time
                .into_inner()
                .unix_timestamp_nanos(),
            i128::from(u64::MAX)
        );
    }

    #[test]
    fn canonical_json_is_total_for_extreme_values() {
        let json = r#"{"version":1,"logs":[{"time":"9999-12-31T23:59:59.999999999Z","body":{"kind":"double","data":"NaN"},"attributes":{"inf":{"kind":"double","data":"-Infinity"},"bytes":{"kind":"bytes","data":"00ff"}}}]}"#;
        let envelope = SubmissionEnvelope::from_json(json, &mut SystemIds::new()).unwrap();
        let canonical = envelope.to_canonical_json();
        assert_eq!(
            serde_json::from_str::<SubmissionEnvelope>(&canonical).unwrap(),
            envelope
        );
    }

    #[test]
    fn oversized_input_is_rejected_before_parsing() {
        let json = " ".repeat(constants::SUBMISSION_MAX_INPUT_BYTES + 1);
        match SubmissionEnvelope::from_json(&json, &mut SystemIds::new()) {
            Err(SubmissionError::Validation { path, .. }) => assert_eq!(path, "input"),
            other => panic!("expected input limit, got {other:?}"),
        }
    }

    #[test]
    fn record_count_limit_applies_to_input_and_envelope() {
        let mut input = SubmissionInput::new();
        input.logs = vec![LogInput::new(); constants::SUBMISSION_MAX_RECORDS_PER_SIGNAL + 1];
        match SubmissionEnvelope::from_input(input, &mut SystemIds::new()) {
            Err(SubmissionError::Validation { path, .. }) => assert_eq!(path, "logs"),
            other => panic!("expected record limit, got {other:?}"),
        }
        let mut input = SubmissionInput::new();
        input.logs.push(LogInput::new());
        let mut envelope = SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap();
        let record = envelope.logs[0].clone();
        envelope.logs = vec![record; constants::SUBMISSION_MAX_RECORDS_PER_SIGNAL + 1];
        assert!(matches!(
            envelope.validate(),
            Err(SubmissionError::Validation { ref path, .. }) if path == "logs"
        ));
    }

    #[test]
    fn deserialization_path_enforces_depth_limit() {
        let mut body = String::from("1");
        for _ in 0..=constants::ANY_VALUE_MAX_DEPTH {
            body = format!(r#"{{"kind":"array","data":[{body}]}}"#);
        }
        let json = format!(r#"{{"version":1,"logs":[{{"body":{body}}}]}}"#);
        let error = SubmissionEnvelope::from_json(&json, &mut SystemIds::new()).unwrap_err();
        assert_eq!(
            error.code(),
            &error_codes::SC_OBSERVABILITY_SUBMIT_VALIDATION
        );
        assert!(error.to_string().contains("nesting exceeds"), "{error}");
    }

    #[test]
    fn version_is_checked_once_and_first() {
        let json = r#"{"version":2,"spans":[{"name":"x","start_time":"1970-01-01T00:00:00Z","end_time":"1970-01-01T00:00:01Z","duration_nanos":2}]}"#;
        match SubmissionEnvelope::from_json(json, &mut SystemIds::new()) {
            Err(SubmissionError::UnsupportedVersion { found, .. }) => assert_eq!(found.get(), 2),
            other => panic!("expected unsupported version, got {other:?}"),
        }
    }

    #[test]
    fn signal_failures_keep_their_source_diagnostic() {
        let mut span = SpanInput::new("x".into(), Timestamp::UNIX_EPOCH);
        span.duration_nanos = Some(1);
        span.trace_state = Some("Invalid Key=v".into());
        let mut input = SubmissionInput::new();
        input.spans.push(span);
        let error = SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap_err();
        let SubmissionError::Validation { path, context } = &error else {
            panic!("expected validation, got {error:?}");
        };
        assert_eq!(path, "trace_state");
        let source = std::error::Error::source(context.as_ref()).expect("signal error source");
        assert!(source.to_string().contains("invalid tracestate member"));
    }
}
