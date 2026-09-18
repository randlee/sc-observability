"""Strict type-check fixture for the public tagged Result contract."""
from __future__ import annotations

from typing import NoReturn

from sc_observability import Err, Ok, Result
from sc_observability import generated
from sc_observability.generated import OutputFailure


def _unreachable(value: NoReturn) -> NoReturn:
    raise AssertionError(f"unreachable failure: {value}")


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


def exhaustively_narrow_failure(error: OutputFailure) -> str:
    """Keep every generated Failure arm narrowed under Python 3.10 typing."""
    match error:
        case generated.OutputFailureValidation(field=field):
            return field
        case generated.OutputFailureQueueFull():
            return error.code
        case generated.OutputFailureBelowBaseline(requested=requested, configured=configured):
            return f"{requested}:{configured}"
        case generated.OutputFailureUnsupportedLevel(requested=requested, available=available):
            return f"{requested}:{available}"
        case generated.OutputFailurePermissionDenied():
            return error.code
        case generated.OutputFailureClosed():
            return error.code
        case generated.OutputFailureUnavailable():
            return error.code
        case generated.OutputFailureIo():
            return error.code
        case generated.OutputFailureInternal():
            return error.code
        case generated.OutputFailureTimeout(operation=operation):
            return operation
        case generated.OutputFailureCancelled(operation=operation):
            return operation
        case generated.OutputFailureUnsupportedVersion(received=received):
            return str(received)
        case generated.OutputFailureUnknownRemote(remote_kind=remote_kind):
            return remote_kind
        case unreachable:
            return _unreachable(unreachable)
