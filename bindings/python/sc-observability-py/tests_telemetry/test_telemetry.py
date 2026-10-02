from __future__ import annotations

import json
from unittest.mock import patch

import unittest

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr


class _NativeTelemetry:
    def emit(self, _payload: str) -> str:
        return json.dumps({"kind": "ok", "value": {"submission_id": "018f01aa-0000-7000-8000-000000000001", "record_key": None, "admitted_at": "2026-10-01T00:00:00Z", "signals": ["logs"], "duplicate": False}})

    def flush(self, _timeout: int | None) -> str:
        return json.dumps({"kind": "ok", "value": {name: {signal: 0 for signal in ("logs", "traces", "metrics", "profiles")} for name in ("delivered", "still_pending", "failed", "evicted")}})

    def flush_submission(self, _submission_id: str, _timeout: int | None) -> str:
        return self.flush(_timeout)

    def shutdown(self, _timeout: int | None) -> str:
        return self.flush(_timeout)

    def status(self, query: str) -> str:
        return json.dumps({"kind": "ok", "value": json.loads(query)})


def test_emit_decodes_a_successful_native_admission() -> None:
    telemetry = Telemetry(_NativeTelemetry())
    result = telemetry.emit({"version": 1, "signals": {}})
    assert isinstance(result, Ok)
    assert result.value.submission_id.endswith("0001")


def test_programmer_misuse_is_limited_to_mapping_and_selector_errors() -> None:
    telemetry = Telemetry(_NativeTelemetry())
    with unittest.TestCase().assertRaisesRegex(TypeError, "Mapping"):
        telemetry.emit([])  # type: ignore[arg-type]
    with unittest.TestCase().assertRaisesRegex(ValueError, "not both"):
        telemetry.status(submissions=("018f01aa-0000-7000-8000-000000000001",), record_keys=("record",))


def test_context_manager_records_shutdown_and_never_suppresses() -> None:
    telemetry = Telemetry(_NativeTelemetry())
    with telemetry:
        pass
    assert isinstance(telemetry.last_shutdown, Ok)


def test_open_decodes_expected_native_failure() -> None:
    class NativeModule:
        @staticmethod
        def open(_args: str) -> tuple[None, str]:
            return None, json.dumps({"kind": "error", "error": {"kind": "config", "variant": "missing_field", "code": "SC_OBSERVABILITY_TELEMETRY_CONFIG_MISSING", "message": "store_path is required", "path": None, "report": None, "remediation": None, "cause": None}})

    with patch("sc_observability.telemetry.importlib.import_module", return_value=NativeModule()):
        result = Telemetry.open()
    assert isinstance(result, TelemetryErr)
    assert result.error.kind == "config"


import pytest
from sc_observability.telemetry import _result, _flush_report, _store_status, LeaseInfo, DeliveryStatus

@pytest.mark.parametrize("payload", [None, {}, {"kind": "future"}, {"kind": "ok"},
    {"kind": "ok", "value": {}}, {"kind": "ok", "value": {"delivered": {"logs": "1"}}},
    {"kind": "error", "error": {"kind": "invented"}}, {"kind": "error", "error": {"kind": "config"}}])
def test_malformed_native_payload_is_an_internal_decode_failure(payload: object) -> None:
    result = _result(payload, _flush_report)
    assert isinstance(result, TelemetryErr)
    assert (result.error.kind, result.error.variant) == ("internal", "decode")

@pytest.mark.parametrize("method,args", [("emit", ({"version": 1},)), ("flush", ()),
    ("flush_submission", ("bad",)), ("shutdown", ()), ("status", ())])
@pytest.mark.parametrize("failure", [RuntimeError("native detail"), ValueError("bad JSON")])
def test_operational_native_failures_are_contained(method: str, args: tuple, failure: Exception) -> None:
    class Broken:
        def __getattr__(self, name: str):
            def call(*args):
                raise failure
            return call
    telemetry = Telemetry(Broken())
    result = getattr(telemetry, method)(*args)
    assert isinstance(result, TelemetryErr)
    assert result.error.kind == "internal"
    assert str(failure) in result.error.cause
    with telemetry:
        pass
    assert isinstance(telemetry.last_shutdown, TelemetryErr)

@pytest.mark.parametrize("exception", [KeyboardInterrupt, SystemExit])
def test_process_control_exceptions_are_not_swallowed(exception: type[BaseException]) -> None:
    class Interrupted:
        def flush(self, timeout):
            raise exception()
    with pytest.raises(exception):
        Telemetry(Interrupted()).flush()


def test_nested_status_is_typed_and_immutable() -> None:
    counts = {name: 0 for name in ("logs", "traces", "metrics", "profiles")}
    source = {name: 0 for name in ("schema_version", "store_bytes", "max_store_bytes", "evicted_by_disk_bound", "rejected_by_disk_bound", "unreadable_newer_envelopes")}
    source.update({name: counts for name in ("pending", "retry_scheduled", "delivered_retained", "failed")})
    source.update(lease={"holder": "worker", "expires_at": "2026-10-01T00:00:00Z"},
        submissions=[{"submission_id": "id", "signals": [["logs", {"state": "delivered", "at": "2026-10-01T00:00:00Z", "attempts": 1}]]}])
    result = _store_status(source)
    assert isinstance(result.lease, LeaseInfo)
    assert isinstance(result.submissions[0], DeliveryStatus)
    with pytest.raises(TypeError):
        result.submissions[0].signals[0][1]["attempts"] = 2
    del source["lease"]["holder"]
    malformed = _result({"kind": "ok", "value": source}, _store_status)
    assert isinstance(malformed, TelemetryErr)
