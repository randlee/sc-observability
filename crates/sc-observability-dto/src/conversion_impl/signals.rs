//! Metric and span signal conversions.
use super::*;

fn model_failure(error: core::v2::MetricModelError) -> Failure {
    failure_from_classification(error.diagnostic(), error.failure_classification())
}
fn finite(value: f64, field: &str) -> Result<core::v2::FiniteF64, Failure> {
    checked(core::v2::FiniteF64::new(value), field)
}
fn to_attributes(value: BTreeMap<String, ValueDto>) -> Result<core::v2::Attributes, Failure> {
    value
        .into_iter()
        .map(|(key, value)| {
            let raw = to_value(value, "attributes", false, 0)?;
            Ok((key, checked(serde_json::from_value(raw), "attributes")?))
        })
        .collect()
}
fn from_attributes(value: &core::v2::Attributes) -> Result<BTreeMap<String, ValueDto>, Failure> {
    value
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                from_json_value(checked(serde_json::to_value(value), "attributes")?)?,
            ))
        })
        .collect()
}
impl From<AggregationTemporalityDto> for core::v2::AggregationTemporality {
    fn from(value: AggregationTemporalityDto) -> Self {
        match value {
            AggregationTemporalityDto::Delta => Self::Delta,
            AggregationTemporalityDto::Cumulative => Self::Cumulative,
        }
    }
}
impl TryFrom<core::v2::AggregationTemporality> for AggregationTemporalityDto {
    type Error = Failure;
    fn try_from(value: core::v2::AggregationTemporality) -> Result<Self, Self::Error> {
        match value {
            core::v2::AggregationTemporality::Delta => Ok(Self::Delta),
            core::v2::AggregationTemporality::Cumulative => Ok(Self::Cumulative),
            _ => Err(invalid_input(
                "temporality",
                "unknown aggregation temporality",
            )),
        }
    }
}
impl TryFrom<HistogramPointDto> for core::v2::HistogramPoint {
    type Error = Failure;
    fn try_from(value: HistogramPointDto) -> Result<Self, Self::Error> {
        Self::try_new(
            value
                .explicit_bounds
                .into_iter()
                .map(|v| finite(v, "explicit_bounds"))
                .collect::<Result<_, _>>()?,
            value
                .bucket_counts
                .into_iter()
                .map(|v| checked(v.as_u64(), "bucket_counts"))
                .collect::<Result<_, _>>()?,
            checked(value.count.as_u64(), "count")?,
            finite(value.sum, "sum")?,
        )
        .map_err(model_failure)
    }
}
impl From<&core::v2::HistogramPoint> for HistogramPointDto {
    fn from(value: &core::v2::HistogramPoint) -> Self {
        Self {
            explicit_bounds: value.explicit_bounds().iter().map(|v| v.get()).collect(),
            bucket_counts: value.bucket_counts().iter().map(|v| (*v).into()).collect(),
            count: value.count().into(),
            sum: value.sum().get(),
        }
    }
}
impl TryFrom<MetricRecordDto> for core::v2::MetricRecord {
    type Error = Failure;
    fn try_from(v: MetricRecordDto) -> Result<Self, Self::Error> {
        let value = match v.value {
            MetricValueDto::Gauge(value) => core::v2::MetricValue::Gauge(finite(value, "value")?),
            MetricValueDto::Sum {
                value,
                monotonic,
                temporality,
                start_time,
            } => core::v2::MetricValue::Sum {
                value: finite(value, "value")?,
                monotonic,
                temporality: temporality.into(),
                start_time: timestamp(start_time, "start_time")?,
            },
            MetricValueDto::Histogram {
                point,
                temporality,
                start_time,
            } => core::v2::MetricValue::Histogram {
                point: point.try_into()?,
                temporality: temporality.into(),
                start_time: timestamp(start_time, "start_time")?,
            },
        };
        let record = Self::try_new(
            timestamp(v.timestamp, "timestamp")?,
            checked(core::ServiceName::new(v.service), "service")?,
            checked(core::MetricName::new(v.name), "name")?,
            value,
        )
        .map_err(model_failure)?;
        Ok(record
            .with_unit(optional_checked(v.unit, "unit", core::MetricUnit::new)?)
            .with_attributes(to_attributes(v.attributes)?))
    }
}
impl TryFrom<&core::v2::MetricRecord> for MetricRecordDto {
    type Error = Failure;
    fn try_from(v: &core::v2::MetricRecord) -> Result<Self, Self::Error> {
        let value = match v.value() {
            core::v2::MetricValue::Gauge(value) => MetricValueDto::Gauge(value.get()),
            core::v2::MetricValue::Sum {
                value,
                monotonic,
                temporality,
                start_time,
            } => MetricValueDto::Sum {
                value: value.get(),
                monotonic: *monotonic,
                temporality: (*temporality).try_into()?,
                start_time: start_time.to_string(),
            },
            core::v2::MetricValue::Histogram {
                point,
                temporality,
                start_time,
            } => MetricValueDto::Histogram {
                point: point.into(),
                temporality: (*temporality).try_into()?,
                start_time: start_time.to_string(),
            },
            _ => return Err(invalid_input("metric", "unknown metric variant")),
        };
        Ok(Self {
            timestamp: v.timestamp().to_string(),
            service: v.service().as_str().into(),
            name: v.name().as_str().into(),
            value,
            unit: v.unit().map(|v| v.as_str().into()),
            attributes: from_attributes(v.attributes())?,
        })
    }
}
/// Decodes and validates a staged metric point, including histogram and temporal invariants.
///
/// # Errors
/// Returns a tagged validation failure for malformed data or invalid native invariants.
pub fn decode_metric(value: Value) -> Result<core::v2::MetricRecord, Failure> {
    let dto: MetricRecordDto = decode(value, "metric")?;
    dto.try_into()
}

impl TryFrom<TraceContextV2Dto> for core::v2::TraceContext {
    type Error = Failure;
    fn try_from(v: TraceContextV2Dto) -> Result<Self, Self::Error> {
        let mut trace = Self::new(
            checked(core::TraceId::new(v.trace_id), "trace_id")?,
            checked(core::SpanId::new(v.span_id), "span_id")?,
            core::v2::TraceFlags::new(v.flags),
        );
        if let Some(parent) = v.parent_span_id {
            trace = trace.with_parent(checked(core::SpanId::new(parent), "parent_span_id")?);
        }
        Ok(trace)
    }
}
impl From<&core::v2::TraceContext> for TraceContextV2Dto {
    fn from(v: &core::v2::TraceContext) -> Self {
        Self {
            trace_id: v.trace_id.as_str().into(),
            span_id: v.span_id.as_str().into(),
            parent_span_id: v.parent_span_id.as_ref().map(|v| v.as_str().into()),
            flags: v.flags.bits(),
        }
    }
}
impl TryFrom<SpanLinkDto> for core::v2::SpanLink {
    type Error = Failure;
    fn try_from(v: SpanLinkDto) -> Result<Self, Self::Error> {
        Ok(Self::new(
            checked(core::TraceId::new(v.trace_id), "trace_id")?,
            checked(core::SpanId::new(v.span_id), "span_id")?,
            core::v2::TraceFlags::new(v.flags),
            to_attributes(v.attributes)?,
        ))
    }
}
impl TryFrom<&core::v2::SpanLink> for SpanLinkDto {
    type Error = Failure;
    fn try_from(v: &core::v2::SpanLink) -> Result<Self, Self::Error> {
        Ok(Self {
            trace_id: v.trace_id.as_str().into(),
            span_id: v.span_id.as_str().into(),
            flags: v.flags.bits(),
            attributes: from_attributes(&v.attributes)?,
        })
    }
}
impl From<SpanKindDto> for core::v2::SpanKind {
    fn from(v: SpanKindDto) -> Self {
        match v {
            SpanKindDto::Internal => Self::Internal,
            SpanKindDto::Server => Self::Server,
            SpanKindDto::Client => Self::Client,
            SpanKindDto::Producer => Self::Producer,
            SpanKindDto::Consumer => Self::Consumer,
        }
    }
}
impl TryFrom<core::v2::SpanKind> for SpanKindDto {
    type Error = Failure;
    fn try_from(v: core::v2::SpanKind) -> Result<Self, Self::Error> {
        match v {
            core::v2::SpanKind::Internal => Ok(Self::Internal),
            core::v2::SpanKind::Server => Ok(Self::Server),
            core::v2::SpanKind::Client => Ok(Self::Client),
            core::v2::SpanKind::Producer => Ok(Self::Producer),
            core::v2::SpanKind::Consumer => Ok(Self::Consumer),
            _ => Err(invalid_input("kind", "unknown span kind")),
        }
    }
}
enum_map!(SpanStatusDto, SpanStatus, Ok, Error, Unset);
/// Preserves the canonical diagnostic payload in the retained stored-diagnostic shape.
fn stored_diagnostic_dto(v: CanonicalDiagnosticDto) -> StoredDiagnosticDto {
    StoredDiagnosticDto {
        timestamp: v.diagnostic.at,
        code: v.diagnostic.code,
        message: v.diagnostic.message,
        remediation: v.diagnostic.remediation,
        cause: v.cause,
        docs: v.docs,
        details: v.details,
    }
}
fn stored_diagnostic(value: &core::Diagnostic) -> Result<StoredDiagnosticDto, Failure> {
    Ok(stored_diagnostic_dto(from_canonical_diagnostic(value)?))
}
fn native_diagnostic(value: StoredDiagnosticDto) -> Result<core::Diagnostic, Failure> {
    let remediation = match value.remediation {
        RemediationDto::Recoverable { steps } => core::Remediation::Recoverable {
            steps: core::RecoverableSteps::all(steps),
        },
        RemediationDto::NotRecoverable { justification } => {
            core::Remediation::not_recoverable(justification)
        }
    };
    let result = core::Diagnostic {
        timestamp: timestamp(value.timestamp, "diagnostic.timestamp")?,
        code: core::ErrorCode::new_owned(value.code),
        message: value.message,
        remediation,
        cause: value.cause,
        docs: value.docs,
        details: value
            .details
            .into_iter()
            .map(|(key, value)| Ok((key, to_value(value, "details", false, 0)?)))
            .collect::<Result<_, Failure>>()?,
    };
    from_canonical_diagnostic(&result)?;
    Ok(result)
}
fn start_span(v: SpanRecordDto) -> Result<core::v2::SpanRecord<core::SpanStarted>, Failure> {
    let mut span = core::v2::SpanRecord::new(
        timestamp(v.timestamp, "timestamp")?,
        checked(core::ServiceName::new(v.service), "service")?,
        checked(core::ActionName::new(v.name), "name")?,
        v.trace.try_into()?,
        to_attributes(v.attributes)?,
    )
    .with_kind(v.kind.into())
    .with_links(
        v.links
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
    );
    if let Some(diagnostic) = v.diagnostic {
        span = span.with_diagnostic(native_diagnostic(diagnostic)?);
    }
    Ok(span)
}
impl TryFrom<SpanSignalDto> for core::v2::SpanSignal {
    type Error = Failure;
    fn try_from(value: SpanSignalDto) -> Result<Self, Self::Error> {
        match value {
            SpanSignalDto::Started(v) => {
                if v.duration_ms.is_some() || v.status != SpanStatusDto::Unset {
                    return Err(invalid_input(
                        "span",
                        "started spans require unset status and no duration",
                    ));
                }
                Ok(Self::Started(start_span(v)?))
            }
            SpanSignalDto::Ended(v) => {
                let duration = checked(
                    v.duration_ms
                        .as_ref()
                        .ok_or_else(|| {
                            invalid_input("duration_ms", "ended spans require duration_ms")
                        })?
                        .as_u64(),
                    "duration_ms",
                )?;
                let status = v.status.into();
                Ok(Self::Ended(start_span(v)?.end(status, duration.into())))
            }
            SpanSignalDto::Event(v) => Ok(Self::Event(core::v2::SpanEvent {
                timestamp: timestamp(v.timestamp, "timestamp")?,
                trace: v.trace.try_into()?,
                name: checked(core::ActionName::new(v.name), "name")?,
                attributes: to_attributes(v.attributes)?,
                diagnostic: v.diagnostic.map(native_diagnostic).transpose()?,
            })),
        }
    }
}
fn span_record<S: core::v2::SpanState>(
    v: &core::v2::SpanRecord<S>,
    duration: Option<core::DurationMs>,
) -> Result<SpanRecordDto, Failure> {
    Ok(SpanRecordDto {
        timestamp: v.timestamp().to_string(),
        service: v.service().as_str().into(),
        name: v.name().as_str().into(),
        trace: v.trace().into(),
        status: v.status().into(),
        diagnostic: v.diagnostic().map(stored_diagnostic).transpose()?,
        attributes: from_attributes(v.attributes())?,
        duration_ms: duration.map(|v| v.as_u64().into()),
        kind: v.kind().try_into()?,
        links: v
            .links()
            .iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
    })
}
impl TryFrom<&core::v2::SpanSignal> for SpanSignalDto {
    type Error = Failure;
    fn try_from(v: &core::v2::SpanSignal) -> Result<Self, Self::Error> {
        match v {
            core::v2::SpanSignal::Started(v) => Ok(Self::Started(span_record(v, None)?)),
            core::v2::SpanSignal::Ended(v) => {
                Ok(Self::Ended(span_record(v, Some(v.duration_ms()))?))
            }
            core::v2::SpanSignal::Event(v) => Ok(Self::Event(SpanEventDto {
                timestamp: v.timestamp.to_string(),
                trace: (&v.trace).into(),
                name: v.name.as_str().into(),
                attributes: from_attributes(&v.attributes)?,
                diagnostic: v.diagnostic.as_ref().map(stored_diagnostic).transpose()?,
            })),
        }
    }
}
/// Validates a span wire signal and reconstructs native typestate through its public API.
///
/// # Errors
/// Rejects malformed correlation, attributes, diagnostics, unknown states or invalid lifecycle fields.
pub fn decode_span(value: Value) -> Result<core::v2::SpanSignal, Failure> {
    let dto: SpanSignalDto = decode(value, "span")?;
    dto.try_into()
}

/// Decodes the compatible operational envelope while retaining additive canonical metadata.
///
/// # Errors
/// Rejects malformed envelopes, overlarge metadata and invalid tagged payloads.
/// Unknown error kinds remain `UnknownRemote`, never a successful result.
pub fn decode_canonical_envelope<T: DeserializeOwned>(
    value: Value,
) -> Result<CanonicalWireEnvelope<T>, Failure> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_input("response", "expected envelope object"))?;
    let raw_version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .ok_or_else(|| invalid_input("response", "invalid schema version"))?;
    version(raw_version)?;
    match object.get("kind").and_then(Value::as_str) {
        Some("ok") => {
            if object.contains_key("error") {
                return Err(invalid_input("response", "conflicting envelope payload"));
            }
            let value = object
                .get("value")
                .ok_or_else(|| invalid_input("response", "missing value"))?
                .clone();
            Ok(CanonicalWireEnvelope::Ok {
                schema_version: 1,
                value: checked(serde_json::from_value(value), "response")?,
            })
        }
        Some("error") => {
            if object.contains_key("value") {
                return Err(invalid_input("response", "conflicting envelope payload"));
            }
            let raw = object
                .get("error")
                .ok_or_else(|| invalid_input("response", "missing error"))?;
            let diagnostic: Diagnostic = checked(serde_json::from_value(raw.clone()), "response")?;
            validate_diagnostic(&diagnostic, "response.error")?;
            let tag = raw
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_input("response", "missing failure kind"))?;
            let error = if CanonicalFailureDto::KNOWN_KINDS.contains(&tag) {
                decode(raw.clone(), "response")?
            } else {
                CanonicalFailureDto::UnknownRemote {
                    diagnostic: Box::new(decode(raw.clone(), "response")?),
                    remote_kind: tag.into(),
                }
            };
            native_diagnostic(stored_diagnostic_dto(error.diagnostic().clone()))?;
            Ok(CanonicalWireEnvelope::Error {
                schema_version: 1,
                error,
            })
        }
        _ => Err(invalid_input("response", "invalid result kind")),
    }
}
