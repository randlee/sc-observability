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

from os import PathLike
from sc_observability import DeliveryStatus, FlushReport, LeaseInfo, Telemetry


def use_facade(path: PathLike[str], telemetry: Telemetry) -> None:
    Telemetry.open(path, store_path=path)
    with telemetry:
        telemetry.flush()
    shutdown: TelemetryResult[FlushReport] | None = telemetry.last_shutdown
    status = telemetry.status()
    if isinstance(status, Ok):
        lease: LeaseInfo | None = status.value.lease
        rows: tuple[DeliveryStatus, ...] = status.value.submissions
        if lease is not None:
            print(lease.holder, lease.expires_at, rows, shutdown)
