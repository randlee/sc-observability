"""Strict type-check fixture for the public tagged Result contract."""
from __future__ import annotations

from sc_observability import Err, Ok, Result


def render(result: Result[str]) -> str:
    if isinstance(result, Ok):
        return result.value
    assert isinstance(result, Err)
    return f"{result.error.kind}:{result.error.code}"


def accepted(result: Result[bool]) -> bool:
    match result:
        case Ok(value=value):
            return value
        case Err(error=error):
            return error.kind == "queue_full"
