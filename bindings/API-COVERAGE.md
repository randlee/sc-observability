# Binding API coverage

## Neutral schema v1

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

The neutral DTO layer carries no ErrorContext, source chain, backtrace, Logger,
LogGuard, LevelOwner, or transport/native ownership capability. Its source-bundle
proof is prepublication and does not claim registry-only installation. Runtime,
transport, and language behavior is covered by the following layers.

## TypeScript and Tauri

| Surface | Evidence |
| --- | --- |
| Generated schema/types and runtime validation | `bindings/typescript/src/generated/index.ts`; `scripts/ci/validate_binding_schema.sh` |
| `encodeValue` null, boolean, string, finite float, safe integer, bigint, arrays and objects | `bindings/typescript/src/encoding.ts`; `bindings/typescript/src/test.ts` |
| BigInt min/max, unsafe numbers, negative zero, cycles, depth, size, symbols, getters and protected keys | `bindings/typescript/src/test.ts` and `bindings/tauri/src/lib.rs` boundary tests |
| `createClient`, `log`, `tryLog`, `query`, `health`, `client_status`, `flush` | `bindings/typescript/src/client.ts`; `bindings/typescript/src/test.ts` |
| Fixed in-flight dispatch and failure counters | `bindings/typescript/src/client.ts` (`reserve`, `recordFailure`); client status smoke assertions |
| Tauri request validation, authorization, target allowlist and redaction | `bindings/tauri/src/lib.rs` strict-request, policy, target and recursive-redaction paths/tests |
| Exact IPC command names and host-owned lifecycle | `bindings/tauri/src/lib.rs`; `examples/tauri-logging/src-tauri/src/main.rs` |
| Real consumer transport and ergonomic event | `examples/tauri-logging/src/main.ts`; `examples/tauri-logging/README.md` |
| Installation, package and adapter gates | `scripts/ci/validate_typescript_bindings.sh` (packed tarball installed outside the checkout, packaged Rust adapter, actual desktop IPC, and strict platform aggregate) |

The native backend remains the shared B.3b `HostLoggingBackend`; this layer
does not duplicate core conversion or logger ownership.

## Python synchronous and asynchronous surfaces

| Surface | Evidence |
| --- | --- |
| Owned factory, explicit lifecycle, attached host, log/query/health/levels/flush | `python/sc_observability/__init__.py`; `tests/test_runtime.py`; actual `examples/rust-python-logging` host |
| Handler installation, formatter failures, recursion and context inheritance | `tests/test_logging_context.py`; mixed Rust/Python host |
| `Logger.submit` and `AttachedLogger.submit` | `tests/test_async_logging.py` single admission and preparation failures; `tests/test_async_runtime.py` actual stored events/context/ignored returns |
| `LogReceipt.state`, `Resolved`, `ReceiptState`, `LogReceipt.wait` | `tests/test_async_logging.py` immediate saved result, zero/max/invalid timeout; `tests/typing/test_async_narrowing.py` exhaustive narrowing |
| `Logger.flush_async` and `AttachedLogger.flush_async` | `tests/test_async_logging.py` cancellation, timeout, native rejection and all Failure categories; `examples/rust-python-logging/src/async_conformance.{rs,py}` actual owned/core/bridge held-writer heartbeat and overlap |
| Shared 64-observer cap and atomic permit release | `tests/test_async_runtime.py` synchronized native permit race; real embedded 64 native-completed/unpolled observations with 65th rejected before native submission |
| Closed loops, ignored receipts and interpreter finalization | Async runtime tests; actual core/bridge finalization subprocesses release native work after Python has finalized |
| Python 3.10 typing, examples, installed wheels and native embedding | `scripts/ci/validate_python_bindings.sh`; `qualification-suite.json`; sole distribution workflow `.github/workflows/b4a-python-distributions.yml` |

Python paths in this section are relative to `bindings/python/sc-observability-py`,
except the repository-level Rust embedding example and CI paths. Per-platform
qualification results are recorded in sprint handoffs; this inventory describes
coverage without claiming an unexecuted matrix pass.

## TypeScript/Tauri artifact qualification

| Surface | Executable qualification evidence |
| --- | --- |
| Production application `requestLevelChange` and shared `parseWireEnvelope` | Unchanged example source staged into both `frontend.js` and `faults.mjs`; actual IPC, every Failure payload and foreign invoke/response getter checks |
| Installed npm client, all canonical schema fixtures, remote failures and local accounting | `scripts/ci/fixtures/tauri-qualification/faults.mjs`; retained `fault-results.json` |
| Exhaustive Result/Failure/Value/Remediation/level narrowing | External packed consumer `scripts/ci/fixtures/tauri-qualification/narrowing.ts` |
| Real command transport, bigint stored/query round trip, correlated Rust/frontend, redaction/provenance | Real desktop webview `scripts/ci/fixtures/tauri-qualification/frontend.js` |
| Five-command secondary-window denial, direct-invoke policy and exact-size normalization | Same frontend fixture; retained main/forbidden records in `ipc.json` |
| Bridge level/health coherence, repeat/reduce/reset/Off rejection, owner contention | Same frontend fixture plus bounded owner observation hook in `qualification.rs` |
| Actual queue saturation, rejected diagnostic with successful level change, retained query/flush slots, responsive blocked I/O | Real console sink held by `_tauri_webview.py`; unchanged supplied native backend and IPC handlers |
| Packaged Rust adapter, exact normalized dependency requirements/registry checksums, denied checkout/cache/network | `build_binding_source_bundle.py`; `validate_tauri_qualification.py`; retained archive, lock, manifest and raw command evidence |
| Supported platform completeness and tamper rejection | `validate_tauri_platform_evidence.py`; `bindings-typescript.yml` matrix and aggregate |

Qualification remains in progress until the final three-platform aggregate and
lead completeness check. Checkpoint `71215ca` passes Linux/macOS CI and local macOS with the
same canonical npm/Rust artifacts: 258 ordinary real IPC assertions, 20 capped
release-host IPC assertions, 368 installed-client/helper cases and 15 policy
cases. Windows remains unresolved. The complete case inventory, including real owner/shutdown
contention, zero-timeout native diagnostics, late shutdown completion and
retained post-stop health, is committed in
`scripts/ci/fixtures/tauri-qualification/required-evidence-cases.json`.
The worktree checklist and `handoff-b-3a-qualification.md` retain the exact gates
and evidence locations without claiming an unfinished platform matrix passed.
