"""Strict typing fixture for the public telemetry tagged-result contract."""
from __future__ import annotations

from pathlib import Path
from typing import NoReturn

from sc_observability import Err, Ok, Result, Telemetry


def _unreachable(value: NoReturn) -> NoReturn:
    raise AssertionError(f"unreachable telemetry result: {value}")


def render(result: Result[None]) -> str:
    match result:
        case Ok():
            return "exported"
        case Err(error=error):
            return f"{error.kind}:{error.code}:{error.message}"
        case unreachable:
            return _unreachable(unreachable)


def use_facade(certificate: Path) -> list[str]:
    telemetry = Telemetry("http://127.0.0.1:4318", headers={"authorization": "Bearer token"},
                          timeout_s=1.5, root_certificate=certificate, service_name="typed")
    return [
        render(telemetry.log("started", severity="warn", attributes={"attempt": 2, "ok": True})),
        render(telemetry.span("build", kind="client", start_time_unix_nano=1, end_time_unix_nano=2,
                              error="timed out", attributes={"ratio": 0.5})),
        render(telemetry.metric("jobs", "up_down_counter", -1, unit="1", attributes={"queue": "default"})),
    ]
