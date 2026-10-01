from __future__ import annotations

import json
from unittest.mock import patch

import unittest

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr


class _NativeTelemetry:
    def emit(self, _payload: str) -> str:
        return json.dumps({"kind": "ok", "value": {"submission_id": "018f01aa-0000-7000-8000-000000000001"}})

    def flush(self, _timeout: int | None) -> str:
        return json.dumps({"kind": "ok", "value": {"delivered": {"logs": 1}}})

    def flush_submission(self, _submission_id: str, _timeout: int | None) -> str:
        return self.flush(_timeout)

    def shutdown(self, _timeout: int | None) -> str:
        return self.flush(_timeout)

    def status(self, query: str) -> str:
        return json.dumps({"kind": "ok", "value": json.loads(query)})


def test_emit_returns_tagged_result_without_raising_for_submission_failure() -> None:
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
            return None, json.dumps({"kind": "error", "error": {"kind": "config", "variant": "missing_field", "code": "SC_OBSERVABILITY_CONFIG_MISSING", "message": "store_path is required", "path": None, "report": None}})

    with patch("sc_observability.telemetry.importlib.import_module", return_value=NativeModule()):
        result = Telemetry.open()
    assert isinstance(result, TelemetryErr)
    assert result.error.kind == "config"
