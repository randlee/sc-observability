//! Checked adapters from the existing neutral producer records.
use super::{
    AggregationTemporality, AnyValue, DataPointFlags, HistogramDataPoint, InstrumentationScope,
    KeyValues, LogPoint, MetricData, MetricStream, NumberPoint, NumberValue, Resource,
    SeverityNumber, SignalValidationError, SpanKindPoint, SpanLinkPoint, SpanPoint,
    SpanStatusPoint, StatusCode,
};
use crate::{
    LogEvent, SpanStatus,
    otlp::{OtlpInstrumentationScope, OtlpResource},
    v2,
};

fn attributes(values: &v2::Attributes, path: &str) -> Result<KeyValues, SignalValidationError> {
    KeyValues::try_from_iter(
        values
            .iter()
            .map(|(k, v)| Ok((k.clone().into(), attribute(v, &format!("{path}.{k}"))?)))
            .collect::<Result<Vec<_>, SignalValidationError>>()?,
    )
}
fn attribute(value: &v2::AttributeValue, path: &str) -> Result<AnyValue, SignalValidationError> {
    Ok(match value {
        v2::AttributeValue::Bool(v) => AnyValue::Bool(*v),
        v2::AttributeValue::Int(v) => AnyValue::Int(*v),
        v2::AttributeValue::UInt(v) => AnyValue::UInt(*v),
        v2::AttributeValue::Float(v) => AnyValue::Double(f64::from(*v).into()),
        v2::AttributeValue::String(v) => AnyValue::String(v.clone()),
        v2::AttributeValue::Array(v) => AnyValue::Array(
            v.iter()
                .enumerate()
                .map(|(i, v)| attribute(v, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?,
        ),
        v2::AttributeValue::Object(v) => AnyValue::KvList(attributes(v, path)?),
        v2::AttributeValue::Null => {
            return Err(SignalValidationError::invalid(
                path,
                "null has no supported signal representation",
            ));
        }
    })
}
fn json_value(value: &serde_json::Value, path: &str) -> Result<AnyValue, SignalValidationError> {
    use serde_json::Value;
    Ok(match value {
        Value::Null => {
            return Err(SignalValidationError::invalid(
                path,
                "null has no supported signal representation",
            ));
        }
        Value::Bool(v) => AnyValue::Bool(*v),
        Value::String(v) => AnyValue::String(v.clone()),
        Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                AnyValue::Int(i)
            } else if let Some(u) = v.as_u64() {
                AnyValue::UInt(u)
            } else {
                AnyValue::Double(
                    v.as_f64()
                        .ok_or_else(|| {
                            SignalValidationError::invalid(path, "unrepresentable number")
                        })?
                        .into(),
                )
            }
        }
        Value::Array(values) => AnyValue::Array(
            values
                .iter()
                .enumerate()
                .map(|(i, v)| json_value(v, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(values) => AnyValue::KvList(KeyValues::try_from_iter(
            values
                .iter()
                .map(|(k, v)| Ok((k.clone().into(), json_value(v, &format!("{path}.{k}"))?)))
                .collect::<Result<Vec<_>, SignalValidationError>>()?,
        )?),
    })
}
impl TryFrom<OtlpResource> for Resource {
    type Error = SignalValidationError;
    fn try_from(value: OtlpResource) -> Result<Self, Self::Error> {
        Ok(Self::new(
            attributes(&value.attributes, "attributes")?,
            0,
            vec![],
            value.schema_url,
        ))
    }
}
impl TryFrom<OtlpInstrumentationScope> for InstrumentationScope {
    type Error = SignalValidationError;
    fn try_from(value: OtlpInstrumentationScope) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.name,
            value.version,
            attributes(&value.attributes, "attributes")?,
            0,
            value.schema_url,
        ))
    }
}
impl TryFrom<LogEvent> for LogPoint {
    type Error = SignalValidationError;
    fn try_from(value: LogEvent) -> Result<Self, Self::Error> {
        let mut attrs = Vec::new();
        for (key, v) in &value.fields {
            attrs.push((key.clone().into(), json_value(v, &format!("fields.{key}"))?));
        }
        let (severity, severity_text) = SeverityNumber::from_level(value.level);
        Ok(Self::new(
            Some(value.timestamp),
            value.timestamp,
            severity,
            Some(severity_text.to_owned()),
            Some(value.action.as_str().to_owned()),
            value.message.map(AnyValue::String),
            KeyValues::try_from_iter(attrs)?,
            0,
            0,
            value.trace.as_ref().map(|t| t.trace_id.clone()),
            value.trace.map(|t| t.span_id),
        ))
    }
}
impl TryFrom<v2::SpanRecord<v2::SpanEnded>> for SpanPoint {
    type Error = SignalValidationError;
    fn try_from(value: v2::SpanRecord<v2::SpanEnded>) -> Result<Self, Self::Error> {
        let attrs = attributes(value.attributes(), "attributes")?;
        let links = value
            .links()
            .iter()
            .enumerate()
            .map(|(i, l)| {
                Ok(SpanLinkPoint::new(
                    l.trace_id.clone(),
                    l.span_id.clone(),
                    None,
                    attributes(&l.attributes, &format!("links[{i}].attributes"))?,
                    0,
                    u32::from(l.flags.bits()),
                ))
            })
            .collect::<Result<Vec<_>, SignalValidationError>>()?;
        let millis = i64::try_from(value.duration_ms().as_u64())
            .map_err(|_| SignalValidationError::invalid("duration_ms", "duration overflow"))?;
        let end = value
            .timestamp()
            .into_inner()
            .checked_add(time::Duration::milliseconds(millis))
            .ok_or_else(|| {
                SignalValidationError::invalid("duration_ms", "end timestamp overflow")
            })?;
        Self::try_new(
            value.trace().trace_id.clone(),
            value.trace().span_id.clone(),
            None,
            value.trace().parent_span_id.clone(),
            u32::from(value.trace().flags.bits()),
            value.name().as_str().into(),
            match value.kind() {
                v2::SpanKind::Internal => SpanKindPoint::Internal,
                v2::SpanKind::Server => SpanKindPoint::Server,
                v2::SpanKind::Client => SpanKindPoint::Client,
                v2::SpanKind::Producer => SpanKindPoint::Producer,
                v2::SpanKind::Consumer => SpanKindPoint::Consumer,
            },
            value.timestamp(),
            end.into(),
            attrs,
            0,
            vec![],
            0,
            links,
            0,
            SpanStatusPoint::new(
                match value.status() {
                    SpanStatus::Ok => StatusCode::Ok,
                    SpanStatus::Error => StatusCode::Error,
                    SpanStatus::Unset => StatusCode::Unset,
                },
                value.diagnostic().map(|d| d.message.clone()),
            ),
        )
    }
}
impl TryFrom<v2::MetricRecord> for MetricStream {
    type Error = SignalValidationError;
    fn try_from(value: v2::MetricRecord) -> Result<Self, Self::Error> {
        let attrs = attributes(value.attributes(), "attributes")?;
        let number = |v: v2::FiniteF64, start| {
            NumberPoint::try_new(
                attrs.clone(),
                start,
                value.timestamp(),
                NumberValue::Double(f64::from(v).into()),
                vec![],
                DataPointFlags::default(),
            )
        };
        let data = match value.value() {
            v2::MetricValue::Gauge(v) => MetricData::Gauge {
                points: vec![number(*v, None)?],
            },
            v2::MetricValue::Sum {
                value: v,
                monotonic,
                temporality,
                start_time,
            } => MetricData::Sum {
                points: vec![number(*v, Some(*start_time))?],
                temporality: temporal(*temporality),
                monotonic: *monotonic,
            },
            v2::MetricValue::Histogram {
                point,
                temporality,
                start_time,
            } => MetricData::Histogram {
                points: vec![HistogramDataPoint::try_new(
                    attrs,
                    Some(*start_time),
                    value.timestamp(),
                    point.count(),
                    Some(f64::from(point.sum()).into()),
                    point.bucket_counts().to_vec(),
                    point
                        .explicit_bounds()
                        .iter()
                        .map(|v| f64::from(*v).into())
                        .collect(),
                    vec![],
                    DataPointFlags::default(),
                    None,
                    None,
                )?],
                temporality: temporal(*temporality),
            },
        };
        Self::try_new(
            value.name().clone(),
            None,
            value.unit().map(|u| u.as_str().into()),
            KeyValues::default(),
            data,
        )
    }
}
fn temporal(value: v2::AggregationTemporality) -> AggregationTemporality {
    match value {
        v2::AggregationTemporality::Delta => AggregationTemporality::Delta,
        v2::AggregationTemporality::Cumulative => AggregationTemporality::Cumulative,
    }
}
