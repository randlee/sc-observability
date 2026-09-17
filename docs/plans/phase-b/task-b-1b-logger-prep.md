---
id: B.1b-logger-prep
status: in_progress
branch: feature/phase-b-1b-logger-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1b-logger-prep
parent: fix/phase-b-1-provenance-integrity
authoritative-sprint-doc: sprint-b-1b-logger-errors.md
contract: error-api-contract.md
---

# B.1b logger preparation — typed logger operations and sink interoperability

This task plan was reconstructed at task start because it was absent from the
verified parent. The named sprint document and error API contract remain the
authoritative requirements.

## Scope

1. Add neutral `LogFailure` and `TryLogFailure` values with lossless conversion
   to the retained logger admission errors.
2. Add typed logger and builder construction, admission, and flush methods.
   The typed path is the common production implementation; legacy methods
   adapt its results while preserving `emit`'s conditional flush behavior.
3. Add `sc_observability::typed::TypedLogSink` and explicit adapters, then
   move built-in sink write/flush production paths to that one implementation.
4. Add focused old/new parity, adapter, source-preservation, construction, and
   queue/filter/owner fixtures; record commands and outcomes in the handoff.

## Boundaries

Do not alter existing public logger signatures, representations, bridge
contracts, control/lifecycle behavior, warning policy, provenance tooling, or
neutral-fixture tests owned by other layers. No B.1 copy, publication, or
warning activation is part of this task.

## Validation checklist

- `cargo fmt --all -- --check`
- `cargo test --locked -p sc-observability --all-targets`
- workspace doctests and clippy
- public API/docs review and bridge regression evidence
- two passes: contract/API completeness, then Rust behavior/source integrity

## Fixture matrix

| Required behavior | Typed/legacy evidence |
| --- | --- |
| startup validation and native spawn source | `logger_builder_rejects_zero_queue_capacity`, `typed_builder_rejects_zero_queue_capacity_with_the_same_diagnostic`, `owner_construction_returns_the_injected_writer_start_source`, `typed_owner_construction_preserves_the_injected_writer_start_source` |
| invalid event and filtering | `invalid_event_returns_event_error`, `typed_logger_admission_preserves_filtering_and_invalid_event_failure` (typed invalid schema, mismatched service, Filtered, and Accepted) |
| queue, owner, and level concurrency | `try_log_reports_queue_full_on_saturated_queue` (legacy and typed assertions), `level_owner_changes_only_its_logger_and_filters_with_shared_admission`, `admission_and_level_mutation_contend_on_one_control_state` |
| flush, timeout, and shutdown | `flush_failures_propagate_and_are_counted_in_health` (legacy and typed assertions), `shutdown_records_join_timeout_but_waits_for_join`, `emit_path_remains_available_during_maintenance_pass` |
| built-in sink, maintenance, and fault behavior | `file_and_console_fan_out_both_receive_event`, sink maintenance/write tests, retained fault-injector tests under `--all-features` |
| adapters and source preservation | `typed_sink_adapters_preserve_default_flush_health_and_single_write`, `typed_sink_adapters_preserve_explicit_failure_source_health_and_call_counts`, `logger_admission_conversions_keep_the_original_source` |
| typed admission with concurrent control work | `typed_admission_and_flush_can_run_concurrently`, plus the retained shutdown timeout fixture; post-shutdown admission is not externally constructible because `Logger::shutdown(self)` consumes the sole running handle into `Logger<Stopped>` |

The copied-bridge suite remains an explicit B.1 integration dependency; no
bridge source or behavior is changed by this preparation layer.
