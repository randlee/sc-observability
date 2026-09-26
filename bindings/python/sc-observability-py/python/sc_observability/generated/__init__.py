# Generated from canonical schema. Do not edit.
from __future__ import annotations
from dataclasses import dataclass, field as dataclass_field
from typing import Literal, Mapping, NoReturn, TypeAlias
from types import MappingProxyType
import json
import re

@dataclass(frozen=True, kw_only=True)
class InputAdmissionAccepted:
    kind: Literal['accepted'] = dataclass_field(default='accepted', init=False)

@dataclass(frozen=True, kw_only=True)
class InputAdmissionFiltered:
    kind: Literal['filtered'] = dataclass_field(default='filtered', init=False)

@dataclass(frozen=True, kw_only=True)
class InputBridgeHealth:
    active_log_path: InputPath
    configured_level: InputLevelFilter
    dropped: InputDropCounts
    effective_level: InputLevelFilter
    level_revision: InputDecimal
    lifecycle: InputLifecycle
    logging: InputLoggingHealth
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputCanonicalDiagnostic:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureValidation:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    field: str
    kind: Literal['validation'] = dataclass_field(default='validation', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureQueueFull:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['queue_full'] = dataclass_field(default='queue_full', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureBelowBaseline:
    at: str
    cause: str | None = None
    code: str
    configured: InputLevelFilter
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['below_baseline'] = dataclass_field(default='below_baseline', init=False)
    message: str
    remediation: InputRemediation
    requested: InputLevelFilter

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureUnsupportedLevel:
    at: str
    available: InputLevelFilter
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unsupported_level'] = dataclass_field(default='unsupported_level', init=False)
    message: str
    remediation: InputRemediation
    requested: InputLevelFilter

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailurePermissionDenied:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['permission_denied'] = dataclass_field(default='permission_denied', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureClosed:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['closed'] = dataclass_field(default='closed', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureUnavailable:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unavailable'] = dataclass_field(default='unavailable', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureIo:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['io'] = dataclass_field(default='io', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureTimeout:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['timeout'] = dataclass_field(default='timeout', init=False)
    message: str
    operation: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureCancelled:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['cancelled'] = dataclass_field(default='cancelled', init=False)
    message: str
    operation: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureUnsupportedVersion:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unsupported_version'] = dataclass_field(default='unsupported_version', init=False)
    message: str
    received: int
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureInternal:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['internal'] = dataclass_field(default='internal', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputCanonicalFailureUnknownRemote:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unknown_remote'] = dataclass_field(default='unknown_remote', init=False)
    message: str
    remediation: InputRemediation
    remote_kind: str

@dataclass(frozen=True, kw_only=True)
class InputCanonicalWireEnvelopeOk:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputAdmission

@dataclass(frozen=True, kw_only=True)
class InputCanonicalWireEnvelopeError:
    error: InputCanonicalFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputChangeDiagnosticAccepted:
    kind: Literal['accepted'] = dataclass_field(default='accepted', init=False)

@dataclass(frozen=True, kw_only=True)
class InputChangeDiagnosticNotAccepted:
    diagnostic: InputDiagnostic
    kind: Literal['not_accepted'] = dataclass_field(default='not_accepted', init=False)

@dataclass(frozen=True, kw_only=True)
class InputClientOutcomeIdle:
    kind: Literal['idle'] = dataclass_field(default='idle', init=False)

@dataclass(frozen=True, kw_only=True)
class InputClientOutcomeScheduled:
    kind: Literal['scheduled'] = dataclass_field(default='scheduled', init=False)
    operation: InputLogOperation

@dataclass(frozen=True, kw_only=True)
class InputClientOutcomeAccepted:
    kind: Literal['accepted'] = dataclass_field(default='accepted', init=False)
    operation: InputAdmissionOperation

@dataclass(frozen=True, kw_only=True)
class InputClientOutcomeFiltered:
    kind: Literal['filtered'] = dataclass_field(default='filtered', init=False)
    operation: InputAdmissionOperation

@dataclass(frozen=True, kw_only=True)
class InputClientOutcomeCompleted:
    kind: Literal['completed'] = dataclass_field(default='completed', init=False)
    operation: InputCompletionOperation

@dataclass(frozen=True, kw_only=True)
class InputClientStatus:
    failures_by_kind: InputFailureCounts
    in_flight: int
    last_failure: InputFailure | None = None
    last_result: InputResult7

@dataclass(frozen=True, kw_only=True)
class InputCompletionCompleted:
    kind: Literal['completed'] = dataclass_field(default='completed', init=False)

@dataclass(frozen=True, kw_only=True)
class InputDiagnostic:
    at: str
    code: str
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputDiagnosticSummary:
    at: str
    code: str | None = None
    message: str

@dataclass(frozen=True, kw_only=True)
class InputDispatchScheduled:
    kind: Literal['scheduled'] = dataclass_field(default='scheduled', init=False)

@dataclass(frozen=True, kw_only=True)
class InputDropCounts:
    invalid_event: InputDecimal
    logger_panicked: InputDecimal
    not_installed: InputDecimal
    queue_full: InputDecimal
    reentrant_emit: InputDecimal
    shutdown_timed_out: InputDecimal
    writer_degraded: InputDecimal

@dataclass(frozen=True, kw_only=True)
class InputFailureValidation:
    at: str
    code: str
    field: str
    kind: Literal['validation'] = dataclass_field(default='validation', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureQueueFull:
    at: str
    code: str
    kind: Literal['queue_full'] = dataclass_field(default='queue_full', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureBelowBaseline:
    at: str
    code: str
    configured: InputLevelFilter
    kind: Literal['below_baseline'] = dataclass_field(default='below_baseline', init=False)
    message: str
    remediation: InputRemediation
    requested: InputLevelFilter

@dataclass(frozen=True, kw_only=True)
class InputFailureUnsupportedLevel:
    at: str
    available: InputLevelFilter
    code: str
    kind: Literal['unsupported_level'] = dataclass_field(default='unsupported_level', init=False)
    message: str
    remediation: InputRemediation
    requested: InputLevelFilter

@dataclass(frozen=True, kw_only=True)
class InputFailurePermissionDenied:
    at: str
    code: str
    kind: Literal['permission_denied'] = dataclass_field(default='permission_denied', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureClosed:
    at: str
    code: str
    kind: Literal['closed'] = dataclass_field(default='closed', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureUnavailable:
    at: str
    code: str
    kind: Literal['unavailable'] = dataclass_field(default='unavailable', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureIo:
    at: str
    code: str
    kind: Literal['io'] = dataclass_field(default='io', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureTimeout:
    at: str
    code: str
    kind: Literal['timeout'] = dataclass_field(default='timeout', init=False)
    message: str
    operation: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureCancelled:
    at: str
    code: str
    kind: Literal['cancelled'] = dataclass_field(default='cancelled', init=False)
    message: str
    operation: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureUnsupportedVersion:
    at: str
    code: str
    kind: Literal['unsupported_version'] = dataclass_field(default='unsupported_version', init=False)
    message: str
    received: int
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureInternal:
    at: str
    code: str
    kind: Literal['internal'] = dataclass_field(default='internal', init=False)
    message: str
    remediation: InputRemediation

@dataclass(frozen=True, kw_only=True)
class InputFailureUnknownRemote:
    at: str
    code: str
    kind: Literal['unknown_remote'] = dataclass_field(default='unknown_remote', init=False)
    message: str
    remediation: InputRemediation
    remote_kind: str

@dataclass(frozen=True, kw_only=True)
class InputFailureCounts:
    below_baseline: InputDecimal
    cancelled: InputDecimal
    closed: InputDecimal
    internal: InputDecimal
    io: InputDecimal
    permission_denied: InputDecimal
    queue_full: InputDecimal
    timeout: InputDecimal
    unavailable: InputDecimal
    unknown_remote: InputDecimal
    unsupported_level: InputDecimal
    unsupported_version: InputDecimal
    validation: InputDecimal

@dataclass(frozen=True, kw_only=True)
class InputFieldMatch:
    field: str
    value: InputValue

@dataclass(frozen=True, kw_only=True)
class InputFlushRequest:
    schema_version: Literal[1]
    timeout_ms: int

@dataclass(frozen=True, kw_only=True)
class InputHealthRequest:
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputHistogramPoint:
    bucket_counts: tuple[InputDecimal, ...]
    count: InputDecimal
    explicit_bounds: tuple[float, ...]
    sum: float

@dataclass(frozen=True, kw_only=True)
class InputLevelChangeChanged:
    current: InputLevelState
    diagnostic: InputChangeDiagnostic
    kind: Literal['changed'] = dataclass_field(default='changed', init=False)
    previous: InputLevelState
    source: InputLevelChangeSource

@dataclass(frozen=True, kw_only=True)
class InputLevelChangeUnchanged:
    kind: Literal['unchanged'] = dataclass_field(default='unchanged', init=False)
    state: InputLevelState

@dataclass(frozen=True, kw_only=True)
class InputLevelChangeRequest:
    change: InputLevelRequest
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputLevelRequestElevate:
    kind: Literal['elevate'] = dataclass_field(default='elevate', init=False)
    level: InputLevelFilter

@dataclass(frozen=True, kw_only=True)
class InputLevelRequestReset:
    kind: Literal['reset'] = dataclass_field(default='reset', init=False)

@dataclass(frozen=True, kw_only=True)
class InputLevelState:
    configured_level: InputLevelFilter
    effective_level: InputLevelFilter
    level_revision: InputDecimal

@dataclass(frozen=True, kw_only=True)
class InputLogEvent:
    action: str
    correlation_id: str | None = None
    fields: Mapping[str, InputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    level: InputLevel
    message: str | None = None
    outcome: str | None = None
    request_id: str | None = None
    schema_version: Literal[1]
    target: str
    trace: InputTraceContext | None = None

@dataclass(frozen=True, kw_only=True)
class InputLogHealth:
    bridge: InputBridgeHealth | None = None
    level_state: InputLevelState
    logging: InputLoggingHealth
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputLogQuery:
    action: str | None = None
    correlation_id: str | None = None
    field_matches: tuple[InputFieldMatch, ...] = ()
    levels: tuple[InputLevel, ...] = ()
    limit: int = 100
    order: InputLogOrder = 'oldest_first'
    request_id: str | None = None
    schema_version: Literal[1]
    service: str | None = None
    since: str | None = None
    target: str | None = None
    until: str | None = None

@dataclass(frozen=True, kw_only=True)
class InputLogSnapshot:
    events: tuple[InputStoredEvent, ...]
    schema_version: Literal[1]
    truncated: bool

@dataclass(frozen=True, kw_only=True)
class InputLoggingHealth:
    active_log_path: InputPath
    dropped_events_total: InputDecimal
    flush_errors_total: InputDecimal
    last_error: InputDiagnosticSummary | None = None
    last_writer_error: InputDiagnosticSummary | None = None
    maintenance: InputMaintenanceHealth | None = None
    query: InputQueryHealth | None = None
    queue_capacity: InputDecimal
    queue_depth: InputDecimal
    queue_full_drops_total: InputDecimal
    queue_high_water_mark: InputDecimal
    sink_statuses: tuple[InputSinkHealth, ...]
    state: InputAvailability
    writer_state: InputWorkerState

@dataclass(frozen=True, kw_only=True)
class InputMaintenanceHealth:
    last_error: InputDiagnosticSummary | None = None
    last_pass_at: str | None = None
    pruned_files_total: InputDecimal
    rotated_files_total: InputDecimal
    state: InputWorkerState

@dataclass(frozen=True, kw_only=True)
class InputMetricRecord:
    attributes: Mapping[str, InputValue]
    name: str
    service: str
    timestamp: str
    unit: str | None = None
    value: InputMetricValue

@dataclass(frozen=True, kw_only=True)
class InputMetricValueGauge:
    data: float
    kind: Literal['gauge'] = dataclass_field(default='gauge', init=False)

@dataclass(frozen=True, kw_only=True)
class InputMetricValueSum:
    data: Mapping[str, NoReturn]
    kind: Literal['sum'] = dataclass_field(default='sum', init=False)

@dataclass(frozen=True, kw_only=True)
class InputMetricValueHistogram:
    data: Mapping[str, NoReturn]
    kind: Literal['histogram'] = dataclass_field(default='histogram', init=False)

@dataclass(frozen=True, kw_only=True)
class InputPathUtf8:
    kind: Literal['utf8'] = dataclass_field(default='utf8', init=False)
    value: str

@dataclass(frozen=True, kw_only=True)
class InputPathUnrepresentable:
    kind: Literal['unrepresentable'] = dataclass_field(default='unrepresentable', init=False)

@dataclass(frozen=True, kw_only=True)
class InputPathAbsent:
    kind: Literal['absent'] = dataclass_field(default='absent', init=False)

@dataclass(frozen=True, kw_only=True)
class InputProcessIdentity:
    hostname: str | None = None
    pid: int | None = None

@dataclass(frozen=True, kw_only=True)
class InputQueryHealth:
    last_error: InputDiagnosticSummary | None = None
    state: InputQueryState

@dataclass(frozen=True, kw_only=True)
class InputQueryRequest:
    query: InputLogQuery
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputRemediationRecoverable:
    kind: Literal['recoverable'] = dataclass_field(default='recoverable', init=False)
    steps: tuple[str, ...]

@dataclass(frozen=True, kw_only=True)
class InputRemediationNotRecoverable:
    justification: str
    kind: Literal['not_recoverable'] = dataclass_field(default='not_recoverable', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResultOk:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputAdmission

@dataclass(frozen=True, kw_only=True)
class InputResultError:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResult2Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputCompletion

@dataclass(frozen=True, kw_only=True)
class InputResult2Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResult3Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputDispatch

@dataclass(frozen=True, kw_only=True)
class InputResult3Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResult4Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputLogSnapshot

@dataclass(frozen=True, kw_only=True)
class InputResult4Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResult5Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputLogHealth

@dataclass(frozen=True, kw_only=True)
class InputResult5Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResult6Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputLevelChange

@dataclass(frozen=True, kw_only=True)
class InputResult6Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResult7Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputClientOutcome

@dataclass(frozen=True, kw_only=True)
class InputResult7Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputResult8Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: InputClientStatus

@dataclass(frozen=True, kw_only=True)
class InputResult8Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class InputSinkHealth:
    last_error: InputDiagnosticSummary | None = None
    name: str
    state: InputAvailability

@dataclass(frozen=True, kw_only=True)
class InputSpanEvent:
    attributes: Mapping[str, InputValue]
    diagnostic: InputStoredDiagnostic | None = None
    name: str
    timestamp: str
    trace: InputTraceContextV2

@dataclass(frozen=True, kw_only=True)
class InputSpanLink:
    attributes: Mapping[str, InputValue]
    flags: int
    span_id: str
    trace_id: str

@dataclass(frozen=True, kw_only=True)
class InputSpanRecord:
    attributes: Mapping[str, InputValue]
    diagnostic: InputStoredDiagnostic | None = None
    duration_ms: InputDecimal | None = None
    kind: InputSpanKind
    links: tuple[InputSpanLink, ...]
    name: str
    service: str
    status: InputSpanStatus
    timestamp: str
    trace: InputTraceContextV2

@dataclass(frozen=True, kw_only=True)
class InputSpanSignal0:
    Started: InputSpanRecord

@dataclass(frozen=True, kw_only=True)
class InputSpanSignal1:
    Event: InputSpanEvent

@dataclass(frozen=True, kw_only=True)
class InputSpanSignal2:
    Ended: InputSpanRecord

@dataclass(frozen=True, kw_only=True)
class InputStateTransition:
    entity_id: str | None = None
    entity_kind: str
    from_state: str
    reason: str | None = None
    to_state: str
    trigger: str | None = None

@dataclass(frozen=True, kw_only=True)
class InputStoredDiagnostic:
    cause: str | None = None
    code: str
    details: Mapping[str, InputValue]
    docs: str | None = None
    message: str
    remediation: InputRemediation
    timestamp: str

@dataclass(frozen=True, kw_only=True)
class InputStoredEvent:
    action: str
    correlation_id: str | None = None
    diagnostic: InputStoredDiagnostic | None = None
    fields: Mapping[str, InputValue]
    identity: InputProcessIdentity
    level: InputLevel
    message: str | None = None
    outcome: str | None = None
    request_id: str | None = None
    service: str
    state_transition: InputStateTransition | None = None
    target: str
    timestamp: str
    trace: InputTraceContext | None = None
    version: str

@dataclass(frozen=True, kw_only=True)
class InputTraceContext:
    parent_span_id: str | None = None
    span_id: str
    trace_id: str

@dataclass(frozen=True, kw_only=True)
class InputTraceContextV2:
    flags: int
    parent_span_id: str | None = None
    span_id: str
    trace_id: str

@dataclass(frozen=True, kw_only=True)
class InputTryLogRequest:
    event: InputLogEvent
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputValueNull:
    kind: Literal['null'] = dataclass_field(default='null', init=False)

@dataclass(frozen=True, kw_only=True)
class InputValueBoolean:
    kind: Literal['boolean'] = dataclass_field(default='boolean', init=False)
    value: bool

@dataclass(frozen=True, kw_only=True)
class InputValueString:
    kind: Literal['string'] = dataclass_field(default='string', init=False)
    value: str

@dataclass(frozen=True, kw_only=True)
class InputValueInteger:
    kind: Literal['integer'] = dataclass_field(default='integer', init=False)
    value: InputDecimal

@dataclass(frozen=True, kw_only=True)
class InputValueFloat:
    kind: Literal['float'] = dataclass_field(default='float', init=False)
    value: float

@dataclass(frozen=True, kw_only=True)
class InputValueArray:
    kind: Literal['array'] = dataclass_field(default='array', init=False)
    value: tuple[InputValue, ...]

@dataclass(frozen=True, kw_only=True)
class InputValueObject:
    kind: Literal['object'] = dataclass_field(default='object', init=False)
    value: Mapping[str, InputValue]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelopeOk:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputAdmission

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelopeError:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope2Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputCompletion

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope2Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope3Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputDispatch

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope3Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope4Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputLogSnapshot

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope4Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope5Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputLogHealth

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope5Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope6Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputLevelChange

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope6Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope7Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputClientOutcome

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope7Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope8Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: InputClientStatus

@dataclass(frozen=True, kw_only=True)
class InputWireEnvelope8Error:
    error: InputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputAdmissionAccepted:
    kind: Literal['accepted'] = dataclass_field(default='accepted', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputAdmissionFiltered:
    kind: Literal['filtered'] = dataclass_field(default='filtered', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputBridgeHealth:
    active_log_path: OutputPath
    configured_level: OutputLevelFilter
    dropped: OutputDropCounts
    effective_level: OutputLevelFilter
    level_revision: OutputDecimal
    lifecycle: OutputLifecycle
    logging: OutputLoggingHealth
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalDiagnostic:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureValidation:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    field: str
    kind: Literal['validation'] = dataclass_field(default='validation', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureQueueFull:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['queue_full'] = dataclass_field(default='queue_full', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureBelowBaseline:
    at: str
    cause: str | None = None
    code: str
    configured: OutputLevelFilter
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['below_baseline'] = dataclass_field(default='below_baseline', init=False)
    message: str
    remediation: OutputRemediation
    requested: OutputLevelFilter

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureUnsupportedLevel:
    at: str
    available: OutputLevelFilter
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unsupported_level'] = dataclass_field(default='unsupported_level', init=False)
    message: str
    remediation: OutputRemediation
    requested: OutputLevelFilter

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailurePermissionDenied:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['permission_denied'] = dataclass_field(default='permission_denied', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureClosed:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['closed'] = dataclass_field(default='closed', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureUnavailable:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unavailable'] = dataclass_field(default='unavailable', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureIo:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['io'] = dataclass_field(default='io', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureTimeout:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['timeout'] = dataclass_field(default='timeout', init=False)
    message: str
    operation: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureCancelled:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['cancelled'] = dataclass_field(default='cancelled', init=False)
    message: str
    operation: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureUnsupportedVersion:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unsupported_version'] = dataclass_field(default='unsupported_version', init=False)
    message: str
    received: int
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureInternal:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['internal'] = dataclass_field(default='internal', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalFailureUnknownRemote:
    at: str
    cause: str | None = None
    code: str
    details: Mapping[str, OutputValue] = dataclass_field(default_factory=lambda: MappingProxyType({}))
    docs: str | None = None
    kind: Literal['unknown_remote'] = dataclass_field(default='unknown_remote', init=False)
    message: str
    remediation: OutputRemediation
    remote_kind: str

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalWireEnvelopeOk:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputAdmission

@dataclass(frozen=True, kw_only=True)
class OutputCanonicalWireEnvelopeError:
    error: OutputCanonicalFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputChangeDiagnosticAccepted:
    kind: Literal['accepted'] = dataclass_field(default='accepted', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputChangeDiagnosticNotAccepted:
    diagnostic: OutputDiagnostic
    kind: Literal['not_accepted'] = dataclass_field(default='not_accepted', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputClientOutcomeIdle:
    kind: Literal['idle'] = dataclass_field(default='idle', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputClientOutcomeScheduled:
    kind: Literal['scheduled'] = dataclass_field(default='scheduled', init=False)
    operation: OutputLogOperation

@dataclass(frozen=True, kw_only=True)
class OutputClientOutcomeAccepted:
    kind: Literal['accepted'] = dataclass_field(default='accepted', init=False)
    operation: OutputAdmissionOperation

@dataclass(frozen=True, kw_only=True)
class OutputClientOutcomeFiltered:
    kind: Literal['filtered'] = dataclass_field(default='filtered', init=False)
    operation: OutputAdmissionOperation

@dataclass(frozen=True, kw_only=True)
class OutputClientOutcomeCompleted:
    kind: Literal['completed'] = dataclass_field(default='completed', init=False)
    operation: OutputCompletionOperation

@dataclass(frozen=True, kw_only=True)
class OutputClientStatus:
    failures_by_kind: OutputFailureCounts
    in_flight: int
    last_failure: OutputFailure | None
    last_result: OutputResult7

@dataclass(frozen=True, kw_only=True)
class OutputCompletionCompleted:
    kind: Literal['completed'] = dataclass_field(default='completed', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputDiagnostic:
    at: str
    code: str
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputDiagnosticSummary:
    at: str
    code: str | None
    message: str

@dataclass(frozen=True, kw_only=True)
class OutputDispatchScheduled:
    kind: Literal['scheduled'] = dataclass_field(default='scheduled', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputDropCounts:
    invalid_event: OutputDecimal
    logger_panicked: OutputDecimal
    not_installed: OutputDecimal
    queue_full: OutputDecimal
    reentrant_emit: OutputDecimal
    shutdown_timed_out: OutputDecimal
    writer_degraded: OutputDecimal

@dataclass(frozen=True, kw_only=True)
class OutputFailureValidation:
    at: str
    code: str
    field: str
    kind: Literal['validation'] = dataclass_field(default='validation', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureQueueFull:
    at: str
    code: str
    kind: Literal['queue_full'] = dataclass_field(default='queue_full', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureBelowBaseline:
    at: str
    code: str
    configured: OutputLevelFilter
    kind: Literal['below_baseline'] = dataclass_field(default='below_baseline', init=False)
    message: str
    remediation: OutputRemediation
    requested: OutputLevelFilter

@dataclass(frozen=True, kw_only=True)
class OutputFailureUnsupportedLevel:
    at: str
    available: OutputLevelFilter
    code: str
    kind: Literal['unsupported_level'] = dataclass_field(default='unsupported_level', init=False)
    message: str
    remediation: OutputRemediation
    requested: OutputLevelFilter

@dataclass(frozen=True, kw_only=True)
class OutputFailurePermissionDenied:
    at: str
    code: str
    kind: Literal['permission_denied'] = dataclass_field(default='permission_denied', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureClosed:
    at: str
    code: str
    kind: Literal['closed'] = dataclass_field(default='closed', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureUnavailable:
    at: str
    code: str
    kind: Literal['unavailable'] = dataclass_field(default='unavailable', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureIo:
    at: str
    code: str
    kind: Literal['io'] = dataclass_field(default='io', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureTimeout:
    at: str
    code: str
    kind: Literal['timeout'] = dataclass_field(default='timeout', init=False)
    message: str
    operation: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureCancelled:
    at: str
    code: str
    kind: Literal['cancelled'] = dataclass_field(default='cancelled', init=False)
    message: str
    operation: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureUnsupportedVersion:
    at: str
    code: str
    kind: Literal['unsupported_version'] = dataclass_field(default='unsupported_version', init=False)
    message: str
    received: int
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureInternal:
    at: str
    code: str
    kind: Literal['internal'] = dataclass_field(default='internal', init=False)
    message: str
    remediation: OutputRemediation

@dataclass(frozen=True, kw_only=True)
class OutputFailureUnknownRemote:
    at: str
    code: str
    kind: Literal['unknown_remote'] = dataclass_field(default='unknown_remote', init=False)
    message: str
    remediation: OutputRemediation
    remote_kind: str

@dataclass(frozen=True, kw_only=True)
class OutputFailureCounts:
    below_baseline: OutputDecimal
    cancelled: OutputDecimal
    closed: OutputDecimal
    internal: OutputDecimal
    io: OutputDecimal
    permission_denied: OutputDecimal
    queue_full: OutputDecimal
    timeout: OutputDecimal
    unavailable: OutputDecimal
    unknown_remote: OutputDecimal
    unsupported_level: OutputDecimal
    unsupported_version: OutputDecimal
    validation: OutputDecimal

@dataclass(frozen=True, kw_only=True)
class OutputFieldMatch:
    field: str
    value: OutputValue

@dataclass(frozen=True, kw_only=True)
class OutputFlushRequest:
    schema_version: Literal[1]
    timeout_ms: int

@dataclass(frozen=True, kw_only=True)
class OutputHealthRequest:
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputHistogramPoint:
    bucket_counts: tuple[OutputDecimal, ...]
    count: OutputDecimal
    explicit_bounds: tuple[float, ...]
    sum: float

@dataclass(frozen=True, kw_only=True)
class OutputLevelChangeChanged:
    current: OutputLevelState
    diagnostic: OutputChangeDiagnostic
    kind: Literal['changed'] = dataclass_field(default='changed', init=False)
    previous: OutputLevelState
    source: OutputLevelChangeSource

@dataclass(frozen=True, kw_only=True)
class OutputLevelChangeUnchanged:
    kind: Literal['unchanged'] = dataclass_field(default='unchanged', init=False)
    state: OutputLevelState

@dataclass(frozen=True, kw_only=True)
class OutputLevelChangeRequest:
    change: OutputLevelRequest
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputLevelRequestElevate:
    kind: Literal['elevate'] = dataclass_field(default='elevate', init=False)
    level: OutputLevelFilter

@dataclass(frozen=True, kw_only=True)
class OutputLevelRequestReset:
    kind: Literal['reset'] = dataclass_field(default='reset', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputLevelState:
    configured_level: OutputLevelFilter
    effective_level: OutputLevelFilter
    level_revision: OutputDecimal

@dataclass(frozen=True, kw_only=True)
class OutputLogEvent:
    action: str
    correlation_id: str | None
    fields: Mapping[str, OutputValue]
    level: OutputLevel
    message: str | None
    outcome: str | None
    request_id: str | None
    schema_version: Literal[1]
    target: str
    trace: OutputTraceContext | None

@dataclass(frozen=True, kw_only=True)
class OutputLogHealth:
    bridge: OutputBridgeHealth | None
    level_state: OutputLevelState
    logging: OutputLoggingHealth
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputLogQuery:
    action: str | None
    correlation_id: str | None
    field_matches: tuple[OutputFieldMatch, ...]
    levels: tuple[OutputLevel, ...]
    limit: int
    order: OutputLogOrder
    request_id: str | None
    schema_version: Literal[1]
    service: str | None
    since: str | None
    target: str | None
    until: str | None

@dataclass(frozen=True, kw_only=True)
class OutputLogSnapshot:
    events: tuple[OutputStoredEvent, ...]
    schema_version: Literal[1]
    truncated: bool

@dataclass(frozen=True, kw_only=True)
class OutputLoggingHealth:
    active_log_path: OutputPath
    dropped_events_total: OutputDecimal
    flush_errors_total: OutputDecimal
    last_error: OutputDiagnosticSummary | None
    last_writer_error: OutputDiagnosticSummary | None
    maintenance: OutputMaintenanceHealth | None
    query: OutputQueryHealth | None
    queue_capacity: OutputDecimal
    queue_depth: OutputDecimal
    queue_full_drops_total: OutputDecimal
    queue_high_water_mark: OutputDecimal
    sink_statuses: tuple[OutputSinkHealth, ...]
    state: OutputAvailability
    writer_state: OutputWorkerState

@dataclass(frozen=True, kw_only=True)
class OutputMaintenanceHealth:
    last_error: OutputDiagnosticSummary | None
    last_pass_at: str | None
    pruned_files_total: OutputDecimal
    rotated_files_total: OutputDecimal
    state: OutputWorkerState

@dataclass(frozen=True, kw_only=True)
class OutputMetricRecord:
    attributes: Mapping[str, OutputValue]
    name: str
    service: str
    timestamp: str
    unit: str | None
    value: OutputMetricValue

@dataclass(frozen=True, kw_only=True)
class OutputMetricValueGauge:
    data: float
    kind: Literal['gauge'] = dataclass_field(default='gauge', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputMetricValueSum:
    data: Mapping[str, NoReturn]
    kind: Literal['sum'] = dataclass_field(default='sum', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputMetricValueHistogram:
    data: Mapping[str, NoReturn]
    kind: Literal['histogram'] = dataclass_field(default='histogram', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputPathUtf8:
    kind: Literal['utf8'] = dataclass_field(default='utf8', init=False)
    value: str

@dataclass(frozen=True, kw_only=True)
class OutputPathUnrepresentable:
    kind: Literal['unrepresentable'] = dataclass_field(default='unrepresentable', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputPathAbsent:
    kind: Literal['absent'] = dataclass_field(default='absent', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputProcessIdentity:
    hostname: str | None
    pid: int | None

@dataclass(frozen=True, kw_only=True)
class OutputQueryHealth:
    last_error: OutputDiagnosticSummary | None
    state: OutputQueryState

@dataclass(frozen=True, kw_only=True)
class OutputQueryRequest:
    query: OutputLogQuery
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputRemediationRecoverable:
    kind: Literal['recoverable'] = dataclass_field(default='recoverable', init=False)
    steps: tuple[str, ...]

@dataclass(frozen=True, kw_only=True)
class OutputRemediationNotRecoverable:
    justification: str
    kind: Literal['not_recoverable'] = dataclass_field(default='not_recoverable', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResultOk:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputAdmission

@dataclass(frozen=True, kw_only=True)
class OutputResultError:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResult2Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputCompletion

@dataclass(frozen=True, kw_only=True)
class OutputResult2Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResult3Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputDispatch

@dataclass(frozen=True, kw_only=True)
class OutputResult3Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResult4Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputLogSnapshot

@dataclass(frozen=True, kw_only=True)
class OutputResult4Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResult5Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputLogHealth

@dataclass(frozen=True, kw_only=True)
class OutputResult5Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResult6Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputLevelChange

@dataclass(frozen=True, kw_only=True)
class OutputResult6Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResult7Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputClientOutcome

@dataclass(frozen=True, kw_only=True)
class OutputResult7Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputResult8Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    value: OutputClientStatus

@dataclass(frozen=True, kw_only=True)
class OutputResult8Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputSinkHealth:
    last_error: OutputDiagnosticSummary | None
    name: str
    state: OutputAvailability

@dataclass(frozen=True, kw_only=True)
class OutputSpanEvent:
    attributes: Mapping[str, OutputValue]
    diagnostic: OutputStoredDiagnostic | None
    name: str
    timestamp: str
    trace: OutputTraceContextV2

@dataclass(frozen=True, kw_only=True)
class OutputSpanLink:
    attributes: Mapping[str, OutputValue]
    flags: int
    span_id: str
    trace_id: str

@dataclass(frozen=True, kw_only=True)
class OutputSpanRecord:
    attributes: Mapping[str, OutputValue]
    diagnostic: OutputStoredDiagnostic | None
    duration_ms: OutputDecimal | None
    kind: OutputSpanKind
    links: tuple[OutputSpanLink, ...]
    name: str
    service: str
    status: OutputSpanStatus
    timestamp: str
    trace: OutputTraceContextV2

@dataclass(frozen=True, kw_only=True)
class OutputSpanSignal0:
    Started: OutputSpanRecord

@dataclass(frozen=True, kw_only=True)
class OutputSpanSignal1:
    Event: OutputSpanEvent

@dataclass(frozen=True, kw_only=True)
class OutputSpanSignal2:
    Ended: OutputSpanRecord

@dataclass(frozen=True, kw_only=True)
class OutputStateTransition:
    entity_id: str | None
    entity_kind: str
    from_state: str
    reason: str | None
    to_state: str
    trigger: str | None

@dataclass(frozen=True, kw_only=True)
class OutputStoredDiagnostic:
    cause: str | None
    code: str
    details: Mapping[str, OutputValue]
    docs: str | None
    message: str
    remediation: OutputRemediation
    timestamp: str

@dataclass(frozen=True, kw_only=True)
class OutputStoredEvent:
    action: str
    correlation_id: str | None
    diagnostic: OutputStoredDiagnostic | None
    fields: Mapping[str, OutputValue]
    identity: OutputProcessIdentity
    level: OutputLevel
    message: str | None
    outcome: str | None
    request_id: str | None
    service: str
    state_transition: OutputStateTransition | None
    target: str
    timestamp: str
    trace: OutputTraceContext | None
    version: str

@dataclass(frozen=True, kw_only=True)
class OutputTraceContext:
    parent_span_id: str | None
    span_id: str
    trace_id: str

@dataclass(frozen=True, kw_only=True)
class OutputTraceContextV2:
    flags: int
    parent_span_id: str | None
    span_id: str
    trace_id: str

@dataclass(frozen=True, kw_only=True)
class OutputTryLogRequest:
    event: OutputLogEvent
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputValueNull:
    kind: Literal['null'] = dataclass_field(default='null', init=False)

@dataclass(frozen=True, kw_only=True)
class OutputValueBoolean:
    kind: Literal['boolean'] = dataclass_field(default='boolean', init=False)
    value: bool

@dataclass(frozen=True, kw_only=True)
class OutputValueString:
    kind: Literal['string'] = dataclass_field(default='string', init=False)
    value: str

@dataclass(frozen=True, kw_only=True)
class OutputValueInteger:
    kind: Literal['integer'] = dataclass_field(default='integer', init=False)
    value: OutputDecimal

@dataclass(frozen=True, kw_only=True)
class OutputValueFloat:
    kind: Literal['float'] = dataclass_field(default='float', init=False)
    value: float

@dataclass(frozen=True, kw_only=True)
class OutputValueArray:
    kind: Literal['array'] = dataclass_field(default='array', init=False)
    value: tuple[OutputValue, ...]

@dataclass(frozen=True, kw_only=True)
class OutputValueObject:
    kind: Literal['object'] = dataclass_field(default='object', init=False)
    value: Mapping[str, OutputValue]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelopeOk:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputAdmission

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelopeError:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope2Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputCompletion

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope2Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope3Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputDispatch

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope3Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope4Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputLogSnapshot

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope4Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope5Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputLogHealth

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope5Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope6Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputLevelChange

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope6Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope7Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputClientOutcome

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope7Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope8Ok:
    kind: Literal['ok'] = dataclass_field(default='ok', init=False)
    schema_version: Literal[1]
    value: OutputClientStatus

@dataclass(frozen=True, kw_only=True)
class OutputWireEnvelope8Error:
    error: OutputFailure
    kind: Literal['error'] = dataclass_field(default='error', init=False)
    schema_version: Literal[1]

InputAdmission: TypeAlias = InputAdmissionAccepted | InputAdmissionFiltered
InputAdmissionOperation: TypeAlias = Literal['log'] | Literal['try_log']
InputAggregationTemporality: TypeAlias = Literal['delta'] | Literal['cumulative']
InputAvailability: TypeAlias = Literal['healthy'] | Literal['degraded_dropping'] | Literal['unavailable']
InputCanonicalFailure: TypeAlias = InputCanonicalFailureValidation | InputCanonicalFailureQueueFull | InputCanonicalFailureBelowBaseline | InputCanonicalFailureUnsupportedLevel | InputCanonicalFailurePermissionDenied | InputCanonicalFailureClosed | InputCanonicalFailureUnavailable | InputCanonicalFailureIo | InputCanonicalFailureTimeout | InputCanonicalFailureCancelled | InputCanonicalFailureUnsupportedVersion | InputCanonicalFailureInternal | InputCanonicalFailureUnknownRemote
InputCanonicalWireEnvelope: TypeAlias = InputCanonicalWireEnvelopeOk | InputCanonicalWireEnvelopeError
InputChangeDiagnostic: TypeAlias = InputChangeDiagnosticAccepted | InputChangeDiagnosticNotAccepted
InputClientOutcome: TypeAlias = InputClientOutcomeIdle | InputClientOutcomeScheduled | InputClientOutcomeAccepted | InputClientOutcomeFiltered | InputClientOutcomeCompleted
InputCompletion: TypeAlias = InputCompletionCompleted
InputCompletionOperation: TypeAlias = Literal['query'] | Literal['health'] | Literal['flush']
InputDecimal: TypeAlias = int
InputDispatch: TypeAlias = InputDispatchScheduled
InputFailure: TypeAlias = InputFailureValidation | InputFailureQueueFull | InputFailureBelowBaseline | InputFailureUnsupportedLevel | InputFailurePermissionDenied | InputFailureClosed | InputFailureUnavailable | InputFailureIo | InputFailureTimeout | InputFailureCancelled | InputFailureUnsupportedVersion | InputFailureInternal | InputFailureUnknownRemote
InputLevelChange: TypeAlias = InputLevelChangeChanged | InputLevelChangeUnchanged
InputLevelChangeSource: TypeAlias = Literal['application'] | Literal['user_request'] | Literal['diagnostic_session']
InputLevel: TypeAlias = Literal['trace'] | Literal['debug'] | Literal['info'] | Literal['warn'] | Literal['error']
InputLevelFilter: TypeAlias = Literal['off'] | Literal['error'] | Literal['warn'] | Literal['info'] | Literal['debug'] | Literal['trace']
InputLevelRequest: TypeAlias = InputLevelRequestElevate | InputLevelRequestReset
InputLifecycle: TypeAlias = Literal['running'] | Literal['stopping'] | Literal['stopped'] | Literal['failed']
InputLogOperation: TypeAlias = Literal['log']
InputLogOrder: TypeAlias = Literal['oldest_first'] | Literal['newest_first']
InputMetricValue: TypeAlias = InputMetricValueGauge | InputMetricValueSum | InputMetricValueHistogram
InputPath: TypeAlias = InputPathUtf8 | InputPathUnrepresentable | InputPathAbsent
InputQueryState: TypeAlias = Literal['healthy'] | Literal['degraded'] | Literal['unavailable']
InputRemediation: TypeAlias = InputRemediationRecoverable | InputRemediationNotRecoverable
InputResult: TypeAlias = InputResultOk | InputResultError
InputResult2: TypeAlias = InputResult2Ok | InputResult2Error
InputResult3: TypeAlias = InputResult3Ok | InputResult3Error
InputResult4: TypeAlias = InputResult4Ok | InputResult4Error
InputResult5: TypeAlias = InputResult5Ok | InputResult5Error
InputResult6: TypeAlias = InputResult6Ok | InputResult6Error
InputResult7: TypeAlias = InputResult7Ok | InputResult7Error
InputResult8: TypeAlias = InputResult8Ok | InputResult8Error
InputSpanKind: TypeAlias = Literal['internal'] | Literal['server'] | Literal['client'] | Literal['producer'] | Literal['consumer']
InputSpanSignal: TypeAlias = InputSpanSignal0 | InputSpanSignal1 | InputSpanSignal2
InputSpanStatus: TypeAlias = Literal['Ok'] | Literal['Error'] | Literal['Unset']
InputValue: TypeAlias = InputValueNull | InputValueBoolean | InputValueString | InputValueInteger | InputValueFloat | InputValueArray | InputValueObject
InputWireEnvelope: TypeAlias = InputWireEnvelopeOk | InputWireEnvelopeError
InputWireEnvelope2: TypeAlias = InputWireEnvelope2Ok | InputWireEnvelope2Error
InputWireEnvelope3: TypeAlias = InputWireEnvelope3Ok | InputWireEnvelope3Error
InputWireEnvelope4: TypeAlias = InputWireEnvelope4Ok | InputWireEnvelope4Error
InputWireEnvelope5: TypeAlias = InputWireEnvelope5Ok | InputWireEnvelope5Error
InputWireEnvelope6: TypeAlias = InputWireEnvelope6Ok | InputWireEnvelope6Error
InputWireEnvelope7: TypeAlias = InputWireEnvelope7Ok | InputWireEnvelope7Error
InputWireEnvelope8: TypeAlias = InputWireEnvelope8Ok | InputWireEnvelope8Error
InputWorkerState: TypeAlias = Literal['running'] | Literal['degraded'] | Literal['stopped']
OutputAdmission: TypeAlias = OutputAdmissionAccepted | OutputAdmissionFiltered
OutputAdmissionOperation: TypeAlias = Literal['log'] | Literal['try_log']
OutputAggregationTemporality: TypeAlias = Literal['delta'] | Literal['cumulative']
OutputAvailability: TypeAlias = Literal['healthy'] | Literal['degraded_dropping'] | Literal['unavailable']
OutputCanonicalFailure: TypeAlias = OutputCanonicalFailureValidation | OutputCanonicalFailureQueueFull | OutputCanonicalFailureBelowBaseline | OutputCanonicalFailureUnsupportedLevel | OutputCanonicalFailurePermissionDenied | OutputCanonicalFailureClosed | OutputCanonicalFailureUnavailable | OutputCanonicalFailureIo | OutputCanonicalFailureTimeout | OutputCanonicalFailureCancelled | OutputCanonicalFailureUnsupportedVersion | OutputCanonicalFailureInternal | OutputCanonicalFailureUnknownRemote
OutputCanonicalWireEnvelope: TypeAlias = OutputCanonicalWireEnvelopeOk | OutputCanonicalWireEnvelopeError
OutputChangeDiagnostic: TypeAlias = OutputChangeDiagnosticAccepted | OutputChangeDiagnosticNotAccepted
OutputClientOutcome: TypeAlias = OutputClientOutcomeIdle | OutputClientOutcomeScheduled | OutputClientOutcomeAccepted | OutputClientOutcomeFiltered | OutputClientOutcomeCompleted
OutputCompletion: TypeAlias = OutputCompletionCompleted
OutputCompletionOperation: TypeAlias = Literal['query'] | Literal['health'] | Literal['flush']
OutputDecimal: TypeAlias = int
OutputDispatch: TypeAlias = OutputDispatchScheduled
OutputFailure: TypeAlias = OutputFailureValidation | OutputFailureQueueFull | OutputFailureBelowBaseline | OutputFailureUnsupportedLevel | OutputFailurePermissionDenied | OutputFailureClosed | OutputFailureUnavailable | OutputFailureIo | OutputFailureTimeout | OutputFailureCancelled | OutputFailureUnsupportedVersion | OutputFailureInternal | OutputFailureUnknownRemote
OutputLevelChange: TypeAlias = OutputLevelChangeChanged | OutputLevelChangeUnchanged
OutputLevelChangeSource: TypeAlias = Literal['application'] | Literal['user_request'] | Literal['diagnostic_session']
OutputLevel: TypeAlias = Literal['trace'] | Literal['debug'] | Literal['info'] | Literal['warn'] | Literal['error']
OutputLevelFilter: TypeAlias = Literal['off'] | Literal['error'] | Literal['warn'] | Literal['info'] | Literal['debug'] | Literal['trace']
OutputLevelRequest: TypeAlias = OutputLevelRequestElevate | OutputLevelRequestReset
OutputLifecycle: TypeAlias = Literal['running'] | Literal['stopping'] | Literal['stopped'] | Literal['failed']
OutputLogOperation: TypeAlias = Literal['log']
OutputLogOrder: TypeAlias = Literal['oldest_first'] | Literal['newest_first']
OutputMetricValue: TypeAlias = OutputMetricValueGauge | OutputMetricValueSum | OutputMetricValueHistogram
OutputPath: TypeAlias = OutputPathUtf8 | OutputPathUnrepresentable | OutputPathAbsent
OutputQueryState: TypeAlias = Literal['healthy'] | Literal['degraded'] | Literal['unavailable']
OutputRemediation: TypeAlias = OutputRemediationRecoverable | OutputRemediationNotRecoverable
OutputResult: TypeAlias = OutputResultOk | OutputResultError
OutputResult2: TypeAlias = OutputResult2Ok | OutputResult2Error
OutputResult3: TypeAlias = OutputResult3Ok | OutputResult3Error
OutputResult4: TypeAlias = OutputResult4Ok | OutputResult4Error
OutputResult5: TypeAlias = OutputResult5Ok | OutputResult5Error
OutputResult6: TypeAlias = OutputResult6Ok | OutputResult6Error
OutputResult7: TypeAlias = OutputResult7Ok | OutputResult7Error
OutputResult8: TypeAlias = OutputResult8Ok | OutputResult8Error
OutputSpanKind: TypeAlias = Literal['internal'] | Literal['server'] | Literal['client'] | Literal['producer'] | Literal['consumer']
OutputSpanSignal: TypeAlias = OutputSpanSignal0 | OutputSpanSignal1 | OutputSpanSignal2
OutputSpanStatus: TypeAlias = Literal['Ok'] | Literal['Error'] | Literal['Unset']
OutputValue: TypeAlias = OutputValueNull | OutputValueBoolean | OutputValueString | OutputValueInteger | OutputValueFloat | OutputValueArray | OutputValueObject
OutputWireEnvelope: TypeAlias = OutputWireEnvelopeOk | OutputWireEnvelopeError
OutputWireEnvelope2: TypeAlias = OutputWireEnvelope2Ok | OutputWireEnvelope2Error
OutputWireEnvelope3: TypeAlias = OutputWireEnvelope3Ok | OutputWireEnvelope3Error
OutputWireEnvelope4: TypeAlias = OutputWireEnvelope4Ok | OutputWireEnvelope4Error
OutputWireEnvelope5: TypeAlias = OutputWireEnvelope5Ok | OutputWireEnvelope5Error
OutputWireEnvelope6: TypeAlias = OutputWireEnvelope6Ok | OutputWireEnvelope6Error
OutputWireEnvelope7: TypeAlias = OutputWireEnvelope7Ok | OutputWireEnvelope7Error
OutputWireEnvelope8: TypeAlias = OutputWireEnvelope8Ok | OutputWireEnvelope8Error
OutputWorkerState: TypeAlias = Literal['running'] | Literal['degraded'] | Literal['stopped']
InputCanonicalWireEnvelopeAdmission: TypeAlias = InputCanonicalWireEnvelope
InputOperationDiagnostic: TypeAlias = InputDiagnostic
InputResultAdmission: TypeAlias = InputResult
InputResultClientOutcome: TypeAlias = InputResult7
InputResultClientStatus: TypeAlias = InputResult8
InputResultCompletion: TypeAlias = InputResult2
InputResultDispatch: TypeAlias = InputResult3
InputResultLevelChange: TypeAlias = InputResult6
InputResultLogHealth: TypeAlias = InputResult5
InputResultLogSnapshot: TypeAlias = InputResult4
InputWireEnvelopeAdmission: TypeAlias = InputWireEnvelope
InputWireEnvelopeClientOutcome: TypeAlias = InputWireEnvelope7
InputWireEnvelopeClientStatus: TypeAlias = InputWireEnvelope8
InputWireEnvelopeCompletion: TypeAlias = InputWireEnvelope2
InputWireEnvelopeDispatch: TypeAlias = InputWireEnvelope3
InputWireEnvelopeLevelChange: TypeAlias = InputWireEnvelope6
InputWireEnvelopeLogHealth: TypeAlias = InputWireEnvelope5
InputWireEnvelopeLogSnapshot: TypeAlias = InputWireEnvelope4
Admission: TypeAlias = OutputAdmission
AdmissionOperation: TypeAlias = OutputAdmissionOperation
AggregationTemporality: TypeAlias = OutputAggregationTemporality
Availability: TypeAlias = OutputAvailability
BridgeHealth: TypeAlias = OutputBridgeHealth
CanonicalDiagnostic: TypeAlias = OutputCanonicalDiagnostic
CanonicalFailure: TypeAlias = OutputCanonicalFailure
OutputCanonicalWireEnvelopeAdmission: TypeAlias = OutputCanonicalWireEnvelope
CanonicalWireEnvelopeAdmission: TypeAlias = OutputCanonicalWireEnvelopeAdmission
ChangeDiagnostic: TypeAlias = OutputChangeDiagnostic
ClientOutcome: TypeAlias = OutputClientOutcome
ClientStatus: TypeAlias = OutputClientStatus
Completion: TypeAlias = OutputCompletion
CompletionOperation: TypeAlias = OutputCompletionOperation
Decimal: TypeAlias = OutputDecimal
Diagnostic: TypeAlias = OutputDiagnostic
DiagnosticSummary: TypeAlias = OutputDiagnosticSummary
Dispatch: TypeAlias = OutputDispatch
DropCounts: TypeAlias = OutputDropCounts
Failure: TypeAlias = OutputFailure
FailureCounts: TypeAlias = OutputFailureCounts
FieldMatch: TypeAlias = OutputFieldMatch
FlushRequest: TypeAlias = OutputFlushRequest
HealthRequest: TypeAlias = OutputHealthRequest
HistogramPoint: TypeAlias = OutputHistogramPoint
LevelChange: TypeAlias = OutputLevelChange
LevelChangeRequest: TypeAlias = OutputLevelChangeRequest
LevelChangeSource: TypeAlias = OutputLevelChangeSource
Level: TypeAlias = OutputLevel
LevelFilter: TypeAlias = OutputLevelFilter
LevelRequest: TypeAlias = OutputLevelRequest
LevelState: TypeAlias = OutputLevelState
Lifecycle: TypeAlias = OutputLifecycle
LogEvent: TypeAlias = OutputLogEvent
LogHealth: TypeAlias = OutputLogHealth
LogOperation: TypeAlias = OutputLogOperation
LogOrder: TypeAlias = OutputLogOrder
LogQuery: TypeAlias = OutputLogQuery
LogSnapshot: TypeAlias = OutputLogSnapshot
LoggingHealth: TypeAlias = OutputLoggingHealth
MaintenanceHealth: TypeAlias = OutputMaintenanceHealth
MetricRecord: TypeAlias = OutputMetricRecord
MetricValue: TypeAlias = OutputMetricValue
OutputOperationDiagnostic: TypeAlias = OutputDiagnostic
OperationDiagnostic: TypeAlias = OutputOperationDiagnostic
Path: TypeAlias = OutputPath
ProcessIdentity: TypeAlias = OutputProcessIdentity
QueryHealth: TypeAlias = OutputQueryHealth
QueryRequest: TypeAlias = OutputQueryRequest
QueryState: TypeAlias = OutputQueryState
Remediation: TypeAlias = OutputRemediation
OutputResultAdmission: TypeAlias = OutputResult
ResultAdmission: TypeAlias = OutputResultAdmission
OutputResultClientOutcome: TypeAlias = OutputResult7
ResultClientOutcome: TypeAlias = OutputResultClientOutcome
OutputResultClientStatus: TypeAlias = OutputResult8
ResultClientStatus: TypeAlias = OutputResultClientStatus
OutputResultCompletion: TypeAlias = OutputResult2
ResultCompletion: TypeAlias = OutputResultCompletion
OutputResultDispatch: TypeAlias = OutputResult3
ResultDispatch: TypeAlias = OutputResultDispatch
OutputResultLevelChange: TypeAlias = OutputResult6
ResultLevelChange: TypeAlias = OutputResultLevelChange
OutputResultLogHealth: TypeAlias = OutputResult5
ResultLogHealth: TypeAlias = OutputResultLogHealth
OutputResultLogSnapshot: TypeAlias = OutputResult4
ResultLogSnapshot: TypeAlias = OutputResultLogSnapshot
SinkHealth: TypeAlias = OutputSinkHealth
SpanEvent: TypeAlias = OutputSpanEvent
SpanKind: TypeAlias = OutputSpanKind
SpanLink: TypeAlias = OutputSpanLink
SpanRecord: TypeAlias = OutputSpanRecord
SpanSignal: TypeAlias = OutputSpanSignal
SpanStatus: TypeAlias = OutputSpanStatus
StateTransition: TypeAlias = OutputStateTransition
StoredDiagnostic: TypeAlias = OutputStoredDiagnostic
StoredEvent: TypeAlias = OutputStoredEvent
TraceContext: TypeAlias = OutputTraceContext
TraceContextV2: TypeAlias = OutputTraceContextV2
TryLogRequest: TypeAlias = OutputTryLogRequest
Value: TypeAlias = OutputValue
OutputWireEnvelopeAdmission: TypeAlias = OutputWireEnvelope
WireEnvelopeAdmission: TypeAlias = OutputWireEnvelopeAdmission
OutputWireEnvelopeClientOutcome: TypeAlias = OutputWireEnvelope7
WireEnvelopeClientOutcome: TypeAlias = OutputWireEnvelopeClientOutcome
OutputWireEnvelopeClientStatus: TypeAlias = OutputWireEnvelope8
WireEnvelopeClientStatus: TypeAlias = OutputWireEnvelopeClientStatus
OutputWireEnvelopeCompletion: TypeAlias = OutputWireEnvelope2
WireEnvelopeCompletion: TypeAlias = OutputWireEnvelopeCompletion
OutputWireEnvelopeDispatch: TypeAlias = OutputWireEnvelope3
WireEnvelopeDispatch: TypeAlias = OutputWireEnvelopeDispatch
OutputWireEnvelopeLevelChange: TypeAlias = OutputWireEnvelope6
WireEnvelopeLevelChange: TypeAlias = OutputWireEnvelopeLevelChange
OutputWireEnvelopeLogHealth: TypeAlias = OutputWireEnvelope5
WireEnvelopeLogHealth: TypeAlias = OutputWireEnvelopeLogHealth
OutputWireEnvelopeLogSnapshot: TypeAlias = OutputWireEnvelope4
WireEnvelopeLogSnapshot: TypeAlias = OutputWireEnvelopeLogSnapshot
WorkerState: TypeAlias = OutputWorkerState

SCHEMA = json.loads('{"$defs":{"InputAdmissionDto":{"description":"Version-one AdmissionDto wire value.","oneOf":[{"description":"Wire accepted.","properties":{"kind":{"const":"accepted","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire filtered.","properties":{"kind":{"const":"filtered","type":"string"}},"required":["kind"],"type":"object"}]},"InputAdmissionOperationDto":{"description":"Operations returning final logging admission.","oneOf":[{"const":"log","description":"Wire log.","type":"string"},{"const":"try_log","description":"Wire try log.","type":"string"}]},"InputAggregationTemporalityDto":{"description":"Staged aggregation interval interpretation.","oneOf":[{"const":"delta","description":"Nonempty collection interval.","type":"string"},{"const":"cumulative","description":"Sequence-start interval.","type":"string"}]},"InputAvailabilityDto":{"description":"Version-one AvailabilityDto wire value.","oneOf":[{"const":"healthy","description":"Wire healthy.","type":"string"},{"const":"degraded_dropping","description":"Wire degraded dropping.","type":"string"},{"const":"unavailable","description":"Wire unavailable.","type":"string"}]},"InputBridgeHealthDto":{"description":"Version-one BridgeHealthDto wire record.","properties":{"active_log_path":{"$ref":"#/$defs/InputPathDto","description":"active log path."},"configured_level":{"$ref":"#/$defs/InputLevelFilterDto","description":"configured level."},"dropped":{"$ref":"#/$defs/InputDropCountsDto","description":"dropped."},"effective_level":{"$ref":"#/$defs/InputLevelFilterDto","description":"effective level."},"level_revision":{"$ref":"#/$defs/InputDecimalDto","description":"level revision.\\nWire level revision.","x-sc-integer-domain":"unsigned"},"lifecycle":{"$ref":"#/$defs/InputLifecycleDto","description":"lifecycle."},"logging":{"$ref":"#/$defs/InputLoggingHealthDto","description":"logging."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","logging","dropped","lifecycle","active_log_path","configured_level","effective_level","level_revision"],"type":"object"},"InputCanonicalDiagnosticDto":{"description":"Additive canonical diagnostic projection; source objects remain native.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["at","code","message","remediation"],"type":"object"},"InputCanonicalFailureDto":{"description":"Additive operational failures preserving canonical diagnostic metadata.","oneOf":[{"description":"Wire validation.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"field":{"description":"Wire field.","type":"string"},"kind":{"const":"validation","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","field"],"type":"object"},{"description":"Wire queue full.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"queue_full","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire below baseline.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"configured":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire configured."},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"below_baseline","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","configured"],"type":"object"},{"description":"Wire unsupported level.","properties":{"at":{"description":"at.","type":"string"},"available":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire available."},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unsupported_level","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","available"],"type":"object"},{"description":"Wire permission denied.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"permission_denied","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire closed.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"closed","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unavailable.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unavailable","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire io.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"io","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire timeout.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"timeout","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire cancelled.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"cancelled","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire unsupported version.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unsupported_version","type":"string"},"message":{"description":"message.","type":"string"},"received":{"description":"Wire received.","format":"uint32","minimum":0,"type":"integer"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","received"],"type":"object"},{"description":"Wire internal.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"internal","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unknown remote.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unknown_remote","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."},"remote_kind":{"description":"Wire remote kind.","type":"string"}},"required":["kind","at","code","message","remediation","remote_kind"],"type":"object"}]},"InputCanonicalWireEnvelope":{"description":"Staged envelope with unchanged schema/version/discriminants and richer diagnostics.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputAdmissionDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputCanonicalFailureDto","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputChangeDiagnosticDto":{"description":"Version-one ChangeDiagnosticDto wire value.","oneOf":[{"description":"Wire accepted.","properties":{"kind":{"const":"accepted","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire not accepted.","properties":{"diagnostic":{"$ref":"#/$defs/InputDiagnostic","description":"Active variant diagnostic."},"kind":{"const":"not_accepted","type":"string"}},"required":["kind","diagnostic"],"type":"object"}]},"InputClientOutcome":{"description":"Payload-free client outcome, distinct from host persistence.","oneOf":[{"description":"Wire idle.","properties":{"kind":{"const":"idle","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire scheduled.","properties":{"kind":{"const":"scheduled","type":"string"},"operation":{"$ref":"#/$defs/InputLogOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"},{"description":"Wire accepted.","properties":{"kind":{"const":"accepted","type":"string"},"operation":{"$ref":"#/$defs/InputAdmissionOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"},{"description":"Wire filtered.","properties":{"kind":{"const":"filtered","type":"string"},"operation":{"$ref":"#/$defs/InputAdmissionOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"},{"description":"Wire completed.","properties":{"kind":{"const":"completed","type":"string"},"operation":{"$ref":"#/$defs/InputCompletionOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"}]},"InputClientStatus":{"description":"Bounded local client status; no ownership or IPC capability is represented.","properties":{"failures_by_kind":{"$ref":"#/$defs/InputFailureCountsDto","description":"Saturating counters for every declared failure kind."},"in_flight":{"description":"Number of outstanding operations, bounded by client admission.\\nWire in flight.","format":"uint32","maximum":256,"minimum":0,"type":"integer"},"last_failure":{"anyOf":[{"$ref":"#/$defs/InputFailure"},{"type":"null"}],"description":"Retained failure survives subsequent successful operations."},"last_result":{"$ref":"#/$defs/InputResultDto7","description":"Most recent completion-order result."}},"required":["in_flight","failures_by_kind","last_result"],"type":"object"},"InputCompletionDto":{"description":"Version-one CompletionDto wire value.","oneOf":[{"description":"Wire completed.","properties":{"kind":{"const":"completed","type":"string"}},"required":["kind"],"type":"object"}]},"InputCompletionOperationDto":{"description":"Operations returning a completed client observation.","oneOf":[{"const":"query","description":"Wire query.","type":"string"},{"const":"health","description":"Wire health.","type":"string"},{"const":"flush","description":"Wire flush.","type":"string"}]},"InputDecimalDto":{"description":"Canonical integer string: signed i64 or unsigned u64; counters additionally reject negatives.","pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)(?![\\\\s\\\\S])","type":"string"},"InputDiagnostic":{"description":"Version-one Diagnostic wire record.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["at","code","message","remediation"],"type":"object"},"InputDiagnosticSummaryDto":{"description":"Version-one DiagnosticSummaryDto wire record.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":["string","null"]},"message":{"description":"message.","type":"string"}},"required":["message","at"],"type":"object"},"InputDispatchDto":{"description":"Version-one DispatchDto wire value.","oneOf":[{"description":"Wire scheduled.","properties":{"kind":{"const":"scheduled","type":"string"}},"required":["kind"],"type":"object"}]},"InputDropCountsDto":{"description":"Version-one DropCountsDto wire record.","properties":{"invalid_event":{"$ref":"#/$defs/InputDecimalDto","description":"invalid event.\\nWire invalid event.","x-sc-integer-domain":"unsigned"},"logger_panicked":{"$ref":"#/$defs/InputDecimalDto","description":"logger panicked.\\nWire logger panicked.","x-sc-integer-domain":"unsigned"},"not_installed":{"$ref":"#/$defs/InputDecimalDto","description":"not installed.\\nWire not installed.","x-sc-integer-domain":"unsigned"},"queue_full":{"$ref":"#/$defs/InputDecimalDto","description":"queue full.\\nWire queue full.","x-sc-integer-domain":"unsigned"},"reentrant_emit":{"$ref":"#/$defs/InputDecimalDto","description":"reentrant emit.\\nWire reentrant emit.","x-sc-integer-domain":"unsigned"},"shutdown_timed_out":{"$ref":"#/$defs/InputDecimalDto","description":"shutdown timed out.\\nWire shutdown timed out.","x-sc-integer-domain":"unsigned"},"writer_degraded":{"$ref":"#/$defs/InputDecimalDto","description":"writer degraded.\\nWire writer degraded.","x-sc-integer-domain":"unsigned"}},"required":["queue_full","invalid_event","writer_degraded","shutdown_timed_out","not_installed","logger_panicked","reentrant_emit"],"type":"object"},"InputFailure":{"description":"Version-one Failure wire value.","oneOf":[{"description":"Wire validation.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"field":{"description":"Wire field.","type":"string"},"kind":{"const":"validation","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","field"],"type":"object"},{"description":"Wire queue full.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"queue_full","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire below baseline.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"configured":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire configured."},"kind":{"const":"below_baseline","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","configured"],"type":"object"},{"description":"Wire unsupported level.","properties":{"at":{"description":"at.","type":"string"},"available":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire available."},"code":{"description":"code.","type":"string"},"kind":{"const":"unsupported_level","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/InputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","available"],"type":"object"},{"description":"Wire permission denied.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"permission_denied","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire closed.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"closed","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unavailable.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"unavailable","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire io.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"io","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire timeout.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"timeout","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire cancelled.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"cancelled","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire unsupported version.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"unsupported_version","type":"string"},"message":{"description":"message.","type":"string"},"received":{"description":"Wire received.","format":"uint32","minimum":0,"type":"integer"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","received"],"type":"object"},{"description":"Wire internal.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"internal","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unknown remote.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"unknown_remote","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."},"remote_kind":{"description":"Wire remote kind.","type":"string"}},"required":["kind","at","code","message","remediation","remote_kind"],"type":"object"}]},"InputFailureCountsDto":{"description":"Fixed bounded counters for the declared Failure union.","properties":{"below_baseline":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating below_baseline counter.\\nWire below baseline.","x-sc-integer-domain":"unsigned"},"cancelled":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating cancelled counter.\\nWire cancelled.","x-sc-integer-domain":"unsigned"},"closed":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating closed counter.\\nWire closed.","x-sc-integer-domain":"unsigned"},"internal":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating internal counter.\\nWire internal.","x-sc-integer-domain":"unsigned"},"io":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating io counter.\\nWire io.","x-sc-integer-domain":"unsigned"},"permission_denied":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating permission_denied counter.\\nWire permission denied.","x-sc-integer-domain":"unsigned"},"queue_full":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating queue_full counter.\\nWire queue full.","x-sc-integer-domain":"unsigned"},"timeout":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating timeout counter.\\nWire timeout.","x-sc-integer-domain":"unsigned"},"unavailable":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating unavailable counter.\\nWire unavailable.","x-sc-integer-domain":"unsigned"},"unknown_remote":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating unknown_remote counter.\\nWire unknown remote.","x-sc-integer-domain":"unsigned"},"unsupported_level":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating unsupported_level counter.\\nWire unsupported level.","x-sc-integer-domain":"unsigned"},"unsupported_version":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating unsupported_version counter.\\nWire unsupported version.","x-sc-integer-domain":"unsigned"},"validation":{"$ref":"#/$defs/InputDecimalDto","description":"Saturating validation counter.\\nWire validation.","x-sc-integer-domain":"unsigned"}},"required":["validation","queue_full","below_baseline","unsupported_level","permission_denied","closed","unavailable","io","timeout","cancelled","unsupported_version","internal","unknown_remote"],"type":"object"},"InputFieldMatchDto":{"additionalProperties":false,"description":"Version-one FieldMatchDto wire record.","properties":{"field":{"description":"field.","type":"string"},"value":{"$ref":"#/$defs/InputValueDto","description":"value."}},"required":["field","value"],"type":"object"},"InputFlushRequest":{"additionalProperties":false,"description":"Version-one FlushRequest wire record.","properties":{"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"timeout_ms":{"description":"timeout ms.\\nWire timeout ms.","format":"uint32","maximum":60000,"minimum":0,"type":"integer"}},"required":["schema_version","timeout_ms"],"type":"object"},"InputHealthRequest":{"additionalProperties":false,"description":"Version-one HealthRequest wire record.","properties":{"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version"],"type":"object"},"InputHistogramPointDto":{"additionalProperties":false,"description":"Explicit distribution; checked conversion validates cross-field invariants.","properties":{"bucket_counts":{"description":"One more count than bounds; canonical unsigned decimal strings.","items":{"$ref":"#/$defs/InputDecimalDto"},"type":"array"},"count":{"$ref":"#/$defs/InputDecimalDto","description":"Checked total sample count as a canonical unsigned decimal string."},"explicit_bounds":{"description":"Finite, strictly increasing bounds.","items":{"format":"double","type":"number"},"type":"array"},"sum":{"description":"Finite sample sum.","format":"double","type":"number"}},"required":["explicit_bounds","bucket_counts","count","sum"],"type":"object"},"InputLevelChangeDto":{"description":"Version-one LevelChangeDto wire value.","oneOf":[{"description":"Wire changed.","properties":{"current":{"$ref":"#/$defs/InputLevelStateDto","description":"Wire current."},"diagnostic":{"$ref":"#/$defs/InputChangeDiagnosticDto","description":"Wire diagnostic."},"kind":{"const":"changed","type":"string"},"previous":{"$ref":"#/$defs/InputLevelStateDto","description":"Wire previous."},"source":{"$ref":"#/$defs/InputLevelChangeSourceDto","description":"Wire source."}},"required":["kind","previous","current","source","diagnostic"],"type":"object"},{"description":"Wire unchanged.","properties":{"kind":{"const":"unchanged","type":"string"},"state":{"$ref":"#/$defs/InputLevelStateDto","description":"Wire state."}},"required":["kind","state"],"type":"object"}]},"InputLevelChangeRequest":{"additionalProperties":false,"description":"Version-one LevelChangeRequest wire record.","properties":{"change":{"$ref":"#/$defs/InputLevelRequestDto","description":"change."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","change"],"type":"object"},"InputLevelChangeSourceDto":{"description":"Version-one LevelChangeSourceDto wire value.","oneOf":[{"const":"application","description":"Wire application.","type":"string"},{"const":"user_request","description":"Wire user request.","type":"string"},{"const":"diagnostic_session","description":"Wire diagnostic session.","type":"string"}]},"InputLevelDto":{"description":"Version-one LevelDto wire value.","oneOf":[{"const":"trace","description":"Wire trace.","type":"string"},{"const":"debug","description":"Wire debug.","type":"string"},{"const":"info","description":"Wire info.","type":"string"},{"const":"warn","description":"Wire warn.","type":"string"},{"const":"error","description":"Wire error.","type":"string"}]},"InputLevelFilterDto":{"description":"Version-one LevelFilterDto wire value.","oneOf":[{"const":"off","description":"Wire off.","type":"string"},{"const":"error","description":"Wire error.","type":"string"},{"const":"warn","description":"Wire warn.","type":"string"},{"const":"info","description":"Wire info.","type":"string"},{"const":"debug","description":"Wire debug.","type":"string"},{"const":"trace","description":"Wire trace.","type":"string"}]},"InputLevelRequestDto":{"description":"Version-one LevelRequestDto wire value.","oneOf":[{"additionalProperties":false,"description":"Wire elevate.","properties":{"kind":{"const":"elevate","type":"string"},"level":{"$ref":"#/$defs/InputLevelFilterDto","description":"Active variant level."}},"required":["kind","level"],"type":"object"},{"additionalProperties":false,"description":"Wire reset.","properties":{"kind":{"const":"reset","type":"string"}},"required":["kind"],"type":"object"}]},"InputLevelStateDto":{"description":"Version-one LevelStateDto wire record.","properties":{"configured_level":{"$ref":"#/$defs/InputLevelFilterDto","description":"configured level."},"effective_level":{"$ref":"#/$defs/InputLevelFilterDto","description":"effective level."},"level_revision":{"$ref":"#/$defs/InputDecimalDto","description":"level revision.\\nWire level revision.","x-sc-integer-domain":"unsigned"}},"required":["configured_level","effective_level","level_revision"],"type":"object"},"InputLifecycleDto":{"description":"Version-one LifecycleDto wire value.","oneOf":[{"const":"running","description":"Wire running.","type":"string"},{"const":"stopping","description":"Wire stopping.","type":"string"},{"const":"stopped","description":"Wire stopped.","type":"string"},{"const":"failed","description":"Wire failed.","type":"string"}]},"InputLogEventDto":{"additionalProperties":false,"description":"Version-one LogEventDto wire record.","properties":{"action":{"description":"action.","type":"string"},"correlation_id":{"description":"correlation id.","type":["string","null"]},"fields":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"default":{},"description":"fields.\\nWire fields.","type":"object"},"level":{"$ref":"#/$defs/InputLevelDto","description":"level."},"message":{"description":"message.","type":["string","null"]},"outcome":{"description":"outcome.","type":["string","null"]},"request_id":{"description":"request id.","type":["string","null"]},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"target":{"description":"target.","type":"string"},"trace":{"anyOf":[{"$ref":"#/$defs/InputTraceContextDto"},{"type":"null"}],"description":"trace."}},"required":["schema_version","level","target","action"],"type":"object"},"InputLogHealthDto":{"description":"Version-one LogHealthDto wire record.","properties":{"bridge":{"anyOf":[{"$ref":"#/$defs/InputBridgeHealthDto"},{"type":"null"}],"description":"bridge."},"level_state":{"$ref":"#/$defs/InputLevelStateDto","description":"level state."},"logging":{"$ref":"#/$defs/InputLoggingHealthDto","description":"logging."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","logging","level_state"],"type":"object"},"InputLogOperationDto":{"description":"The log-only operation in a scheduled client outcome.","oneOf":[{"const":"log","description":"Wire log.","type":"string"}]},"InputLogOrderDto":{"description":"Version-one LogOrderDto wire value.","oneOf":[{"const":"oldest_first","description":"Wire oldest first.","type":"string"},{"const":"newest_first","description":"Wire newest first.","type":"string"}]},"InputLogQueryDto":{"additionalProperties":false,"description":"Version-one LogQueryDto wire record.","properties":{"action":{"description":"action.","type":["string","null"]},"correlation_id":{"description":"correlation id.","type":["string","null"]},"field_matches":{"default":[],"description":"field matches.\\nWire field matches.","items":{"$ref":"#/$defs/InputFieldMatchDto"},"type":"array"},"levels":{"default":[],"description":"levels.\\nWire levels.","items":{"$ref":"#/$defs/InputLevelDto"},"type":"array"},"limit":{"default":100,"description":"limit.\\nWire limit.","format":"uint","maximum":1000,"minimum":1,"type":"integer"},"order":{"$ref":"#/$defs/InputLogOrderDto","default":"oldest_first","description":"order.\\nWire order."},"request_id":{"description":"request id.","type":["string","null"]},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"service":{"description":"service.","type":["string","null"]},"since":{"description":"since.","type":["string","null"]},"target":{"description":"target.","type":["string","null"]},"until":{"description":"until.","type":["string","null"]}},"required":["schema_version"],"type":"object"},"InputLogSnapshotDto":{"description":"Version-one LogSnapshotDto wire record.","properties":{"events":{"description":"events.","items":{"$ref":"#/$defs/InputStoredEventDto"},"type":"array"},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"truncated":{"description":"truncated.","type":"boolean"}},"required":["schema_version","events","truncated"],"type":"object"},"InputLoggingHealthDto":{"description":"Version-one LoggingHealthDto wire record.","properties":{"active_log_path":{"$ref":"#/$defs/InputPathDto","description":"active log path."},"dropped_events_total":{"$ref":"#/$defs/InputDecimalDto","description":"dropped events total.\\nWire dropped events total.","x-sc-integer-domain":"unsigned"},"flush_errors_total":{"$ref":"#/$defs/InputDecimalDto","description":"flush errors total.\\nWire flush errors total.","x-sc-integer-domain":"unsigned"},"last_error":{"anyOf":[{"$ref":"#/$defs/InputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"last_writer_error":{"anyOf":[{"$ref":"#/$defs/InputDiagnosticSummaryDto"},{"type":"null"}],"description":"last writer error."},"maintenance":{"anyOf":[{"$ref":"#/$defs/InputMaintenanceHealthDto"},{"type":"null"}],"description":"maintenance."},"query":{"anyOf":[{"$ref":"#/$defs/InputQueryHealthDto"},{"type":"null"}],"description":"query."},"queue_capacity":{"$ref":"#/$defs/InputDecimalDto","description":"queue capacity.\\nWire queue capacity.","x-sc-integer-domain":"unsigned"},"queue_depth":{"$ref":"#/$defs/InputDecimalDto","description":"queue depth.\\nWire queue depth.","x-sc-integer-domain":"unsigned"},"queue_full_drops_total":{"$ref":"#/$defs/InputDecimalDto","description":"queue full drops total.\\nWire queue full drops total.","x-sc-integer-domain":"unsigned"},"queue_high_water_mark":{"$ref":"#/$defs/InputDecimalDto","description":"queue high water mark.\\nWire queue high water mark.","x-sc-integer-domain":"unsigned"},"sink_statuses":{"description":"sink statuses.","items":{"$ref":"#/$defs/InputSinkHealthDto"},"type":"array"},"state":{"$ref":"#/$defs/InputAvailabilityDto","description":"state."},"writer_state":{"$ref":"#/$defs/InputWorkerStateDto","description":"writer state."}},"required":["state","dropped_events_total","flush_errors_total","queue_depth","queue_capacity","queue_high_water_mark","queue_full_drops_total","active_log_path","sink_statuses","writer_state"],"type":"object"},"InputMaintenanceHealthDto":{"description":"Version-one MaintenanceHealthDto wire record.","properties":{"last_error":{"anyOf":[{"$ref":"#/$defs/InputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"last_pass_at":{"description":"last pass at.","type":["string","null"]},"pruned_files_total":{"$ref":"#/$defs/InputDecimalDto","description":"pruned files total.\\nWire pruned files total.","x-sc-integer-domain":"unsigned"},"rotated_files_total":{"$ref":"#/$defs/InputDecimalDto","description":"rotated files total.\\nWire rotated files total.","x-sc-integer-domain":"unsigned"},"state":{"$ref":"#/$defs/InputWorkerStateDto","description":"state."}},"required":["state","rotated_files_total","pruned_files_total"],"type":"object"},"InputMetricRecordDto":{"additionalProperties":false,"description":"Staged metric point with lossless numeric and temporal projection.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"description":"Neutral attributes with tagged integer/text distinction.","type":"object"},"name":{"description":"Validated metric name.","type":"string"},"service":{"description":"Validated service name.","type":"string"},"timestamp":{"description":"UTC point timestamp.","type":"string"},"unit":{"description":"Optional validated unit.","type":["string","null"]},"value":{"$ref":"#/$defs/InputMetricValueDto","description":"Aggregated value."}},"required":["timestamp","service","name","value","attributes"],"type":"object"},"InputMetricValueDto":{"description":"Discriminated aggregation, preserving its interval and all histogram data.","oneOf":[{"additionalProperties":false,"description":"Instantaneous finite scalar.","properties":{"data":{"format":"double","type":"number"},"kind":{"const":"gauge","type":"string"}},"required":["kind","data"],"type":"object"},{"additionalProperties":false,"description":"Finite aggregated sum.","properties":{"data":{"additionalProperties":false,"properties":{"monotonic":{"description":"Monotonic sequence indicator.","type":"boolean"},"start_time":{"description":"UTC start of interval.","type":"string"},"temporality":{"$ref":"#/$defs/InputAggregationTemporalityDto","description":"Aggregation interpretation."},"value":{"description":"Sum value.","format":"double","type":"number"}},"required":["value","monotonic","temporality","start_time"],"type":"object"},"kind":{"const":"sum","type":"string"}},"required":["kind","data"],"type":"object"},{"additionalProperties":false,"description":"Full explicit histogram.","properties":{"data":{"additionalProperties":false,"properties":{"point":{"$ref":"#/$defs/InputHistogramPointDto","description":"Validated distribution on checked conversion."},"start_time":{"description":"UTC start of interval.","type":"string"},"temporality":{"$ref":"#/$defs/InputAggregationTemporalityDto","description":"Aggregation interpretation."}},"required":["point","temporality","start_time"],"type":"object"},"kind":{"const":"histogram","type":"string"}},"required":["kind","data"],"type":"object"}]},"InputPathDto":{"description":"Version-one PathDto wire value.","oneOf":[{"description":"Wire utf8.","properties":{"kind":{"const":"utf8","type":"string"},"value":{"description":"Active variant value.","type":"string"}},"required":["kind","value"],"type":"object"},{"description":"Wire unrepresentable.","properties":{"kind":{"const":"unrepresentable","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire absent.","properties":{"kind":{"const":"absent","type":"string"}},"required":["kind"],"type":"object"}]},"InputProcessIdentityDto":{"description":"Version-one ProcessIdentityDto wire record.","properties":{"hostname":{"description":"hostname.","type":["string","null"]},"pid":{"description":"pid.","format":"uint32","minimum":0,"type":["integer","null"]}},"type":"object"},"InputQueryHealthDto":{"description":"Version-one QueryHealthDto wire record.","properties":{"last_error":{"anyOf":[{"$ref":"#/$defs/InputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"state":{"$ref":"#/$defs/InputQueryStateDto","description":"state."}},"required":["state"],"type":"object"},"InputQueryRequest":{"additionalProperties":false,"description":"Version-one QueryRequest wire record.","properties":{"query":{"$ref":"#/$defs/InputLogQueryDto","description":"query."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","query"],"type":"object"},"InputQueryStateDto":{"description":"Version-one QueryStateDto wire value.","oneOf":[{"const":"healthy","description":"Wire healthy.","type":"string"},{"const":"degraded","description":"Wire degraded.","type":"string"},{"const":"unavailable","description":"Wire unavailable.","type":"string"}]},"InputRemediationDto":{"description":"Version-one RemediationDto wire value.","oneOf":[{"description":"Wire recoverable.","properties":{"kind":{"const":"recoverable","type":"string"},"steps":{"description":"Active variant steps.","items":{"type":"string"},"type":"array"}},"required":["kind","steps"],"type":"object"},{"description":"Wire not recoverable.","properties":{"justification":{"description":"Active variant justification.","type":"string"},"kind":{"const":"not_recoverable","type":"string"}},"required":["kind","justification"],"type":"object"}]},"InputResultDto":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputAdmissionDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputResultDto2":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputCompletionDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputResultDto3":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputDispatchDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputResultDto4":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputLogSnapshotDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputResultDto5":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputLogHealthDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputResultDto6":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputLevelChangeDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputResultDto7":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputClientOutcome","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputResultDto8":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/InputClientStatus","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"InputSinkHealthDto":{"description":"Version-one SinkHealthDto wire record.","properties":{"last_error":{"anyOf":[{"$ref":"#/$defs/InputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"name":{"description":"name.","type":"string"},"state":{"$ref":"#/$defs/InputAvailabilityDto","description":"state."}},"required":["name","state"],"type":"object"},"InputSpanEventDto":{"additionalProperties":false,"description":"Span event without a lifecycle transition.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"description":"Neutral attributes.","type":"object"},"diagnostic":{"anyOf":[{"$ref":"#/$defs/InputStoredDiagnosticDto"},{"type":"null"}],"description":"Optional diagnostic."},"name":{"description":"Event action.","type":"string"},"timestamp":{"description":"UTC event timestamp.","type":"string"},"trace":{"$ref":"#/$defs/InputTraceContextV2Dto","description":"W3C correlation."}},"required":["timestamp","trace","name","attributes"],"type":"object"},"InputSpanKindDto":{"description":"Staged span role.","oneOf":[{"const":"internal","description":"Internal work.","type":"string"},{"const":"server","description":"Incoming request.","type":"string"},{"const":"client","description":"Outgoing request.","type":"string"},{"const":"producer","description":"Message producer.","type":"string"},{"const":"consumer","description":"Message consumer.","type":"string"}]},"InputSpanLinkDto":{"additionalProperties":false,"description":"Independent link correlation without a duplicate nested trace context.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"description":"Neutral attributes.","type":"object"},"flags":{"description":"Linked flags.","format":"uint8","maximum":255,"minimum":0,"type":"integer"},"span_id":{"description":"Linked span id.","type":"string"},"trace_id":{"description":"Linked trace id.","type":"string"}},"required":["trace_id","span_id","flags","attributes"],"type":"object"},"InputSpanRecordDto":{"additionalProperties":false,"description":"Raw span wire record; checked conversion replays the native typestate API.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"description":"Neutral span attributes.","type":"object"},"diagnostic":{"anyOf":[{"$ref":"#/$defs/InputStoredDiagnosticDto"},{"type":"null"}],"description":"Optional structured diagnostic."},"duration_ms":{"anyOf":[{"$ref":"#/$defs/InputDecimalDto"},{"type":"null"}],"description":"Present only for ended spans, represented as unsigned decimal milliseconds."},"kind":{"$ref":"#/$defs/InputSpanKindDto","description":"Span role."},"links":{"description":"Independent links.","items":{"$ref":"#/$defs/InputSpanLinkDto"},"type":"array"},"name":{"description":"Span action.","type":"string"},"service":{"description":"Producing service.","type":"string"},"status":{"$ref":"#/$defs/InputSpanStatusDto","description":"Outcome; active spans require Unset."},"timestamp":{"description":"UTC start timestamp.","type":"string"},"trace":{"$ref":"#/$defs/InputTraceContextV2Dto","description":"W3C correlation."}},"required":["timestamp","service","name","trace","status","attributes","kind","links"],"type":"object"},"InputSpanSignalDto":{"description":"Native-compatible external state tag; never accepts an unknown state as success.","oneOf":[{"additionalProperties":false,"description":"Active span; no duration.","properties":{"Started":{"$ref":"#/$defs/InputSpanRecordDto"}},"required":["Started"],"type":"object"},{"additionalProperties":false,"description":"Point event.","properties":{"Event":{"$ref":"#/$defs/InputSpanEventDto"}},"required":["Event"],"type":"object"},{"additionalProperties":false,"description":"Completed span with required duration.","properties":{"Ended":{"$ref":"#/$defs/InputSpanRecordDto"}},"required":["Ended"],"type":"object"}]},"InputSpanStatusDto":{"description":"Completed or active span outcome, matching native spelling.","oneOf":[{"const":"Ok","description":"Successful completion.","type":"string"},{"const":"Error","description":"Failed completion.","type":"string"},{"const":"Unset","description":"No explicit outcome.","type":"string"}]},"InputStateTransitionDto":{"description":"Version-one StateTransitionDto wire record.","properties":{"entity_id":{"description":"entity id.","type":["string","null"]},"entity_kind":{"description":"entity kind.","type":"string"},"from_state":{"description":"from state.","type":"string"},"reason":{"description":"reason.","type":["string","null"]},"to_state":{"description":"to state.","type":"string"},"trigger":{"description":"trigger.","type":["string","null"]}},"required":["entity_kind","from_state","to_state"],"type":"object"},"InputStoredDiagnosticDto":{"description":"Version-one StoredDiagnosticDto wire record.","properties":{"cause":{"description":"cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"description":"details.","type":"object"},"docs":{"description":"docs.","type":["string","null"]},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/InputRemediationDto","description":"remediation."},"timestamp":{"description":"timestamp.","type":"string"}},"required":["timestamp","code","message","remediation","details"],"type":"object"},"InputStoredEventDto":{"description":"Version-one StoredEventDto wire record.","properties":{"action":{"description":"action.","type":"string"},"correlation_id":{"description":"correlation id.","type":["string","null"]},"diagnostic":{"anyOf":[{"$ref":"#/$defs/InputStoredDiagnosticDto"},{"type":"null"}],"description":"diagnostic."},"fields":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"description":"fields.","type":"object"},"identity":{"$ref":"#/$defs/InputProcessIdentityDto","description":"identity."},"level":{"$ref":"#/$defs/InputLevelDto","description":"level."},"message":{"description":"message.","type":["string","null"]},"outcome":{"description":"outcome.","type":["string","null"]},"request_id":{"description":"request id.","type":["string","null"]},"service":{"description":"service.","type":"string"},"state_transition":{"anyOf":[{"$ref":"#/$defs/InputStateTransitionDto"},{"type":"null"}],"description":"state transition."},"target":{"description":"target.","type":"string"},"timestamp":{"description":"timestamp.","type":"string"},"trace":{"anyOf":[{"$ref":"#/$defs/InputTraceContextDto"},{"type":"null"}],"description":"trace."},"version":{"description":"version.","type":"string"}},"required":["version","timestamp","service","identity","level","target","action","fields"],"type":"object"},"InputTraceContextDto":{"additionalProperties":false,"description":"Version-one TraceContextDto wire record.","properties":{"parent_span_id":{"description":"parent span id.","type":["string","null"]},"span_id":{"description":"span id.","type":"string"},"trace_id":{"description":"trace id.","type":"string"}},"required":["trace_id","span_id"],"type":"object"},"InputTraceContextV2Dto":{"additionalProperties":false,"description":"Staged trace correlation including the complete W3C flags byte.","properties":{"flags":{"description":"All trace flags, including reserved bits.","format":"uint8","maximum":255,"minimum":0,"type":"integer"},"parent_span_id":{"description":"Optional parent span id.","type":["string","null"]},"span_id":{"description":"Lowercase W3C span id.","type":"string"},"trace_id":{"description":"Lowercase W3C trace id.","type":"string"}},"required":["trace_id","span_id","flags"],"type":"object"},"InputTryLogRequest":{"additionalProperties":false,"description":"Version-one TryLogRequest wire record.","properties":{"event":{"$ref":"#/$defs/InputLogEventDto","description":"event."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","event"],"type":"object"},"InputValueDto":{"description":"Version-one ValueDto wire value.","oneOf":[{"additionalProperties":false,"description":"Wire null.","properties":{"kind":{"const":"null","type":"string"}},"required":["kind"],"type":"object"},{"additionalProperties":false,"description":"Wire boolean.","properties":{"kind":{"const":"boolean","type":"string"},"value":{"description":"Active variant value.","type":"boolean"}},"required":["kind","value"],"type":"object"},{"additionalProperties":false,"description":"Wire string.","properties":{"kind":{"const":"string","type":"string"},"value":{"description":"Active variant value.","type":"string"}},"required":["kind","value"],"type":"object"},{"additionalProperties":false,"description":"Wire integer.","properties":{"kind":{"const":"integer","type":"string"},"value":{"$ref":"#/$defs/InputDecimalDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"additionalProperties":false,"description":"Wire float.","properties":{"kind":{"const":"float","type":"string"},"value":{"description":"Active variant value.","format":"double","type":"number"}},"required":["kind","value"],"type":"object"},{"additionalProperties":false,"description":"Wire array.","properties":{"kind":{"const":"array","type":"string"},"value":{"description":"Active variant value.","items":{"$ref":"#/$defs/InputValueDto"},"type":"array"}},"required":["kind","value"],"type":"object"},{"additionalProperties":false,"description":"Wire object.","properties":{"kind":{"const":"object","type":"string"},"value":{"additionalProperties":{"$ref":"#/$defs/InputValueDto"},"description":"Active variant value.","type":"object"}},"required":["kind","value"],"type":"object"}]},"InputWireEnvelope":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputAdmissionDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWireEnvelope2":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputCompletionDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWireEnvelope3":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputDispatchDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWireEnvelope4":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputLogSnapshotDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWireEnvelope5":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputLogHealthDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWireEnvelope6":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputLevelChangeDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWireEnvelope7":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputClientOutcome","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWireEnvelope8":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/InputClientStatus","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/InputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"InputWorkerStateDto":{"description":"Version-one WorkerStateDto wire value.","oneOf":[{"const":"running","description":"Wire running.","type":"string"},{"const":"degraded","description":"Wire degraded.","type":"string"},{"const":"stopped","description":"Wire stopped.","type":"string"}]},"OutputAdmissionDto":{"description":"Version-one AdmissionDto wire value.","oneOf":[{"description":"Wire accepted.","properties":{"kind":{"const":"accepted","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire filtered.","properties":{"kind":{"const":"filtered","type":"string"}},"required":["kind"],"type":"object"}]},"OutputAdmissionOperationDto":{"description":"Operations returning final logging admission.","oneOf":[{"const":"log","description":"Wire log.","type":"string"},{"const":"try_log","description":"Wire try log.","type":"string"}]},"OutputAggregationTemporalityDto":{"description":"Staged aggregation interval interpretation.","oneOf":[{"const":"delta","description":"Nonempty collection interval.","type":"string"},{"const":"cumulative","description":"Sequence-start interval.","type":"string"}]},"OutputAvailabilityDto":{"description":"Version-one AvailabilityDto wire value.","oneOf":[{"const":"healthy","description":"Wire healthy.","type":"string"},{"const":"degraded_dropping","description":"Wire degraded dropping.","type":"string"},{"const":"unavailable","description":"Wire unavailable.","type":"string"}]},"OutputBridgeHealthDto":{"description":"Version-one BridgeHealthDto wire record.","properties":{"active_log_path":{"$ref":"#/$defs/OutputPathDto","description":"active log path."},"configured_level":{"$ref":"#/$defs/OutputLevelFilterDto","description":"configured level."},"dropped":{"$ref":"#/$defs/OutputDropCountsDto","description":"dropped."},"effective_level":{"$ref":"#/$defs/OutputLevelFilterDto","description":"effective level."},"level_revision":{"$ref":"#/$defs/OutputDecimalDto","description":"level revision.\\nWire level revision.","x-sc-integer-domain":"unsigned"},"lifecycle":{"$ref":"#/$defs/OutputLifecycleDto","description":"lifecycle."},"logging":{"$ref":"#/$defs/OutputLoggingHealthDto","description":"logging."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","logging","dropped","lifecycle","active_log_path","configured_level","effective_level","level_revision"],"type":"object"},"OutputCanonicalDiagnosticDto":{"description":"Additive canonical diagnostic projection; source objects remain native.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["at","code","message","remediation"],"type":"object"},"OutputCanonicalFailureDto":{"description":"Additive operational failures preserving canonical diagnostic metadata.","oneOf":[{"description":"Wire validation.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"field":{"description":"Wire field.","type":"string"},"kind":{"const":"validation","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","field"],"type":"object"},{"description":"Wire queue full.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"queue_full","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire below baseline.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"configured":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire configured."},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"below_baseline","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","configured"],"type":"object"},{"description":"Wire unsupported level.","properties":{"at":{"description":"at.","type":"string"},"available":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire available."},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unsupported_level","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","available"],"type":"object"},{"description":"Wire permission denied.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"permission_denied","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire closed.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"closed","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unavailable.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unavailable","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire io.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"io","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire timeout.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"timeout","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire cancelled.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"cancelled","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire unsupported version.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unsupported_version","type":"string"},"message":{"description":"message.","type":"string"},"received":{"description":"Wire received.","format":"uint32","minimum":0,"type":"integer"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","received"],"type":"object"},{"description":"Wire internal.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"internal","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unknown remote.","properties":{"at":{"description":"at.","type":"string"},"cause":{"description":"Already-redacted human-readable cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"Bounded structured details using the lossless integer codec.","type":"object"},"docs":{"description":"Documentation reference.","type":["string","null"]},"kind":{"const":"unknown_remote","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."},"remote_kind":{"description":"Wire remote kind.","type":"string"}},"required":["kind","at","code","message","remediation","remote_kind"],"type":"object"}]},"OutputCanonicalWireEnvelope":{"description":"Staged envelope with unchanged schema/version/discriminants and richer diagnostics.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputAdmissionDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputCanonicalFailureDto","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputChangeDiagnosticDto":{"description":"Version-one ChangeDiagnosticDto wire value.","oneOf":[{"description":"Wire accepted.","properties":{"kind":{"const":"accepted","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire not accepted.","properties":{"diagnostic":{"$ref":"#/$defs/OutputDiagnostic","description":"Active variant diagnostic."},"kind":{"const":"not_accepted","type":"string"}},"required":["kind","diagnostic"],"type":"object"}]},"OutputClientOutcome":{"description":"Payload-free client outcome, distinct from host persistence.","oneOf":[{"description":"Wire idle.","properties":{"kind":{"const":"idle","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire scheduled.","properties":{"kind":{"const":"scheduled","type":"string"},"operation":{"$ref":"#/$defs/OutputLogOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"},{"description":"Wire accepted.","properties":{"kind":{"const":"accepted","type":"string"},"operation":{"$ref":"#/$defs/OutputAdmissionOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"},{"description":"Wire filtered.","properties":{"kind":{"const":"filtered","type":"string"},"operation":{"$ref":"#/$defs/OutputAdmissionOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"},{"description":"Wire completed.","properties":{"kind":{"const":"completed","type":"string"},"operation":{"$ref":"#/$defs/OutputCompletionOperationDto","description":"Active variant operation."}},"required":["kind","operation"],"type":"object"}]},"OutputClientStatus":{"description":"Bounded local client status; no ownership or IPC capability is represented.","properties":{"failures_by_kind":{"$ref":"#/$defs/OutputFailureCountsDto","description":"Saturating counters for every declared failure kind."},"in_flight":{"description":"Number of outstanding operations, bounded by client admission.\\nWire in flight.","format":"uint32","maximum":256,"minimum":0,"type":"integer"},"last_failure":{"anyOf":[{"$ref":"#/$defs/OutputFailure"},{"type":"null"}],"description":"Retained failure survives subsequent successful operations."},"last_result":{"$ref":"#/$defs/OutputResultDto7","description":"Most recent completion-order result."}},"required":["in_flight","failures_by_kind","last_result","last_failure"],"type":"object"},"OutputCompletionDto":{"description":"Version-one CompletionDto wire value.","oneOf":[{"description":"Wire completed.","properties":{"kind":{"const":"completed","type":"string"}},"required":["kind"],"type":"object"}]},"OutputCompletionOperationDto":{"description":"Operations returning a completed client observation.","oneOf":[{"const":"query","description":"Wire query.","type":"string"},{"const":"health","description":"Wire health.","type":"string"},{"const":"flush","description":"Wire flush.","type":"string"}]},"OutputDecimalDto":{"description":"Canonical integer string: signed i64 or unsigned u64; counters additionally reject negatives.","pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)(?![\\\\s\\\\S])","type":"string"},"OutputDiagnostic":{"description":"Version-one Diagnostic wire record.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["at","code","message","remediation"],"type":"object"},"OutputDiagnosticSummaryDto":{"description":"Version-one DiagnosticSummaryDto wire record.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":["string","null"]},"message":{"description":"message.","type":"string"}},"required":["code","message","at"],"type":"object"},"OutputDispatchDto":{"description":"Version-one DispatchDto wire value.","oneOf":[{"description":"Wire scheduled.","properties":{"kind":{"const":"scheduled","type":"string"}},"required":["kind"],"type":"object"}]},"OutputDropCountsDto":{"description":"Version-one DropCountsDto wire record.","properties":{"invalid_event":{"$ref":"#/$defs/OutputDecimalDto","description":"invalid event.\\nWire invalid event.","x-sc-integer-domain":"unsigned"},"logger_panicked":{"$ref":"#/$defs/OutputDecimalDto","description":"logger panicked.\\nWire logger panicked.","x-sc-integer-domain":"unsigned"},"not_installed":{"$ref":"#/$defs/OutputDecimalDto","description":"not installed.\\nWire not installed.","x-sc-integer-domain":"unsigned"},"queue_full":{"$ref":"#/$defs/OutputDecimalDto","description":"queue full.\\nWire queue full.","x-sc-integer-domain":"unsigned"},"reentrant_emit":{"$ref":"#/$defs/OutputDecimalDto","description":"reentrant emit.\\nWire reentrant emit.","x-sc-integer-domain":"unsigned"},"shutdown_timed_out":{"$ref":"#/$defs/OutputDecimalDto","description":"shutdown timed out.\\nWire shutdown timed out.","x-sc-integer-domain":"unsigned"},"writer_degraded":{"$ref":"#/$defs/OutputDecimalDto","description":"writer degraded.\\nWire writer degraded.","x-sc-integer-domain":"unsigned"}},"required":["queue_full","invalid_event","writer_degraded","shutdown_timed_out","not_installed","logger_panicked","reentrant_emit"],"type":"object"},"OutputFailure":{"description":"Version-one Failure wire value.","oneOf":[{"description":"Wire validation.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"field":{"description":"Wire field.","type":"string"},"kind":{"const":"validation","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","field"],"type":"object"},{"description":"Wire queue full.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"queue_full","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire below baseline.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"configured":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire configured."},"kind":{"const":"below_baseline","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","configured"],"type":"object"},{"description":"Wire unsupported level.","properties":{"at":{"description":"at.","type":"string"},"available":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire available."},"code":{"description":"code.","type":"string"},"kind":{"const":"unsupported_level","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."},"requested":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Wire requested."}},"required":["kind","at","code","message","remediation","requested","available"],"type":"object"},{"description":"Wire permission denied.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"permission_denied","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire closed.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"closed","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unavailable.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"unavailable","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire io.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"io","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire timeout.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"timeout","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire cancelled.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"cancelled","type":"string"},"message":{"description":"message.","type":"string"},"operation":{"description":"Wire operation.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","operation"],"type":"object"},{"description":"Wire unsupported version.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"unsupported_version","type":"string"},"message":{"description":"message.","type":"string"},"received":{"description":"Wire received.","format":"uint32","minimum":0,"type":"integer"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation","received"],"type":"object"},{"description":"Wire internal.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"internal","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."}},"required":["kind","at","code","message","remediation"],"type":"object"},{"description":"Wire unknown remote.","properties":{"at":{"description":"at.","type":"string"},"code":{"description":"code.","type":"string"},"kind":{"const":"unknown_remote","type":"string"},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."},"remote_kind":{"description":"Wire remote kind.","type":"string"}},"required":["kind","at","code","message","remediation","remote_kind"],"type":"object"}]},"OutputFailureCountsDto":{"description":"Fixed bounded counters for the declared Failure union.","properties":{"below_baseline":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating below_baseline counter.\\nWire below baseline.","x-sc-integer-domain":"unsigned"},"cancelled":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating cancelled counter.\\nWire cancelled.","x-sc-integer-domain":"unsigned"},"closed":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating closed counter.\\nWire closed.","x-sc-integer-domain":"unsigned"},"internal":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating internal counter.\\nWire internal.","x-sc-integer-domain":"unsigned"},"io":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating io counter.\\nWire io.","x-sc-integer-domain":"unsigned"},"permission_denied":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating permission_denied counter.\\nWire permission denied.","x-sc-integer-domain":"unsigned"},"queue_full":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating queue_full counter.\\nWire queue full.","x-sc-integer-domain":"unsigned"},"timeout":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating timeout counter.\\nWire timeout.","x-sc-integer-domain":"unsigned"},"unavailable":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating unavailable counter.\\nWire unavailable.","x-sc-integer-domain":"unsigned"},"unknown_remote":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating unknown_remote counter.\\nWire unknown remote.","x-sc-integer-domain":"unsigned"},"unsupported_level":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating unsupported_level counter.\\nWire unsupported level.","x-sc-integer-domain":"unsigned"},"unsupported_version":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating unsupported_version counter.\\nWire unsupported version.","x-sc-integer-domain":"unsigned"},"validation":{"$ref":"#/$defs/OutputDecimalDto","description":"Saturating validation counter.\\nWire validation.","x-sc-integer-domain":"unsigned"}},"required":["validation","queue_full","below_baseline","unsupported_level","permission_denied","closed","unavailable","io","timeout","cancelled","unsupported_version","internal","unknown_remote"],"type":"object"},"OutputFieldMatchDto":{"additionalProperties":false,"description":"Version-one FieldMatchDto wire record.","properties":{"field":{"description":"field.","type":"string"},"value":{"$ref":"#/$defs/OutputValueDto","description":"value."}},"required":["field","value"],"type":"object"},"OutputFlushRequest":{"additionalProperties":false,"description":"Version-one FlushRequest wire record.","properties":{"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"timeout_ms":{"description":"timeout ms.\\nWire timeout ms.","format":"uint32","maximum":60000,"minimum":0,"type":"integer"}},"required":["schema_version","timeout_ms"],"type":"object"},"OutputHealthRequest":{"additionalProperties":false,"description":"Version-one HealthRequest wire record.","properties":{"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version"],"type":"object"},"OutputHistogramPointDto":{"additionalProperties":false,"description":"Explicit distribution; checked conversion validates cross-field invariants.","properties":{"bucket_counts":{"description":"One more count than bounds; canonical unsigned decimal strings.","items":{"$ref":"#/$defs/OutputDecimalDto"},"type":"array"},"count":{"$ref":"#/$defs/OutputDecimalDto","description":"Checked total sample count as a canonical unsigned decimal string."},"explicit_bounds":{"description":"Finite, strictly increasing bounds.","items":{"format":"double","type":"number"},"type":"array"},"sum":{"description":"Finite sample sum.","format":"double","type":"number"}},"required":["explicit_bounds","bucket_counts","count","sum"],"type":"object"},"OutputLevelChangeDto":{"description":"Version-one LevelChangeDto wire value.","oneOf":[{"description":"Wire changed.","properties":{"current":{"$ref":"#/$defs/OutputLevelStateDto","description":"Wire current."},"diagnostic":{"$ref":"#/$defs/OutputChangeDiagnosticDto","description":"Wire diagnostic."},"kind":{"const":"changed","type":"string"},"previous":{"$ref":"#/$defs/OutputLevelStateDto","description":"Wire previous."},"source":{"$ref":"#/$defs/OutputLevelChangeSourceDto","description":"Wire source."}},"required":["kind","previous","current","source","diagnostic"],"type":"object"},{"description":"Wire unchanged.","properties":{"kind":{"const":"unchanged","type":"string"},"state":{"$ref":"#/$defs/OutputLevelStateDto","description":"Wire state."}},"required":["kind","state"],"type":"object"}]},"OutputLevelChangeRequest":{"additionalProperties":false,"description":"Version-one LevelChangeRequest wire record.","properties":{"change":{"$ref":"#/$defs/OutputLevelRequestDto","description":"change."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","change"],"type":"object"},"OutputLevelChangeSourceDto":{"description":"Version-one LevelChangeSourceDto wire value.","oneOf":[{"const":"application","description":"Wire application.","type":"string"},{"const":"user_request","description":"Wire user request.","type":"string"},{"const":"diagnostic_session","description":"Wire diagnostic session.","type":"string"}]},"OutputLevelDto":{"description":"Version-one LevelDto wire value.","oneOf":[{"const":"trace","description":"Wire trace.","type":"string"},{"const":"debug","description":"Wire debug.","type":"string"},{"const":"info","description":"Wire info.","type":"string"},{"const":"warn","description":"Wire warn.","type":"string"},{"const":"error","description":"Wire error.","type":"string"}]},"OutputLevelFilterDto":{"description":"Version-one LevelFilterDto wire value.","oneOf":[{"const":"off","description":"Wire off.","type":"string"},{"const":"error","description":"Wire error.","type":"string"},{"const":"warn","description":"Wire warn.","type":"string"},{"const":"info","description":"Wire info.","type":"string"},{"const":"debug","description":"Wire debug.","type":"string"},{"const":"trace","description":"Wire trace.","type":"string"}]},"OutputLevelRequestDto":{"description":"Version-one LevelRequestDto wire value.","oneOf":[{"additionalProperties":false,"description":"Wire elevate.","properties":{"kind":{"const":"elevate","type":"string"},"level":{"$ref":"#/$defs/OutputLevelFilterDto","description":"Active variant level."}},"required":["kind","level"],"type":"object"},{"additionalProperties":false,"description":"Wire reset.","properties":{"kind":{"const":"reset","type":"string"}},"required":["kind"],"type":"object"}]},"OutputLevelStateDto":{"description":"Version-one LevelStateDto wire record.","properties":{"configured_level":{"$ref":"#/$defs/OutputLevelFilterDto","description":"configured level."},"effective_level":{"$ref":"#/$defs/OutputLevelFilterDto","description":"effective level."},"level_revision":{"$ref":"#/$defs/OutputDecimalDto","description":"level revision.\\nWire level revision.","x-sc-integer-domain":"unsigned"}},"required":["configured_level","effective_level","level_revision"],"type":"object"},"OutputLifecycleDto":{"description":"Version-one LifecycleDto wire value.","oneOf":[{"const":"running","description":"Wire running.","type":"string"},{"const":"stopping","description":"Wire stopping.","type":"string"},{"const":"stopped","description":"Wire stopped.","type":"string"},{"const":"failed","description":"Wire failed.","type":"string"}]},"OutputLogEventDto":{"additionalProperties":false,"description":"Version-one LogEventDto wire record.","properties":{"action":{"description":"action.","type":"string"},"correlation_id":{"description":"correlation id.","type":["string","null"]},"fields":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"default":{},"description":"fields.\\nWire fields.","type":"object"},"level":{"$ref":"#/$defs/OutputLevelDto","description":"level."},"message":{"description":"message.","type":["string","null"]},"outcome":{"description":"outcome.","type":["string","null"]},"request_id":{"description":"request id.","type":["string","null"]},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"target":{"description":"target.","type":"string"},"trace":{"anyOf":[{"$ref":"#/$defs/OutputTraceContextDto"},{"type":"null"}],"description":"trace."}},"required":["schema_version","level","target","action","message","trace","request_id","correlation_id","outcome","fields"],"type":"object"},"OutputLogHealthDto":{"description":"Version-one LogHealthDto wire record.","properties":{"bridge":{"anyOf":[{"$ref":"#/$defs/OutputBridgeHealthDto"},{"type":"null"}],"description":"bridge."},"level_state":{"$ref":"#/$defs/OutputLevelStateDto","description":"level state."},"logging":{"$ref":"#/$defs/OutputLoggingHealthDto","description":"logging."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","logging","bridge","level_state"],"type":"object"},"OutputLogOperationDto":{"description":"The log-only operation in a scheduled client outcome.","oneOf":[{"const":"log","description":"Wire log.","type":"string"}]},"OutputLogOrderDto":{"description":"Version-one LogOrderDto wire value.","oneOf":[{"const":"oldest_first","description":"Wire oldest first.","type":"string"},{"const":"newest_first","description":"Wire newest first.","type":"string"}]},"OutputLogQueryDto":{"additionalProperties":false,"description":"Version-one LogQueryDto wire record.","properties":{"action":{"description":"action.","type":["string","null"]},"correlation_id":{"description":"correlation id.","type":["string","null"]},"field_matches":{"default":[],"description":"field matches.\\nWire field matches.","items":{"$ref":"#/$defs/OutputFieldMatchDto"},"type":"array"},"levels":{"default":[],"description":"levels.\\nWire levels.","items":{"$ref":"#/$defs/OutputLevelDto"},"type":"array"},"limit":{"default":100,"description":"limit.\\nWire limit.","format":"uint","maximum":1000,"minimum":1,"type":"integer"},"order":{"$ref":"#/$defs/OutputLogOrderDto","default":"oldest_first","description":"order.\\nWire order."},"request_id":{"description":"request id.","type":["string","null"]},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"service":{"description":"service.","type":["string","null"]},"since":{"description":"since.","type":["string","null"]},"target":{"description":"target.","type":["string","null"]},"until":{"description":"until.","type":["string","null"]}},"required":["schema_version","service","levels","target","action","request_id","correlation_id","since","until","field_matches","limit","order"],"type":"object"},"OutputLogSnapshotDto":{"description":"Version-one LogSnapshotDto wire record.","properties":{"events":{"description":"events.","items":{"$ref":"#/$defs/OutputStoredEventDto"},"type":"array"},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"truncated":{"description":"truncated.","type":"boolean"}},"required":["schema_version","events","truncated"],"type":"object"},"OutputLoggingHealthDto":{"description":"Version-one LoggingHealthDto wire record.","properties":{"active_log_path":{"$ref":"#/$defs/OutputPathDto","description":"active log path."},"dropped_events_total":{"$ref":"#/$defs/OutputDecimalDto","description":"dropped events total.\\nWire dropped events total.","x-sc-integer-domain":"unsigned"},"flush_errors_total":{"$ref":"#/$defs/OutputDecimalDto","description":"flush errors total.\\nWire flush errors total.","x-sc-integer-domain":"unsigned"},"last_error":{"anyOf":[{"$ref":"#/$defs/OutputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"last_writer_error":{"anyOf":[{"$ref":"#/$defs/OutputDiagnosticSummaryDto"},{"type":"null"}],"description":"last writer error."},"maintenance":{"anyOf":[{"$ref":"#/$defs/OutputMaintenanceHealthDto"},{"type":"null"}],"description":"maintenance."},"query":{"anyOf":[{"$ref":"#/$defs/OutputQueryHealthDto"},{"type":"null"}],"description":"query."},"queue_capacity":{"$ref":"#/$defs/OutputDecimalDto","description":"queue capacity.\\nWire queue capacity.","x-sc-integer-domain":"unsigned"},"queue_depth":{"$ref":"#/$defs/OutputDecimalDto","description":"queue depth.\\nWire queue depth.","x-sc-integer-domain":"unsigned"},"queue_full_drops_total":{"$ref":"#/$defs/OutputDecimalDto","description":"queue full drops total.\\nWire queue full drops total.","x-sc-integer-domain":"unsigned"},"queue_high_water_mark":{"$ref":"#/$defs/OutputDecimalDto","description":"queue high water mark.\\nWire queue high water mark.","x-sc-integer-domain":"unsigned"},"sink_statuses":{"description":"sink statuses.","items":{"$ref":"#/$defs/OutputSinkHealthDto"},"type":"array"},"state":{"$ref":"#/$defs/OutputAvailabilityDto","description":"state."},"writer_state":{"$ref":"#/$defs/OutputWorkerStateDto","description":"writer state."}},"required":["state","dropped_events_total","flush_errors_total","queue_depth","queue_capacity","queue_high_water_mark","queue_full_drops_total","active_log_path","sink_statuses","writer_state","last_writer_error","query","maintenance","last_error"],"type":"object"},"OutputMaintenanceHealthDto":{"description":"Version-one MaintenanceHealthDto wire record.","properties":{"last_error":{"anyOf":[{"$ref":"#/$defs/OutputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"last_pass_at":{"description":"last pass at.","type":["string","null"]},"pruned_files_total":{"$ref":"#/$defs/OutputDecimalDto","description":"pruned files total.\\nWire pruned files total.","x-sc-integer-domain":"unsigned"},"rotated_files_total":{"$ref":"#/$defs/OutputDecimalDto","description":"rotated files total.\\nWire rotated files total.","x-sc-integer-domain":"unsigned"},"state":{"$ref":"#/$defs/OutputWorkerStateDto","description":"state."}},"required":["state","last_pass_at","rotated_files_total","pruned_files_total","last_error"],"type":"object"},"OutputMetricRecordDto":{"additionalProperties":false,"description":"Staged metric point with lossless numeric and temporal projection.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"description":"Neutral attributes with tagged integer/text distinction.","type":"object"},"name":{"description":"Validated metric name.","type":"string"},"service":{"description":"Validated service name.","type":"string"},"timestamp":{"description":"UTC point timestamp.","type":"string"},"unit":{"description":"Optional validated unit.","type":["string","null"]},"value":{"$ref":"#/$defs/OutputMetricValueDto","description":"Aggregated value."}},"required":["timestamp","service","name","value","unit","attributes"],"type":"object"},"OutputMetricValueDto":{"description":"Discriminated aggregation, preserving its interval and all histogram data.","oneOf":[{"additionalProperties":false,"description":"Instantaneous finite scalar.","properties":{"data":{"format":"double","type":"number"},"kind":{"const":"gauge","type":"string"}},"required":["kind","data"],"type":"object"},{"additionalProperties":false,"description":"Finite aggregated sum.","properties":{"data":{"additionalProperties":false,"properties":{"monotonic":{"description":"Monotonic sequence indicator.","type":"boolean"},"start_time":{"description":"UTC start of interval.","type":"string"},"temporality":{"$ref":"#/$defs/OutputAggregationTemporalityDto","description":"Aggregation interpretation."},"value":{"description":"Sum value.","format":"double","type":"number"}},"required":["value","monotonic","temporality","start_time"],"type":"object"},"kind":{"const":"sum","type":"string"}},"required":["kind","data"],"type":"object"},{"additionalProperties":false,"description":"Full explicit histogram.","properties":{"data":{"additionalProperties":false,"properties":{"point":{"$ref":"#/$defs/OutputHistogramPointDto","description":"Validated distribution on checked conversion."},"start_time":{"description":"UTC start of interval.","type":"string"},"temporality":{"$ref":"#/$defs/OutputAggregationTemporalityDto","description":"Aggregation interpretation."}},"required":["point","temporality","start_time"],"type":"object"},"kind":{"const":"histogram","type":"string"}},"required":["kind","data"],"type":"object"}]},"OutputPathDto":{"description":"Version-one PathDto wire value.","oneOf":[{"description":"Wire utf8.","properties":{"kind":{"const":"utf8","type":"string"},"value":{"description":"Active variant value.","type":"string"}},"required":["kind","value"],"type":"object"},{"description":"Wire unrepresentable.","properties":{"kind":{"const":"unrepresentable","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire absent.","properties":{"kind":{"const":"absent","type":"string"}},"required":["kind"],"type":"object"}]},"OutputProcessIdentityDto":{"description":"Version-one ProcessIdentityDto wire record.","properties":{"hostname":{"description":"hostname.","type":["string","null"]},"pid":{"description":"pid.","format":"uint32","minimum":0,"type":["integer","null"]}},"required":["hostname","pid"],"type":"object"},"OutputQueryHealthDto":{"description":"Version-one QueryHealthDto wire record.","properties":{"last_error":{"anyOf":[{"$ref":"#/$defs/OutputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"state":{"$ref":"#/$defs/OutputQueryStateDto","description":"state."}},"required":["state","last_error"],"type":"object"},"OutputQueryRequest":{"additionalProperties":false,"description":"Version-one QueryRequest wire record.","properties":{"query":{"$ref":"#/$defs/OutputLogQueryDto","description":"query."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","query"],"type":"object"},"OutputQueryStateDto":{"description":"Version-one QueryStateDto wire value.","oneOf":[{"const":"healthy","description":"Wire healthy.","type":"string"},{"const":"degraded","description":"Wire degraded.","type":"string"},{"const":"unavailable","description":"Wire unavailable.","type":"string"}]},"OutputRemediationDto":{"description":"Version-one RemediationDto wire value.","oneOf":[{"description":"Wire recoverable.","properties":{"kind":{"const":"recoverable","type":"string"},"steps":{"description":"Active variant steps.","items":{"type":"string"},"type":"array"}},"required":["kind","steps"],"type":"object"},{"description":"Wire not recoverable.","properties":{"justification":{"description":"Active variant justification.","type":"string"},"kind":{"const":"not_recoverable","type":"string"}},"required":["kind","justification"],"type":"object"}]},"OutputResultDto":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputAdmissionDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputResultDto2":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputCompletionDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputResultDto3":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputDispatchDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputResultDto4":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputLogSnapshotDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputResultDto5":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputLogHealthDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputResultDto6":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputLevelChangeDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputResultDto7":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputClientOutcome","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputResultDto8":{"description":"Version-one `ResultDto<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"value":{"$ref":"#/$defs/OutputClientStatus","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"}},"required":["kind","error"],"type":"object"}]},"OutputSinkHealthDto":{"description":"Version-one SinkHealthDto wire record.","properties":{"last_error":{"anyOf":[{"$ref":"#/$defs/OutputDiagnosticSummaryDto"},{"type":"null"}],"description":"last error."},"name":{"description":"name.","type":"string"},"state":{"$ref":"#/$defs/OutputAvailabilityDto","description":"state."}},"required":["name","state","last_error"],"type":"object"},"OutputSpanEventDto":{"additionalProperties":false,"description":"Span event without a lifecycle transition.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"description":"Neutral attributes.","type":"object"},"diagnostic":{"anyOf":[{"$ref":"#/$defs/OutputStoredDiagnosticDto"},{"type":"null"}],"description":"Optional diagnostic."},"name":{"description":"Event action.","type":"string"},"timestamp":{"description":"UTC event timestamp.","type":"string"},"trace":{"$ref":"#/$defs/OutputTraceContextV2Dto","description":"W3C correlation."}},"required":["timestamp","trace","name","attributes","diagnostic"],"type":"object"},"OutputSpanKindDto":{"description":"Staged span role.","oneOf":[{"const":"internal","description":"Internal work.","type":"string"},{"const":"server","description":"Incoming request.","type":"string"},{"const":"client","description":"Outgoing request.","type":"string"},{"const":"producer","description":"Message producer.","type":"string"},{"const":"consumer","description":"Message consumer.","type":"string"}]},"OutputSpanLinkDto":{"additionalProperties":false,"description":"Independent link correlation without a duplicate nested trace context.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"description":"Neutral attributes.","type":"object"},"flags":{"description":"Linked flags.","format":"uint8","maximum":255,"minimum":0,"type":"integer"},"span_id":{"description":"Linked span id.","type":"string"},"trace_id":{"description":"Linked trace id.","type":"string"}},"required":["trace_id","span_id","flags","attributes"],"type":"object"},"OutputSpanRecordDto":{"additionalProperties":false,"description":"Raw span wire record; checked conversion replays the native typestate API.","properties":{"attributes":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"description":"Neutral span attributes.","type":"object"},"diagnostic":{"anyOf":[{"$ref":"#/$defs/OutputStoredDiagnosticDto"},{"type":"null"}],"description":"Optional structured diagnostic."},"duration_ms":{"anyOf":[{"$ref":"#/$defs/OutputDecimalDto"},{"type":"null"}],"description":"Present only for ended spans, represented as unsigned decimal milliseconds."},"kind":{"$ref":"#/$defs/OutputSpanKindDto","description":"Span role."},"links":{"description":"Independent links.","items":{"$ref":"#/$defs/OutputSpanLinkDto"},"type":"array"},"name":{"description":"Span action.","type":"string"},"service":{"description":"Producing service.","type":"string"},"status":{"$ref":"#/$defs/OutputSpanStatusDto","description":"Outcome; active spans require Unset."},"timestamp":{"description":"UTC start timestamp.","type":"string"},"trace":{"$ref":"#/$defs/OutputTraceContextV2Dto","description":"W3C correlation."}},"required":["timestamp","service","name","trace","status","diagnostic","attributes","duration_ms","kind","links"],"type":"object"},"OutputSpanSignalDto":{"description":"Native-compatible external state tag; never accepts an unknown state as success.","oneOf":[{"additionalProperties":false,"description":"Active span; no duration.","properties":{"Started":{"$ref":"#/$defs/OutputSpanRecordDto"}},"required":["Started"],"type":"object"},{"additionalProperties":false,"description":"Point event.","properties":{"Event":{"$ref":"#/$defs/OutputSpanEventDto"}},"required":["Event"],"type":"object"},{"additionalProperties":false,"description":"Completed span with required duration.","properties":{"Ended":{"$ref":"#/$defs/OutputSpanRecordDto"}},"required":["Ended"],"type":"object"}]},"OutputSpanStatusDto":{"description":"Completed or active span outcome, matching native spelling.","oneOf":[{"const":"Ok","description":"Successful completion.","type":"string"},{"const":"Error","description":"Failed completion.","type":"string"},{"const":"Unset","description":"No explicit outcome.","type":"string"}]},"OutputStateTransitionDto":{"description":"Version-one StateTransitionDto wire record.","properties":{"entity_id":{"description":"entity id.","type":["string","null"]},"entity_kind":{"description":"entity kind.","type":"string"},"from_state":{"description":"from state.","type":"string"},"reason":{"description":"reason.","type":["string","null"]},"to_state":{"description":"to state.","type":"string"},"trigger":{"description":"trigger.","type":["string","null"]}},"required":["entity_kind","entity_id","from_state","to_state","reason","trigger"],"type":"object"},"OutputStoredDiagnosticDto":{"description":"Version-one StoredDiagnosticDto wire record.","properties":{"cause":{"description":"cause.","type":["string","null"]},"code":{"description":"code.","type":"string"},"details":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"description":"details.","type":"object"},"docs":{"description":"docs.","type":["string","null"]},"message":{"description":"message.","type":"string"},"remediation":{"$ref":"#/$defs/OutputRemediationDto","description":"remediation."},"timestamp":{"description":"timestamp.","type":"string"}},"required":["timestamp","code","message","cause","remediation","docs","details"],"type":"object"},"OutputStoredEventDto":{"description":"Version-one StoredEventDto wire record.","properties":{"action":{"description":"action.","type":"string"},"correlation_id":{"description":"correlation id.","type":["string","null"]},"diagnostic":{"anyOf":[{"$ref":"#/$defs/OutputStoredDiagnosticDto"},{"type":"null"}],"description":"diagnostic."},"fields":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"description":"fields.","type":"object"},"identity":{"$ref":"#/$defs/OutputProcessIdentityDto","description":"identity."},"level":{"$ref":"#/$defs/OutputLevelDto","description":"level."},"message":{"description":"message.","type":["string","null"]},"outcome":{"description":"outcome.","type":["string","null"]},"request_id":{"description":"request id.","type":["string","null"]},"service":{"description":"service.","type":"string"},"state_transition":{"anyOf":[{"$ref":"#/$defs/OutputStateTransitionDto"},{"type":"null"}],"description":"state transition."},"target":{"description":"target.","type":"string"},"timestamp":{"description":"timestamp.","type":"string"},"trace":{"anyOf":[{"$ref":"#/$defs/OutputTraceContextDto"},{"type":"null"}],"description":"trace."},"version":{"description":"version.","type":"string"}},"required":["version","timestamp","service","identity","level","target","action","message","trace","request_id","correlation_id","outcome","fields","diagnostic","state_transition"],"type":"object"},"OutputTraceContextDto":{"description":"Version-one TraceContextDto wire record.","properties":{"parent_span_id":{"description":"parent span id.","type":["string","null"]},"span_id":{"description":"span id.","type":"string"},"trace_id":{"description":"trace id.","type":"string"}},"required":["trace_id","span_id","parent_span_id"],"type":"object"},"OutputTraceContextV2Dto":{"additionalProperties":false,"description":"Staged trace correlation including the complete W3C flags byte.","properties":{"flags":{"description":"All trace flags, including reserved bits.","format":"uint8","maximum":255,"minimum":0,"type":"integer"},"parent_span_id":{"description":"Optional parent span id.","type":["string","null"]},"span_id":{"description":"Lowercase W3C span id.","type":"string"},"trace_id":{"description":"Lowercase W3C trace id.","type":"string"}},"required":["trace_id","span_id","parent_span_id","flags"],"type":"object"},"OutputTryLogRequest":{"additionalProperties":false,"description":"Version-one TryLogRequest wire record.","properties":{"event":{"$ref":"#/$defs/OutputLogEventDto","description":"event."},"schema_version":{"description":"schema version.\\nWire schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["schema_version","event"],"type":"object"},"OutputValueDto":{"description":"Version-one ValueDto wire value.","oneOf":[{"description":"Wire null.","properties":{"kind":{"const":"null","type":"string"}},"required":["kind"],"type":"object"},{"description":"Wire boolean.","properties":{"kind":{"const":"boolean","type":"string"},"value":{"description":"Active variant value.","type":"boolean"}},"required":["kind","value"],"type":"object"},{"description":"Wire string.","properties":{"kind":{"const":"string","type":"string"},"value":{"description":"Active variant value.","type":"string"}},"required":["kind","value"],"type":"object"},{"description":"Wire integer.","properties":{"kind":{"const":"integer","type":"string"},"value":{"$ref":"#/$defs/OutputDecimalDto","description":"Active variant value."}},"required":["kind","value"],"type":"object"},{"description":"Wire float.","properties":{"kind":{"const":"float","type":"string"},"value":{"description":"Active variant value.","format":"double","type":"number"}},"required":["kind","value"],"type":"object"},{"description":"Wire array.","properties":{"kind":{"const":"array","type":"string"},"value":{"description":"Active variant value.","items":{"$ref":"#/$defs/OutputValueDto"},"type":"array"}},"required":["kind","value"],"type":"object"},{"description":"Wire object.","properties":{"kind":{"const":"object","type":"string"},"value":{"additionalProperties":{"$ref":"#/$defs/OutputValueDto"},"description":"Active variant value.","type":"object"}},"required":["kind","value"],"type":"object"}]},"OutputWireEnvelope":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputAdmissionDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWireEnvelope2":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputCompletionDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWireEnvelope3":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputDispatchDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWireEnvelope4":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputLogSnapshotDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWireEnvelope5":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputLogHealthDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWireEnvelope6":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputLevelChangeDto","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWireEnvelope7":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputClientOutcome","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWireEnvelope8":{"description":"Version-one `WireEnvelope<T>` wire value.","oneOf":[{"description":"Wire ok.","properties":{"kind":{"const":"ok","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"},"value":{"$ref":"#/$defs/OutputClientStatus","description":"Active variant value."}},"required":["kind","schema_version","value"],"type":"object"},{"description":"Wire error.","properties":{"error":{"$ref":"#/$defs/OutputFailure","description":"Active variant error."},"kind":{"const":"error","type":"string"},"schema_version":{"description":"Active variant schema version.","format":"uint32","maximum":1,"minimum":1,"type":"integer"}},"required":["kind","schema_version","error"],"type":"object"}]},"OutputWorkerStateDto":{"description":"Version-one WorkerStateDto wire value.","oneOf":[{"const":"running","description":"Wire running.","type":"string"},{"const":"degraded","description":"Wire degraded.","type":"string"},{"const":"stopped","description":"Wire stopped.","type":"string"}]}},"$id":"https://sc-observability.dev/bindings/v1.json","$schema":"https://json-schema.org/draft/2020-12/schema","x-sc-bindings":{"defaults":{"query_limit":100,"query_order":"oldest_first"},"generic_projections":[{"name":"Result","parameter_ref":"OutputAdmissionDto","source":"OutputResultDtoAdmissionDto"},{"name":"WireEnvelope","parameter_ref":"OutputAdmissionDto","source":"OutputWireEnvelopeAdmissionDto"}],"integer":{"canonical_pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)(?![\\\\s\\\\S])","counter_min":"0","event_min":"-9223372036854775808","max":"18446744073709551615"},"limits":{"container_depth":32,"diagnostic_string_bytes":4096,"query_limit":1000,"remediation_steps":32,"request_bytes":65536,"timeout_ms":60000},"operations":{"change_level":{"input":"InputLevelChangeRequest","output":"OutputWireEnvelopeLevelChangeDto"},"flush":{"input":"InputFlushRequest","output":"OutputWireEnvelopeCompletionDto"},"health":{"input":"InputHealthRequest","output":"OutputWireEnvelopeLogHealthDto"},"query":{"input":"InputQueryRequest","output":"OutputWireEnvelopeLogSnapshotDto"},"try_log":{"input":"InputTryLogRequest","output":"OutputWireEnvelopeAdmissionDto"}},"reserved_field_namespace":"sc_observability.binding.","schema_version":1},"x-sc-entrypoints":{"InputAdmissionDto":{"$ref":"#/$defs/InputAdmissionDto"},"InputAdmissionOperationDto":{"$ref":"#/$defs/InputAdmissionOperationDto"},"InputAggregationTemporalityDto":{"$ref":"#/$defs/InputAggregationTemporalityDto"},"InputAvailabilityDto":{"$ref":"#/$defs/InputAvailabilityDto"},"InputBridgeHealthDto":{"$ref":"#/$defs/InputBridgeHealthDto"},"InputCanonicalDiagnosticDto":{"$ref":"#/$defs/InputCanonicalDiagnosticDto"},"InputCanonicalFailureDto":{"$ref":"#/$defs/InputCanonicalFailureDto"},"InputCanonicalWireEnvelopeAdmissionDto":{"$ref":"#/$defs/InputCanonicalWireEnvelope"},"InputChangeDiagnosticDto":{"$ref":"#/$defs/InputChangeDiagnosticDto"},"InputClientOutcome":{"$ref":"#/$defs/InputClientOutcome"},"InputClientStatus":{"$ref":"#/$defs/InputClientStatus"},"InputCompletionDto":{"$ref":"#/$defs/InputCompletionDto"},"InputCompletionOperationDto":{"$ref":"#/$defs/InputCompletionOperationDto"},"InputDecimalDto":{"$ref":"#/$defs/InputDecimalDto"},"InputDiagnostic":{"$ref":"#/$defs/InputDiagnostic"},"InputDiagnosticSummaryDto":{"$ref":"#/$defs/InputDiagnosticSummaryDto"},"InputDispatchDto":{"$ref":"#/$defs/InputDispatchDto"},"InputDropCountsDto":{"$ref":"#/$defs/InputDropCountsDto"},"InputFailure":{"$ref":"#/$defs/InputFailure"},"InputFailureCountsDto":{"$ref":"#/$defs/InputFailureCountsDto"},"InputFieldMatchDto":{"$ref":"#/$defs/InputFieldMatchDto"},"InputFlushRequest":{"$ref":"#/$defs/InputFlushRequest"},"InputHealthRequest":{"$ref":"#/$defs/InputHealthRequest"},"InputHistogramPointDto":{"$ref":"#/$defs/InputHistogramPointDto"},"InputLevelChangeDto":{"$ref":"#/$defs/InputLevelChangeDto"},"InputLevelChangeRequest":{"$ref":"#/$defs/InputLevelChangeRequest"},"InputLevelChangeSourceDto":{"$ref":"#/$defs/InputLevelChangeSourceDto"},"InputLevelDto":{"$ref":"#/$defs/InputLevelDto"},"InputLevelFilterDto":{"$ref":"#/$defs/InputLevelFilterDto"},"InputLevelRequestDto":{"$ref":"#/$defs/InputLevelRequestDto"},"InputLevelStateDto":{"$ref":"#/$defs/InputLevelStateDto"},"InputLifecycleDto":{"$ref":"#/$defs/InputLifecycleDto"},"InputLogEventDto":{"$ref":"#/$defs/InputLogEventDto"},"InputLogHealthDto":{"$ref":"#/$defs/InputLogHealthDto"},"InputLogOperationDto":{"$ref":"#/$defs/InputLogOperationDto"},"InputLogOrderDto":{"$ref":"#/$defs/InputLogOrderDto"},"InputLogQueryDto":{"$ref":"#/$defs/InputLogQueryDto"},"InputLogSnapshotDto":{"$ref":"#/$defs/InputLogSnapshotDto"},"InputLoggingHealthDto":{"$ref":"#/$defs/InputLoggingHealthDto"},"InputMaintenanceHealthDto":{"$ref":"#/$defs/InputMaintenanceHealthDto"},"InputMetricRecordDto":{"$ref":"#/$defs/InputMetricRecordDto"},"InputMetricValueDto":{"$ref":"#/$defs/InputMetricValueDto"},"InputOperationDiagnosticDto":{"$ref":"#/$defs/InputDiagnostic"},"InputPathDto":{"$ref":"#/$defs/InputPathDto"},"InputProcessIdentityDto":{"$ref":"#/$defs/InputProcessIdentityDto"},"InputQueryHealthDto":{"$ref":"#/$defs/InputQueryHealthDto"},"InputQueryRequest":{"$ref":"#/$defs/InputQueryRequest"},"InputQueryStateDto":{"$ref":"#/$defs/InputQueryStateDto"},"InputRemediationDto":{"$ref":"#/$defs/InputRemediationDto"},"InputResultDtoAdmissionDto":{"$ref":"#/$defs/InputResultDto"},"InputResultDtoClientOutcome":{"$ref":"#/$defs/InputResultDto7"},"InputResultDtoClientStatus":{"$ref":"#/$defs/InputResultDto8"},"InputResultDtoCompletionDto":{"$ref":"#/$defs/InputResultDto2"},"InputResultDtoDispatchDto":{"$ref":"#/$defs/InputResultDto3"},"InputResultDtoLevelChangeDto":{"$ref":"#/$defs/InputResultDto6"},"InputResultDtoLogHealthDto":{"$ref":"#/$defs/InputResultDto5"},"InputResultDtoLogSnapshotDto":{"$ref":"#/$defs/InputResultDto4"},"InputSinkHealthDto":{"$ref":"#/$defs/InputSinkHealthDto"},"InputSpanEventDto":{"$ref":"#/$defs/InputSpanEventDto"},"InputSpanKindDto":{"$ref":"#/$defs/InputSpanKindDto"},"InputSpanLinkDto":{"$ref":"#/$defs/InputSpanLinkDto"},"InputSpanRecordDto":{"$ref":"#/$defs/InputSpanRecordDto"},"InputSpanSignalDto":{"$ref":"#/$defs/InputSpanSignalDto"},"InputSpanStatusDto":{"$ref":"#/$defs/InputSpanStatusDto"},"InputStateTransitionDto":{"$ref":"#/$defs/InputStateTransitionDto"},"InputStoredDiagnosticDto":{"$ref":"#/$defs/InputStoredDiagnosticDto"},"InputStoredEventDto":{"$ref":"#/$defs/InputStoredEventDto"},"InputTraceContextDto":{"$ref":"#/$defs/InputTraceContextDto"},"InputTraceContextV2Dto":{"$ref":"#/$defs/InputTraceContextV2Dto"},"InputTryLogRequest":{"$ref":"#/$defs/InputTryLogRequest"},"InputValueDto":{"$ref":"#/$defs/InputValueDto"},"InputWireEnvelopeAdmissionDto":{"$ref":"#/$defs/InputWireEnvelope"},"InputWireEnvelopeClientOutcome":{"$ref":"#/$defs/InputWireEnvelope7"},"InputWireEnvelopeClientStatus":{"$ref":"#/$defs/InputWireEnvelope8"},"InputWireEnvelopeCompletionDto":{"$ref":"#/$defs/InputWireEnvelope2"},"InputWireEnvelopeDispatchDto":{"$ref":"#/$defs/InputWireEnvelope3"},"InputWireEnvelopeLevelChangeDto":{"$ref":"#/$defs/InputWireEnvelope6"},"InputWireEnvelopeLogHealthDto":{"$ref":"#/$defs/InputWireEnvelope5"},"InputWireEnvelopeLogSnapshotDto":{"$ref":"#/$defs/InputWireEnvelope4"},"InputWorkerStateDto":{"$ref":"#/$defs/InputWorkerStateDto"},"OutputAdmissionDto":{"$ref":"#/$defs/OutputAdmissionDto"},"OutputAdmissionOperationDto":{"$ref":"#/$defs/OutputAdmissionOperationDto"},"OutputAggregationTemporalityDto":{"$ref":"#/$defs/OutputAggregationTemporalityDto"},"OutputAvailabilityDto":{"$ref":"#/$defs/OutputAvailabilityDto"},"OutputBridgeHealthDto":{"$ref":"#/$defs/OutputBridgeHealthDto"},"OutputCanonicalDiagnosticDto":{"$ref":"#/$defs/OutputCanonicalDiagnosticDto"},"OutputCanonicalFailureDto":{"$ref":"#/$defs/OutputCanonicalFailureDto"},"OutputCanonicalWireEnvelopeAdmissionDto":{"$ref":"#/$defs/OutputCanonicalWireEnvelope"},"OutputChangeDiagnosticDto":{"$ref":"#/$defs/OutputChangeDiagnosticDto"},"OutputClientOutcome":{"$ref":"#/$defs/OutputClientOutcome"},"OutputClientStatus":{"$ref":"#/$defs/OutputClientStatus"},"OutputCompletionDto":{"$ref":"#/$defs/OutputCompletionDto"},"OutputCompletionOperationDto":{"$ref":"#/$defs/OutputCompletionOperationDto"},"OutputDecimalDto":{"$ref":"#/$defs/OutputDecimalDto"},"OutputDiagnostic":{"$ref":"#/$defs/OutputDiagnostic"},"OutputDiagnosticSummaryDto":{"$ref":"#/$defs/OutputDiagnosticSummaryDto"},"OutputDispatchDto":{"$ref":"#/$defs/OutputDispatchDto"},"OutputDropCountsDto":{"$ref":"#/$defs/OutputDropCountsDto"},"OutputFailure":{"$ref":"#/$defs/OutputFailure"},"OutputFailureCountsDto":{"$ref":"#/$defs/OutputFailureCountsDto"},"OutputFieldMatchDto":{"$ref":"#/$defs/OutputFieldMatchDto"},"OutputFlushRequest":{"$ref":"#/$defs/OutputFlushRequest"},"OutputHealthRequest":{"$ref":"#/$defs/OutputHealthRequest"},"OutputHistogramPointDto":{"$ref":"#/$defs/OutputHistogramPointDto"},"OutputLevelChangeDto":{"$ref":"#/$defs/OutputLevelChangeDto"},"OutputLevelChangeRequest":{"$ref":"#/$defs/OutputLevelChangeRequest"},"OutputLevelChangeSourceDto":{"$ref":"#/$defs/OutputLevelChangeSourceDto"},"OutputLevelDto":{"$ref":"#/$defs/OutputLevelDto"},"OutputLevelFilterDto":{"$ref":"#/$defs/OutputLevelFilterDto"},"OutputLevelRequestDto":{"$ref":"#/$defs/OutputLevelRequestDto"},"OutputLevelStateDto":{"$ref":"#/$defs/OutputLevelStateDto"},"OutputLifecycleDto":{"$ref":"#/$defs/OutputLifecycleDto"},"OutputLogEventDto":{"$ref":"#/$defs/OutputLogEventDto"},"OutputLogHealthDto":{"$ref":"#/$defs/OutputLogHealthDto"},"OutputLogOperationDto":{"$ref":"#/$defs/OutputLogOperationDto"},"OutputLogOrderDto":{"$ref":"#/$defs/OutputLogOrderDto"},"OutputLogQueryDto":{"$ref":"#/$defs/OutputLogQueryDto"},"OutputLogSnapshotDto":{"$ref":"#/$defs/OutputLogSnapshotDto"},"OutputLoggingHealthDto":{"$ref":"#/$defs/OutputLoggingHealthDto"},"OutputMaintenanceHealthDto":{"$ref":"#/$defs/OutputMaintenanceHealthDto"},"OutputMetricRecordDto":{"$ref":"#/$defs/OutputMetricRecordDto"},"OutputMetricValueDto":{"$ref":"#/$defs/OutputMetricValueDto"},"OutputOperationDiagnosticDto":{"$ref":"#/$defs/OutputDiagnostic"},"OutputPathDto":{"$ref":"#/$defs/OutputPathDto"},"OutputProcessIdentityDto":{"$ref":"#/$defs/OutputProcessIdentityDto"},"OutputQueryHealthDto":{"$ref":"#/$defs/OutputQueryHealthDto"},"OutputQueryRequest":{"$ref":"#/$defs/OutputQueryRequest"},"OutputQueryStateDto":{"$ref":"#/$defs/OutputQueryStateDto"},"OutputRemediationDto":{"$ref":"#/$defs/OutputRemediationDto"},"OutputResultDtoAdmissionDto":{"$ref":"#/$defs/OutputResultDto"},"OutputResultDtoClientOutcome":{"$ref":"#/$defs/OutputResultDto7"},"OutputResultDtoClientStatus":{"$ref":"#/$defs/OutputResultDto8"},"OutputResultDtoCompletionDto":{"$ref":"#/$defs/OutputResultDto2"},"OutputResultDtoDispatchDto":{"$ref":"#/$defs/OutputResultDto3"},"OutputResultDtoLevelChangeDto":{"$ref":"#/$defs/OutputResultDto6"},"OutputResultDtoLogHealthDto":{"$ref":"#/$defs/OutputResultDto5"},"OutputResultDtoLogSnapshotDto":{"$ref":"#/$defs/OutputResultDto4"},"OutputSinkHealthDto":{"$ref":"#/$defs/OutputSinkHealthDto"},"OutputSpanEventDto":{"$ref":"#/$defs/OutputSpanEventDto"},"OutputSpanKindDto":{"$ref":"#/$defs/OutputSpanKindDto"},"OutputSpanLinkDto":{"$ref":"#/$defs/OutputSpanLinkDto"},"OutputSpanRecordDto":{"$ref":"#/$defs/OutputSpanRecordDto"},"OutputSpanSignalDto":{"$ref":"#/$defs/OutputSpanSignalDto"},"OutputSpanStatusDto":{"$ref":"#/$defs/OutputSpanStatusDto"},"OutputStateTransitionDto":{"$ref":"#/$defs/OutputStateTransitionDto"},"OutputStoredDiagnosticDto":{"$ref":"#/$defs/OutputStoredDiagnosticDto"},"OutputStoredEventDto":{"$ref":"#/$defs/OutputStoredEventDto"},"OutputTraceContextDto":{"$ref":"#/$defs/OutputTraceContextDto"},"OutputTraceContextV2Dto":{"$ref":"#/$defs/OutputTraceContextV2Dto"},"OutputTryLogRequest":{"$ref":"#/$defs/OutputTryLogRequest"},"OutputValueDto":{"$ref":"#/$defs/OutputValueDto"},"OutputWireEnvelopeAdmissionDto":{"$ref":"#/$defs/OutputWireEnvelope"},"OutputWireEnvelopeClientOutcome":{"$ref":"#/$defs/OutputWireEnvelope7"},"OutputWireEnvelopeClientStatus":{"$ref":"#/$defs/OutputWireEnvelope8"},"OutputWireEnvelopeCompletionDto":{"$ref":"#/$defs/OutputWireEnvelope2"},"OutputWireEnvelopeDispatchDto":{"$ref":"#/$defs/OutputWireEnvelope3"},"OutputWireEnvelopeLevelChangeDto":{"$ref":"#/$defs/OutputWireEnvelope6"},"OutputWireEnvelopeLogHealthDto":{"$ref":"#/$defs/OutputWireEnvelope5"},"OutputWireEnvelopeLogSnapshotDto":{"$ref":"#/$defs/OutputWireEnvelope4"},"OutputWorkerStateDto":{"$ref":"#/$defs/OutputWorkerStateDto"}},"x-sc-error-registry":[{"code":"SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID","kind":"validation","remediation":"Enter and close each scope once in LIFO order on its originating thread and task"},{"code":"SC_OBSERVABILITY_BINDING_INVALID_INPUT","kind":"validation","remediation":"Correct the named input field and submit a new request"},{"code":"SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION","kind":"unsupported_version","remediation":"Install client and host packages supporting the same schema"},{"code":"SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE","kind":"validation","remediation":"Reduce remote diagnostic text or remediation steps to the documented bounds"},{"code":"SC_OBSERVABILITY_BINDING_CLOSED","kind":"closed","remediation":"Stop submitting through the closed backend and inspect its retained health"},{"code":"SC_OBSERVABILITY_BINDING_DISPATCH_FULL","kind":"queue_full","remediation":"Wait for an outstanding request to complete before submitting again"},{"code":"SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS","kind":"queue_full","remediation":"Wait for the current adapter flush to finish before submitting another"},{"code":"SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED","kind":"unavailable","remediation":"Restore native thread resources before explicitly creating another backend"},{"code":"SC_OBSERVABILITY_BINDING_WAITERS_FULL","kind":"queue_full","remediation":"Wait for an existing operation observer to finish before registering another"},{"code":"SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS","kind":"queue_full","remediation":"Wait for the existing query to finish before starting another"},{"code":"SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED","kind":"unavailable","remediation":"Install a host backend before requesting an attached logger"},{"code":"SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED","kind":"unavailable","remediation":"Reuse the module\'s existing backend; replacement is unsupported"},{"code":"SC_OBSERVABILITY_BINDING_PERMISSION_DENIED","kind":"permission_denied","remediation":"Request access through the application\'s authorized window"},{"code":"SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE","kind":"unavailable","remediation":"Restore the host connection before submitting a new request"},{"code":"SC_OBSERVABILITY_BINDING_TIMEOUT","kind":"timeout","remediation":"Inspect operation status before deciding whether another operation is needed"},{"code":"SC_OBSERVABILITY_BINDING_CANCELLED","kind":"cancelled","remediation":"Inspect the saved operation result if confirmation is still needed"},{"code":"SC_OBSERVABILITY_PY_HANDLER_REENTRANT","kind":"internal","remediation":"Remove logging calls from handler formatting and error callbacks"},{"code":"SC_OBSERVABILITY_BINDING_INTERNAL","kind":"internal","remediation":"Inspect the retained status and restore the affected host or client"}]}')
CLASSES = {'InputAdmissionDto#0': InputAdmissionAccepted, 'InputAdmissionDto#1': InputAdmissionFiltered, 'InputBridgeHealthDto': InputBridgeHealth, 'InputCanonicalDiagnosticDto': InputCanonicalDiagnostic, 'InputCanonicalFailureDto#0': InputCanonicalFailureValidation, 'InputCanonicalFailureDto#1': InputCanonicalFailureQueueFull, 'InputCanonicalFailureDto#2': InputCanonicalFailureBelowBaseline, 'InputCanonicalFailureDto#3': InputCanonicalFailureUnsupportedLevel, 'InputCanonicalFailureDto#4': InputCanonicalFailurePermissionDenied, 'InputCanonicalFailureDto#5': InputCanonicalFailureClosed, 'InputCanonicalFailureDto#6': InputCanonicalFailureUnavailable, 'InputCanonicalFailureDto#7': InputCanonicalFailureIo, 'InputCanonicalFailureDto#8': InputCanonicalFailureTimeout, 'InputCanonicalFailureDto#9': InputCanonicalFailureCancelled, 'InputCanonicalFailureDto#10': InputCanonicalFailureUnsupportedVersion, 'InputCanonicalFailureDto#11': InputCanonicalFailureInternal, 'InputCanonicalFailureDto#12': InputCanonicalFailureUnknownRemote, 'InputCanonicalWireEnvelope#0': InputCanonicalWireEnvelopeOk, 'InputCanonicalWireEnvelope#1': InputCanonicalWireEnvelopeError, 'InputChangeDiagnosticDto#0': InputChangeDiagnosticAccepted, 'InputChangeDiagnosticDto#1': InputChangeDiagnosticNotAccepted, 'InputClientOutcome#0': InputClientOutcomeIdle, 'InputClientOutcome#1': InputClientOutcomeScheduled, 'InputClientOutcome#2': InputClientOutcomeAccepted, 'InputClientOutcome#3': InputClientOutcomeFiltered, 'InputClientOutcome#4': InputClientOutcomeCompleted, 'InputClientStatus': InputClientStatus, 'InputCompletionDto#0': InputCompletionCompleted, 'InputDiagnostic': InputDiagnostic, 'InputDiagnosticSummaryDto': InputDiagnosticSummary, 'InputDispatchDto#0': InputDispatchScheduled, 'InputDropCountsDto': InputDropCounts, 'InputFailure#0': InputFailureValidation, 'InputFailure#1': InputFailureQueueFull, 'InputFailure#2': InputFailureBelowBaseline, 'InputFailure#3': InputFailureUnsupportedLevel, 'InputFailure#4': InputFailurePermissionDenied, 'InputFailure#5': InputFailureClosed, 'InputFailure#6': InputFailureUnavailable, 'InputFailure#7': InputFailureIo, 'InputFailure#8': InputFailureTimeout, 'InputFailure#9': InputFailureCancelled, 'InputFailure#10': InputFailureUnsupportedVersion, 'InputFailure#11': InputFailureInternal, 'InputFailure#12': InputFailureUnknownRemote, 'InputFailureCountsDto': InputFailureCounts, 'InputFieldMatchDto': InputFieldMatch, 'InputFlushRequest': InputFlushRequest, 'InputHealthRequest': InputHealthRequest, 'InputHistogramPointDto': InputHistogramPoint, 'InputLevelChangeDto#0': InputLevelChangeChanged, 'InputLevelChangeDto#1': InputLevelChangeUnchanged, 'InputLevelChangeRequest': InputLevelChangeRequest, 'InputLevelRequestDto#0': InputLevelRequestElevate, 'InputLevelRequestDto#1': InputLevelRequestReset, 'InputLevelStateDto': InputLevelState, 'InputLogEventDto': InputLogEvent, 'InputLogHealthDto': InputLogHealth, 'InputLogQueryDto': InputLogQuery, 'InputLogSnapshotDto': InputLogSnapshot, 'InputLoggingHealthDto': InputLoggingHealth, 'InputMaintenanceHealthDto': InputMaintenanceHealth, 'InputMetricRecordDto': InputMetricRecord, 'InputMetricValueDto#0': InputMetricValueGauge, 'InputMetricValueDto#1': InputMetricValueSum, 'InputMetricValueDto#2': InputMetricValueHistogram, 'InputPathDto#0': InputPathUtf8, 'InputPathDto#1': InputPathUnrepresentable, 'InputPathDto#2': InputPathAbsent, 'InputProcessIdentityDto': InputProcessIdentity, 'InputQueryHealthDto': InputQueryHealth, 'InputQueryRequest': InputQueryRequest, 'InputRemediationDto#0': InputRemediationRecoverable, 'InputRemediationDto#1': InputRemediationNotRecoverable, 'InputResultDto#0': InputResultOk, 'InputResultDto#1': InputResultError, 'InputResultDto2#0': InputResult2Ok, 'InputResultDto2#1': InputResult2Error, 'InputResultDto3#0': InputResult3Ok, 'InputResultDto3#1': InputResult3Error, 'InputResultDto4#0': InputResult4Ok, 'InputResultDto4#1': InputResult4Error, 'InputResultDto5#0': InputResult5Ok, 'InputResultDto5#1': InputResult5Error, 'InputResultDto6#0': InputResult6Ok, 'InputResultDto6#1': InputResult6Error, 'InputResultDto7#0': InputResult7Ok, 'InputResultDto7#1': InputResult7Error, 'InputResultDto8#0': InputResult8Ok, 'InputResultDto8#1': InputResult8Error, 'InputSinkHealthDto': InputSinkHealth, 'InputSpanEventDto': InputSpanEvent, 'InputSpanLinkDto': InputSpanLink, 'InputSpanRecordDto': InputSpanRecord, 'InputSpanSignalDto#0': InputSpanSignal0, 'InputSpanSignalDto#1': InputSpanSignal1, 'InputSpanSignalDto#2': InputSpanSignal2, 'InputStateTransitionDto': InputStateTransition, 'InputStoredDiagnosticDto': InputStoredDiagnostic, 'InputStoredEventDto': InputStoredEvent, 'InputTraceContextDto': InputTraceContext, 'InputTraceContextV2Dto': InputTraceContextV2, 'InputTryLogRequest': InputTryLogRequest, 'InputValueDto#0': InputValueNull, 'InputValueDto#1': InputValueBoolean, 'InputValueDto#2': InputValueString, 'InputValueDto#3': InputValueInteger, 'InputValueDto#4': InputValueFloat, 'InputValueDto#5': InputValueArray, 'InputValueDto#6': InputValueObject, 'InputWireEnvelope#0': InputWireEnvelopeOk, 'InputWireEnvelope#1': InputWireEnvelopeError, 'InputWireEnvelope2#0': InputWireEnvelope2Ok, 'InputWireEnvelope2#1': InputWireEnvelope2Error, 'InputWireEnvelope3#0': InputWireEnvelope3Ok, 'InputWireEnvelope3#1': InputWireEnvelope3Error, 'InputWireEnvelope4#0': InputWireEnvelope4Ok, 'InputWireEnvelope4#1': InputWireEnvelope4Error, 'InputWireEnvelope5#0': InputWireEnvelope5Ok, 'InputWireEnvelope5#1': InputWireEnvelope5Error, 'InputWireEnvelope6#0': InputWireEnvelope6Ok, 'InputWireEnvelope6#1': InputWireEnvelope6Error, 'InputWireEnvelope7#0': InputWireEnvelope7Ok, 'InputWireEnvelope7#1': InputWireEnvelope7Error, 'InputWireEnvelope8#0': InputWireEnvelope8Ok, 'InputWireEnvelope8#1': InputWireEnvelope8Error, 'OutputAdmissionDto#0': OutputAdmissionAccepted, 'OutputAdmissionDto#1': OutputAdmissionFiltered, 'OutputBridgeHealthDto': OutputBridgeHealth, 'OutputCanonicalDiagnosticDto': OutputCanonicalDiagnostic, 'OutputCanonicalFailureDto#0': OutputCanonicalFailureValidation, 'OutputCanonicalFailureDto#1': OutputCanonicalFailureQueueFull, 'OutputCanonicalFailureDto#2': OutputCanonicalFailureBelowBaseline, 'OutputCanonicalFailureDto#3': OutputCanonicalFailureUnsupportedLevel, 'OutputCanonicalFailureDto#4': OutputCanonicalFailurePermissionDenied, 'OutputCanonicalFailureDto#5': OutputCanonicalFailureClosed, 'OutputCanonicalFailureDto#6': OutputCanonicalFailureUnavailable, 'OutputCanonicalFailureDto#7': OutputCanonicalFailureIo, 'OutputCanonicalFailureDto#8': OutputCanonicalFailureTimeout, 'OutputCanonicalFailureDto#9': OutputCanonicalFailureCancelled, 'OutputCanonicalFailureDto#10': OutputCanonicalFailureUnsupportedVersion, 'OutputCanonicalFailureDto#11': OutputCanonicalFailureInternal, 'OutputCanonicalFailureDto#12': OutputCanonicalFailureUnknownRemote, 'OutputCanonicalWireEnvelope#0': OutputCanonicalWireEnvelopeOk, 'OutputCanonicalWireEnvelope#1': OutputCanonicalWireEnvelopeError, 'OutputChangeDiagnosticDto#0': OutputChangeDiagnosticAccepted, 'OutputChangeDiagnosticDto#1': OutputChangeDiagnosticNotAccepted, 'OutputClientOutcome#0': OutputClientOutcomeIdle, 'OutputClientOutcome#1': OutputClientOutcomeScheduled, 'OutputClientOutcome#2': OutputClientOutcomeAccepted, 'OutputClientOutcome#3': OutputClientOutcomeFiltered, 'OutputClientOutcome#4': OutputClientOutcomeCompleted, 'OutputClientStatus': OutputClientStatus, 'OutputCompletionDto#0': OutputCompletionCompleted, 'OutputDiagnostic': OutputDiagnostic, 'OutputDiagnosticSummaryDto': OutputDiagnosticSummary, 'OutputDispatchDto#0': OutputDispatchScheduled, 'OutputDropCountsDto': OutputDropCounts, 'OutputFailure#0': OutputFailureValidation, 'OutputFailure#1': OutputFailureQueueFull, 'OutputFailure#2': OutputFailureBelowBaseline, 'OutputFailure#3': OutputFailureUnsupportedLevel, 'OutputFailure#4': OutputFailurePermissionDenied, 'OutputFailure#5': OutputFailureClosed, 'OutputFailure#6': OutputFailureUnavailable, 'OutputFailure#7': OutputFailureIo, 'OutputFailure#8': OutputFailureTimeout, 'OutputFailure#9': OutputFailureCancelled, 'OutputFailure#10': OutputFailureUnsupportedVersion, 'OutputFailure#11': OutputFailureInternal, 'OutputFailure#12': OutputFailureUnknownRemote, 'OutputFailureCountsDto': OutputFailureCounts, 'OutputFieldMatchDto': OutputFieldMatch, 'OutputFlushRequest': OutputFlushRequest, 'OutputHealthRequest': OutputHealthRequest, 'OutputHistogramPointDto': OutputHistogramPoint, 'OutputLevelChangeDto#0': OutputLevelChangeChanged, 'OutputLevelChangeDto#1': OutputLevelChangeUnchanged, 'OutputLevelChangeRequest': OutputLevelChangeRequest, 'OutputLevelRequestDto#0': OutputLevelRequestElevate, 'OutputLevelRequestDto#1': OutputLevelRequestReset, 'OutputLevelStateDto': OutputLevelState, 'OutputLogEventDto': OutputLogEvent, 'OutputLogHealthDto': OutputLogHealth, 'OutputLogQueryDto': OutputLogQuery, 'OutputLogSnapshotDto': OutputLogSnapshot, 'OutputLoggingHealthDto': OutputLoggingHealth, 'OutputMaintenanceHealthDto': OutputMaintenanceHealth, 'OutputMetricRecordDto': OutputMetricRecord, 'OutputMetricValueDto#0': OutputMetricValueGauge, 'OutputMetricValueDto#1': OutputMetricValueSum, 'OutputMetricValueDto#2': OutputMetricValueHistogram, 'OutputPathDto#0': OutputPathUtf8, 'OutputPathDto#1': OutputPathUnrepresentable, 'OutputPathDto#2': OutputPathAbsent, 'OutputProcessIdentityDto': OutputProcessIdentity, 'OutputQueryHealthDto': OutputQueryHealth, 'OutputQueryRequest': OutputQueryRequest, 'OutputRemediationDto#0': OutputRemediationRecoverable, 'OutputRemediationDto#1': OutputRemediationNotRecoverable, 'OutputResultDto#0': OutputResultOk, 'OutputResultDto#1': OutputResultError, 'OutputResultDto2#0': OutputResult2Ok, 'OutputResultDto2#1': OutputResult2Error, 'OutputResultDto3#0': OutputResult3Ok, 'OutputResultDto3#1': OutputResult3Error, 'OutputResultDto4#0': OutputResult4Ok, 'OutputResultDto4#1': OutputResult4Error, 'OutputResultDto5#0': OutputResult5Ok, 'OutputResultDto5#1': OutputResult5Error, 'OutputResultDto6#0': OutputResult6Ok, 'OutputResultDto6#1': OutputResult6Error, 'OutputResultDto7#0': OutputResult7Ok, 'OutputResultDto7#1': OutputResult7Error, 'OutputResultDto8#0': OutputResult8Ok, 'OutputResultDto8#1': OutputResult8Error, 'OutputSinkHealthDto': OutputSinkHealth, 'OutputSpanEventDto': OutputSpanEvent, 'OutputSpanLinkDto': OutputSpanLink, 'OutputSpanRecordDto': OutputSpanRecord, 'OutputSpanSignalDto#0': OutputSpanSignal0, 'OutputSpanSignalDto#1': OutputSpanSignal1, 'OutputSpanSignalDto#2': OutputSpanSignal2, 'OutputStateTransitionDto': OutputStateTransition, 'OutputStoredDiagnosticDto': OutputStoredDiagnostic, 'OutputStoredEventDto': OutputStoredEvent, 'OutputTraceContextDto': OutputTraceContext, 'OutputTraceContextV2Dto': OutputTraceContextV2, 'OutputTryLogRequest': OutputTryLogRequest, 'OutputValueDto#0': OutputValueNull, 'OutputValueDto#1': OutputValueBoolean, 'OutputValueDto#2': OutputValueString, 'OutputValueDto#3': OutputValueInteger, 'OutputValueDto#4': OutputValueFloat, 'OutputValueDto#5': OutputValueArray, 'OutputValueDto#6': OutputValueObject, 'OutputWireEnvelope#0': OutputWireEnvelopeOk, 'OutputWireEnvelope#1': OutputWireEnvelopeError, 'OutputWireEnvelope2#0': OutputWireEnvelope2Ok, 'OutputWireEnvelope2#1': OutputWireEnvelope2Error, 'OutputWireEnvelope3#0': OutputWireEnvelope3Ok, 'OutputWireEnvelope3#1': OutputWireEnvelope3Error, 'OutputWireEnvelope4#0': OutputWireEnvelope4Ok, 'OutputWireEnvelope4#1': OutputWireEnvelope4Error, 'OutputWireEnvelope5#0': OutputWireEnvelope5Ok, 'OutputWireEnvelope5#1': OutputWireEnvelope5Error, 'OutputWireEnvelope6#0': OutputWireEnvelope6Ok, 'OutputWireEnvelope6#1': OutputWireEnvelope6Error, 'OutputWireEnvelope7#0': OutputWireEnvelope7Ok, 'OutputWireEnvelope7#1': OutputWireEnvelope7Error, 'OutputWireEnvelope8#0': OutputWireEnvelope8Ok, 'OutputWireEnvelope8#1': OutputWireEnvelope8Error}
def name_of(ref):return ref.rsplit('/',1)[-1]

def validate(schema, node, value, path='$'):
    import math
    if node is True:return
    if node is False:raise ValueError(f'{path}: forbidden')
    if '$ref' in node:validate(schema,schema['$defs'][name_of(node['$ref'])],value,path)
    if node.get('x-sc-integer-domain')=='unsigned' and (not isinstance(value,str) or not value.isascii() or not value.isdecimal()):raise ValueError(f'{path}: expected unsigned decimal')
    if 'const' in node and (type(value)!=type(node['const']) or value!=node['const']):raise ValueError(f'{path}: const')
    if 'enum' in node and value not in node['enum']:raise ValueError(f'{path}: enum')
    for key in ('oneOf','anyOf'):
        if key in node:
            matches=0
            for candidate in node[key]:
                try:validate(schema,candidate,value,path);matches+=1
                except ValueError:pass
            if not matches or (key=='oneOf' and matches!=1):raise ValueError(f'{path}: {key}')
    for candidate in node.get('allOf',[]):validate(schema,candidate,value,path)
    typ=node.get('type')
    if isinstance(typ,list):
        for candidate in typ:
            try:validate(schema,{**node,'type':candidate},value,path);return
            except ValueError:pass
        raise ValueError(f'{path}: type')
    if typ=='null' and value is not None:raise ValueError(f'{path}: null')
    if typ=='boolean' and type(value) is not bool:raise ValueError(f'{path}: boolean')
    if typ=='string':
        if not isinstance(value,str):raise ValueError(f'{path}: string')
        if 'pattern' in node and re.fullmatch(node['pattern'],value) is None:raise ValueError(f'{path}: pattern')
        if 'pattern' in node and '0|[1-9]' in node['pattern'] and not -(2**63)<=int(value)<2**64:raise ValueError(f'{path}: integer range')
        if not node.get('minLength',0)<=len(value)<=node.get('maxLength',float('inf')):raise ValueError(f'{path}: string length')
    if typ in ('integer','number'):
        if type(value) not in (int,float) or (type(value) is float and not math.isfinite(value)) or (typ=='integer' and type(value) is not int):raise ValueError(f'{path}: number')
        if not node.get('minimum',-float('inf'))<=value<=node.get('maximum',float('inf')):raise ValueError(f'{path}: range')
    if typ=='array':
        if not isinstance(value,list):raise ValueError(f'{path}: array')
        if not node.get('minItems',0)<=len(value)<=node.get('maxItems',float('inf')):raise ValueError(f'{path}: array length')
        for i,item in enumerate(value):validate(schema,node['items'],item,f'{path}[{i}]')
    if typ=='object':
        if not isinstance(value,dict) or any(not isinstance(k,str) for k in value):raise ValueError(f'{path}: object')
        if not set(node.get('required',[]))<=set(value):raise ValueError(f'{path}: missing field')
        for key,item in value.items():
            if key in node.get('properties',{}):validate(schema,node['properties'][key],item,f'{path}.{key}')
            else:validate(schema,node.get('additionalProperties',True),item,f'{path}.{key}')


def validate_wire(name: str, value: object) -> None:
    validate(SCHEMA, SCHEMA['x-sc-entrypoints'][name], value)

def _decode(node, value, name=None):
    if '$ref' in node:
        name=name_of(node['$ref']);return _decode(SCHEMA['$defs'][name],value,name)
    if name and name.endswith('DecimalDto'):return int(value)
    for key in ('oneOf','anyOf'):
        if key in node:
            for i,child in enumerate(node[key]):
                try:validate(SCHEMA,child,value)
                except ValueError:continue
                return _decode(child,value,f'{name}#{i}' if name else None)
    if value is None:return None
    if node.get('type')=='array':return tuple(_decode(node['items'],v) for v in value)
    if node.get('type')=='object':
        props=node.get('properties',{})
        result={k:_decode(props.get(k,node.get('additionalProperties',{})),v) for k,v in value.items() if not props or k in props}
        cls=CLASSES.get(name)
        if cls:
            for k,s in props.items():
                if k not in result:result[k]=_decode(s,s.get('default')) if 'default' in s else None
            return cls(**{k:v for k,v in result.items() if k!='kind'})
        return MappingProxyType(result)
    return value

def from_wire(name: str, value: object) -> object:
    """Validate a wire entrypoint then create immutable ergonomic values."""
    validate_wire(name,value)
    return _decode(SCHEMA['x-sc-entrypoints'][name],value)

ERROR_REGISTRY = tuple(MappingProxyType(entry) for entry in SCHEMA["x-sc-error-registry"])
SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID = 'SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID'
SC_OBSERVABILITY_BINDING_INVALID_INPUT = 'SC_OBSERVABILITY_BINDING_INVALID_INPUT'
SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION = 'SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION'
SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE = 'SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE'
SC_OBSERVABILITY_BINDING_CLOSED = 'SC_OBSERVABILITY_BINDING_CLOSED'
SC_OBSERVABILITY_BINDING_DISPATCH_FULL = 'SC_OBSERVABILITY_BINDING_DISPATCH_FULL'
SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS = 'SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS'
SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED = 'SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED'
SC_OBSERVABILITY_BINDING_WAITERS_FULL = 'SC_OBSERVABILITY_BINDING_WAITERS_FULL'
SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS = 'SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS'
SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED = 'SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED'
SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED = 'SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED'
SC_OBSERVABILITY_BINDING_PERMISSION_DENIED = 'SC_OBSERVABILITY_BINDING_PERMISSION_DENIED'
SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE = 'SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE'
SC_OBSERVABILITY_BINDING_TIMEOUT = 'SC_OBSERVABILITY_BINDING_TIMEOUT'
SC_OBSERVABILITY_BINDING_CANCELLED = 'SC_OBSERVABILITY_BINDING_CANCELLED'
SC_OBSERVABILITY_PY_HANDLER_REENTRANT = 'SC_OBSERVABILITY_PY_HANDLER_REENTRANT'
SC_OBSERVABILITY_BINDING_INTERNAL = 'SC_OBSERVABILITY_BINDING_INTERNAL'
