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

def validate_wire(name: str, value: object) -> None: ...
def from_wire(name: str, value: object) -> object: ...
SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID: str
SC_OBSERVABILITY_BINDING_INVALID_INPUT: str
SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION: str
SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE: str
SC_OBSERVABILITY_BINDING_CLOSED: str
SC_OBSERVABILITY_BINDING_DISPATCH_FULL: str
SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS: str
SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED: str
SC_OBSERVABILITY_BINDING_WAITERS_FULL: str
SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS: str
SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED: str
SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED: str
SC_OBSERVABILITY_BINDING_PERMISSION_DENIED: str
SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE: str
SC_OBSERVABILITY_BINDING_TIMEOUT: str
SC_OBSERVABILITY_BINDING_CANCELLED: str
SC_OBSERVABILITY_PY_HANDLER_REENTRANT: str
SC_OBSERVABILITY_BINDING_INTERNAL: str
