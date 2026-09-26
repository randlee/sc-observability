use sc_observability_dto::*;
use serde_json::Value;
fn roundtrip<T: serde::de::DeserializeOwned + serde::Serialize>(v: Value) -> Value {
    serde_json::to_value(serde_json::from_value::<T>(v).expect("Serde accepts schema fixture"))
        .unwrap()
}
#[test]
fn every_registered_type_agrees_with_serde_and_frozen_expectations() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../../conformance/v1/schema-cases.json")).unwrap();
    for case in cases {
        if case["valid"] != true {
            continue;
        }
        let entry = case["entrypoint"].as_str().unwrap();
        let name = entry
            .strip_prefix("Input")
            .or_else(|| entry.strip_prefix("Output"))
            .unwrap();
        let value = case["value"].clone();
        let actual = match name {
            "AdmissionDto" => roundtrip::<AdmissionDto>(value),
            "AvailabilityDto" => roundtrip::<AvailabilityDto>(value),
            "BridgeHealthDto" => roundtrip::<BridgeHealthDto>(value),
            "ChangeDiagnosticDto" => roundtrip::<ChangeDiagnosticDto>(value),
            "CompletionDto" => roundtrip::<CompletionDto>(value),
            "DecimalDto" => roundtrip::<DecimalDto>(value),
            "Diagnostic" => roundtrip::<Diagnostic>(value),
            "DiagnosticSummaryDto" => roundtrip::<DiagnosticSummaryDto>(value),
            "DispatchDto" => roundtrip::<DispatchDto>(value),
            "DropCountsDto" => roundtrip::<DropCountsDto>(value),
            "Failure" => roundtrip::<Failure>(value),
            "FieldMatchDto" => roundtrip::<FieldMatchDto>(value),
            "FlushRequest" => roundtrip::<FlushRequest>(value),
            "HealthRequest" => roundtrip::<HealthRequest>(value),
            "LevelChangeDto" => roundtrip::<LevelChangeDto>(value),
            "LevelChangeRequest" => roundtrip::<LevelChangeRequest>(value),
            "LevelChangeSourceDto" => roundtrip::<LevelChangeSourceDto>(value),
            "LevelDto" => roundtrip::<LevelDto>(value),
            "LevelFilterDto" => roundtrip::<LevelFilterDto>(value),
            "LevelRequestDto" => roundtrip::<LevelRequestDto>(value),
            "LevelStateDto" => roundtrip::<LevelStateDto>(value),
            "LifecycleDto" => roundtrip::<LifecycleDto>(value),
            "LogEventDto" => roundtrip::<LogEventDto>(value),
            "LogHealthDto" => roundtrip::<LogHealthDto>(value),
            "LogOrderDto" => roundtrip::<LogOrderDto>(value),
            "LogQueryDto" => roundtrip::<LogQueryDto>(value),
            "LogSnapshotDto" => roundtrip::<LogSnapshotDto>(value),
            "LoggingHealthDto" => roundtrip::<LoggingHealthDto>(value),
            "MaintenanceHealthDto" => roundtrip::<MaintenanceHealthDto>(value),
            "PathDto" => roundtrip::<PathDto>(value),
            "ProcessIdentityDto" => roundtrip::<ProcessIdentityDto>(value),
            "QueryHealthDto" => roundtrip::<QueryHealthDto>(value),
            "QueryRequest" => roundtrip::<QueryRequest>(value),
            "QueryStateDto" => roundtrip::<QueryStateDto>(value),
            "RemediationDto" => roundtrip::<RemediationDto>(value),
            "ResultDtoAdmissionDto" => roundtrip::<ResultDto<AdmissionDto>>(value),
            "ResultDtoCompletionDto" => roundtrip::<ResultDto<CompletionDto>>(value),
            "ResultDtoDispatchDto" => roundtrip::<ResultDto<DispatchDto>>(value),
            "ResultDtoLevelChangeDto" => roundtrip::<ResultDto<LevelChangeDto>>(value),
            "ResultDtoLogHealthDto" => roundtrip::<ResultDto<LogHealthDto>>(value),
            "ResultDtoLogSnapshotDto" => roundtrip::<ResultDto<LogSnapshotDto>>(value),
            "SinkHealthDto" => roundtrip::<SinkHealthDto>(value),
            "StateTransitionDto" => roundtrip::<StateTransitionDto>(value),
            "StoredDiagnosticDto" => roundtrip::<StoredDiagnosticDto>(value),
            "StoredEventDto" => roundtrip::<StoredEventDto>(value),
            "TraceContextDto" => roundtrip::<TraceContextDto>(value),
            "TryLogRequest" => roundtrip::<TryLogRequest>(value),
            "ValueDto" => roundtrip::<ValueDto>(value),
            "WireEnvelopeAdmissionDto" => roundtrip::<WireEnvelope<AdmissionDto>>(value),
            "WireEnvelopeCompletionDto" => roundtrip::<WireEnvelope<CompletionDto>>(value),
            "WireEnvelopeDispatchDto" => roundtrip::<WireEnvelope<DispatchDto>>(value),
            "WireEnvelopeLevelChangeDto" => roundtrip::<WireEnvelope<LevelChangeDto>>(value),
            "WireEnvelopeLogHealthDto" => roundtrip::<WireEnvelope<LogHealthDto>>(value),
            "WireEnvelopeLogSnapshotDto" => roundtrip::<WireEnvelope<LogSnapshotDto>>(value),
            "WorkerStateDto" => roundtrip::<WorkerStateDto>(value),
            "OperationDiagnosticDto" => roundtrip::<OperationDiagnosticDto>(value),
            "ClientOutcome" => roundtrip::<ClientOutcome>(value),
            "ClientStatus" => roundtrip::<ClientStatus>(value),
            "FailureCountsDto" => roundtrip::<FailureCountsDto>(value),
            "LogOperationDto" => roundtrip::<LogOperationDto>(value),
            "AdmissionOperationDto" => roundtrip::<AdmissionOperationDto>(value),
            "CompletionOperationDto" => roundtrip::<CompletionOperationDto>(value),
            "ResultDtoClientOutcome" => roundtrip::<ResultDto<ClientOutcome>>(value),
            "ResultDtoClientStatus" => roundtrip::<ResultDto<ClientStatus>>(value),
            "WireEnvelopeClientOutcome" => roundtrip::<WireEnvelope<ClientOutcome>>(value),
            "WireEnvelopeClientStatus" => roundtrip::<WireEnvelope<ClientStatus>>(value),
            "CanonicalDiagnosticDto" => roundtrip::<CanonicalDiagnosticDto>(value),
            "CanonicalFailureDto" => roundtrip::<CanonicalFailureDto>(value),
            "TraceContextV2Dto" => roundtrip::<TraceContextV2Dto>(value),
            "SpanLinkDto" => roundtrip::<SpanLinkDto>(value),
            "SpanKindDto" => roundtrip::<SpanKindDto>(value),
            "AggregationTemporalityDto" => roundtrip::<AggregationTemporalityDto>(value),
            "HistogramPointDto" => roundtrip::<HistogramPointDto>(value),
            "MetricValueDto" => roundtrip::<MetricValueDto>(value),
            "MetricRecordDto" => roundtrip::<MetricRecordDto>(value),
            "SpanStatusDto" => roundtrip::<SpanStatusDto>(value),
            "SpanRecordDto" => roundtrip::<SpanRecordDto>(value),
            "SpanEventDto" => roundtrip::<SpanEventDto>(value),
            "SpanSignalDto" => roundtrip::<SpanSignalDto>(value),
            "CanonicalWireEnvelopeAdmissionDto" => {
                roundtrip::<CanonicalWireEnvelope<AdmissionDto>>(value)
            }
            _ => panic!("unregistered fixture type: {name}"),
        };
        assert_eq!(actual, case["serde_output"], "{}", case["id"]);
    }
}

#[test]
fn semantic_negatives_have_exact_failure_kinds_and_codes() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../../conformance/v1/conversion-cases.json")).unwrap();
    for case in cases {
        let value = case["value"].clone();
        let error = match case["operation"].as_str().unwrap() {
            "metric" => decode_metric(value).unwrap_err(),
            "span" => decode_span(value).unwrap_err(),
            "event" => decode_event(value).unwrap_err(),
            "query" => decode_query(value).unwrap_err(),
            "level" => decode_level_request(value).unwrap_err(),
            "timeout" => decode_timeout(value).unwrap_err(),
            "envelope" => decode_envelope::<AdmissionDto>(value).unwrap_err(),
            _ => panic!("unknown operation"),
        };
        let wire = serde_json::to_value(error).unwrap();
        assert_eq!(wire["kind"], case["kind"], "{}", case["id"]);
        assert_eq!(wire["code"], case["code"], "{}", case["id"]);
    }
}
