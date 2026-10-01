"""Strict typing fixture for the public telemetry tagged-result contract."""
from __future__ import annotations

from typing import NoReturn

from sc_observability import Ok
from sc_observability.telemetry import TelemetryErr, TelemetryResult, build_envelope


def _unreachable(value: NoReturn) -> NoReturn:
    raise AssertionError(f"unreachable telemetry result: {value}")


def render(result: TelemetryResult[str]) -> str:
    match result:
        case Ok(value=value):
            return value
        case TelemetryErr(error=error):
            return f"{error.kind}:{error.code}"
        case unreachable:
            return _unreachable(unreachable)


def build(document: dict[str, object]) -> str:
    result = build_envelope(document)
    return render(result)
