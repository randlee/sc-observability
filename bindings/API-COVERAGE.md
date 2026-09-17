# Binding API coverage, schema v1

Rust DTO declarations own these projections. Frozen wire cases live in
`conformance/v1/schema-cases.json`; every registered type roundtrips through Rust
Serde and both generated validators. `schema-generator/tests/conformance.rs`
compares against committed normalized values without rewriting them.

| Operation | Conversion evidence | Runtime evidence |
| --- | --- | --- |
| Event admission | `decode_event`, `to_core_event`, integer/stamp/provenance tests; external bundled consumer | B.3b/B.3a/B.4 append native and transported admission |
| Query | `decode_query`, `to_core_query`, inclusive bounds/default/limit tests; external consumer | B.3b/B.3a/B.4 append runtime query |
| Snapshot | `from_core_event`, `from_core_snapshot`, all nested stored fields fixture | Native runtime query returns this projection |
| Health | `from_core_health`, `from_logging_health`, complete health/max revision fixture; external consumer | B.3b alone adds bridge snapshots |
| Runtime levels | `decode_level_request`, `from_level_change`, `from_level_error`, all native error variants and unsuccessful diagnostics | B.3b owner implementation |
| Remote envelopes | `decode_envelope`, known/unknown/malformed/oversized tests | B.3a/B.4 transport containment |
| Paths/timeouts | `from_path`, `to_path`, `decode_timeout`, non-UTF8 and zero/max/overflow fixtures | Adapters enforce before operations |
| encodeValue / encodeEvent | Schema declarations and integer/provenance limits provided here | B.3a ergonomic implementations and external transport proof |

| Wire type | Frozen conversion fixture |
| --- | --- |
| `AdmissionDto` | `InputAdmissionDto-*` and `OutputAdmissionDto-*`; Rust Serde and generated TS/Python |
| `AdmissionOperationDto` | `InputAdmissionOperationDto-*` and `OutputAdmissionOperationDto-*`; Rust Serde and generated TS/Python |
| `AvailabilityDto` | `InputAvailabilityDto-*` and `OutputAvailabilityDto-*`; Rust Serde and generated TS/Python |
| `BridgeHealthDto` | `InputBridgeHealthDto-*` and `OutputBridgeHealthDto-*`; Rust Serde and generated TS/Python |
| `ChangeDiagnosticDto` | `InputChangeDiagnosticDto-*` and `OutputChangeDiagnosticDto-*`; Rust Serde and generated TS/Python |
| `ClientOutcome` | `InputClientOutcome-*` and `OutputClientOutcome-*`; Rust Serde and generated TS/Python |
| `ClientStatus` | `InputClientStatus-*` and `OutputClientStatus-*`; Rust Serde and generated TS/Python |
| `CompletionDto` | `InputCompletionDto-*` and `OutputCompletionDto-*`; Rust Serde and generated TS/Python |
| `CompletionOperationDto` | `InputCompletionOperationDto-*` and `OutputCompletionOperationDto-*`; Rust Serde and generated TS/Python |
| `DecimalDto` | `InputDecimalDto-*` and `OutputDecimalDto-*`; Rust Serde and generated TS/Python |
| `Diagnostic` | `InputDiagnostic-*` and `OutputDiagnostic-*`; Rust Serde and generated TS/Python |
| `DiagnosticSummaryDto` | `InputDiagnosticSummaryDto-*` and `OutputDiagnosticSummaryDto-*`; Rust Serde and generated TS/Python |
| `DispatchDto` | `InputDispatchDto-*` and `OutputDispatchDto-*`; Rust Serde and generated TS/Python |
| `DropCountsDto` | `InputDropCountsDto-*` and `OutputDropCountsDto-*`; Rust Serde and generated TS/Python |
| `Failure` | `InputFailure-*` and `OutputFailure-*`; Rust Serde and generated TS/Python |
| `FailureCountsDto` | `InputFailureCountsDto-*` and `OutputFailureCountsDto-*`; Rust Serde and generated TS/Python |
| `FieldMatchDto` | `InputFieldMatchDto-*` and `OutputFieldMatchDto-*`; Rust Serde and generated TS/Python |
| `FlushRequest` | `InputFlushRequest-*` and `OutputFlushRequest-*`; Rust Serde and generated TS/Python |
| `HealthRequest` | `InputHealthRequest-*` and `OutputHealthRequest-*`; Rust Serde and generated TS/Python |
| `LevelChangeDto` | `InputLevelChangeDto-*` and `OutputLevelChangeDto-*`; Rust Serde and generated TS/Python |
| `LevelChangeRequest` | `InputLevelChangeRequest-*` and `OutputLevelChangeRequest-*`; Rust Serde and generated TS/Python |
| `LevelChangeSourceDto` | `InputLevelChangeSourceDto-*` and `OutputLevelChangeSourceDto-*`; Rust Serde and generated TS/Python |
| `LevelDto` | `InputLevelDto-*` and `OutputLevelDto-*`; Rust Serde and generated TS/Python |
| `LevelFilterDto` | `InputLevelFilterDto-*` and `OutputLevelFilterDto-*`; Rust Serde and generated TS/Python |
| `LevelRequestDto` | `InputLevelRequestDto-*` and `OutputLevelRequestDto-*`; Rust Serde and generated TS/Python |
| `LevelStateDto` | `InputLevelStateDto-*` and `OutputLevelStateDto-*`; Rust Serde and generated TS/Python |
| `LifecycleDto` | `InputLifecycleDto-*` and `OutputLifecycleDto-*`; Rust Serde and generated TS/Python |
| `LogEventDto` | `InputLogEventDto-*` and `OutputLogEventDto-*`; Rust Serde and generated TS/Python |
| `LogHealthDto` | `InputLogHealthDto-*` and `OutputLogHealthDto-*`; Rust Serde and generated TS/Python |
| `LogOperationDto` | `InputLogOperationDto-*` and `OutputLogOperationDto-*`; Rust Serde and generated TS/Python |
| `LogOrderDto` | `InputLogOrderDto-*` and `OutputLogOrderDto-*`; Rust Serde and generated TS/Python |
| `LogQueryDto` | `InputLogQueryDto-*` and `OutputLogQueryDto-*`; Rust Serde and generated TS/Python |
| `LogSnapshotDto` | `InputLogSnapshotDto-*` and `OutputLogSnapshotDto-*`; Rust Serde and generated TS/Python |
| `LoggingHealthDto` | `InputLoggingHealthDto-*` and `OutputLoggingHealthDto-*`; Rust Serde and generated TS/Python |
| `MaintenanceHealthDto` | `InputMaintenanceHealthDto-*` and `OutputMaintenanceHealthDto-*`; Rust Serde and generated TS/Python |
| `OperationDiagnosticDto` | `InputOperationDiagnosticDto-*` and `OutputOperationDiagnosticDto-*`; Rust Serde and generated TS/Python |
| `PathDto` | `InputPathDto-*` and `OutputPathDto-*`; Rust Serde and generated TS/Python |
| `ProcessIdentityDto` | `InputProcessIdentityDto-*` and `OutputProcessIdentityDto-*`; Rust Serde and generated TS/Python |
| `QueryHealthDto` | `InputQueryHealthDto-*` and `OutputQueryHealthDto-*`; Rust Serde and generated TS/Python |
| `QueryRequest` | `InputQueryRequest-*` and `OutputQueryRequest-*`; Rust Serde and generated TS/Python |
| `QueryStateDto` | `InputQueryStateDto-*` and `OutputQueryStateDto-*`; Rust Serde and generated TS/Python |
| `RemediationDto` | `InputRemediationDto-*` and `OutputRemediationDto-*`; Rust Serde and generated TS/Python |
| `ResultDtoAdmissionDto` | `InputResultDtoAdmissionDto-*` and `OutputResultDtoAdmissionDto-*`; Rust Serde and generated TS/Python |
| `ResultDtoClientOutcome` | `InputResultDtoClientOutcome-*` and `OutputResultDtoClientOutcome-*`; Rust Serde and generated TS/Python |
| `ResultDtoClientStatus` | `InputResultDtoClientStatus-*` and `OutputResultDtoClientStatus-*`; Rust Serde and generated TS/Python |
| `ResultDtoCompletionDto` | `InputResultDtoCompletionDto-*` and `OutputResultDtoCompletionDto-*`; Rust Serde and generated TS/Python |
| `ResultDtoDispatchDto` | `InputResultDtoDispatchDto-*` and `OutputResultDtoDispatchDto-*`; Rust Serde and generated TS/Python |
| `ResultDtoLevelChangeDto` | `InputResultDtoLevelChangeDto-*` and `OutputResultDtoLevelChangeDto-*`; Rust Serde and generated TS/Python |
| `ResultDtoLogHealthDto` | `InputResultDtoLogHealthDto-*` and `OutputResultDtoLogHealthDto-*`; Rust Serde and generated TS/Python |
| `ResultDtoLogSnapshotDto` | `InputResultDtoLogSnapshotDto-*` and `OutputResultDtoLogSnapshotDto-*`; Rust Serde and generated TS/Python |
| `SinkHealthDto` | `InputSinkHealthDto-*` and `OutputSinkHealthDto-*`; Rust Serde and generated TS/Python |
| `StateTransitionDto` | `InputStateTransitionDto-*` and `OutputStateTransitionDto-*`; Rust Serde and generated TS/Python |
| `StoredDiagnosticDto` | `InputStoredDiagnosticDto-*` and `OutputStoredDiagnosticDto-*`; Rust Serde and generated TS/Python |
| `StoredEventDto` | `InputStoredEventDto-*` and `OutputStoredEventDto-*`; Rust Serde and generated TS/Python |
| `TraceContextDto` | `InputTraceContextDto-*` and `OutputTraceContextDto-*`; Rust Serde and generated TS/Python |
| `TryLogRequest` | `InputTryLogRequest-*` and `OutputTryLogRequest-*`; Rust Serde and generated TS/Python |
| `ValueDto` | `InputValueDto-*` and `OutputValueDto-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeAdmissionDto` | `InputWireEnvelopeAdmissionDto-*` and `OutputWireEnvelopeAdmissionDto-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeClientOutcome` | `InputWireEnvelopeClientOutcome-*` and `OutputWireEnvelopeClientOutcome-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeClientStatus` | `InputWireEnvelopeClientStatus-*` and `OutputWireEnvelopeClientStatus-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeCompletionDto` | `InputWireEnvelopeCompletionDto-*` and `OutputWireEnvelopeCompletionDto-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeDispatchDto` | `InputWireEnvelopeDispatchDto-*` and `OutputWireEnvelopeDispatchDto-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeLevelChangeDto` | `InputWireEnvelopeLevelChangeDto-*` and `OutputWireEnvelopeLevelChangeDto-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeLogHealthDto` | `InputWireEnvelopeLogHealthDto-*` and `OutputWireEnvelopeLogHealthDto-*`; Rust Serde and generated TS/Python |
| `WireEnvelopeLogSnapshotDto` | `InputWireEnvelopeLogSnapshotDto-*` and `OutputWireEnvelopeLogSnapshotDto-*`; Rust Serde and generated TS/Python |
| `WorkerStateDto` | `InputWorkerStateDto-*` and `OutputWorkerStateDto-*`; Rust Serde and generated TS/Python |

Explicit exclusions: Tauri IPC/authorization, Python logger/runtime/async/handler behavior,
native coordinator races, bridge health/error conversion, wheels/sdists, and registry
publication are owned by later sprints. DTOs carry no ErrorContext, source chain,
backtrace, Logger, LogGuard, LevelOwner, or transport/native ownership capability.
Source bundle proof is prepublication; it does not claim registry-only installation.
