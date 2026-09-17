from __future__ import annotations

import sys
from pathlib import Path
from typing import Any, cast

sys.path.insert(0, str(Path(__file__).parents[1] / "python"))

from sc_observability import AttachedLogger, Err, LogEvent, Logger, LoggerConfig, LogQuery, Ok, create_logger
from sc_observability import _event, _timeout


def test_ergonomic_event_preserves_exact_integers() -> None:
    result = _event(
        LogEvent(
            level="info",
            target="python.test",
            action="emit",
            fields={"minimum": -(2**63), "maximum": 2**64 - 1},
        )
    )

    assert isinstance(result, Ok)
    encoded = result.value
    assert encoded["schema_version"] == 1
    assert encoded["fields"]["minimum"] == {"kind": "integer", "value": str(-(2**63))}
    assert encoded["fields"]["maximum"] == {"kind": "integer", "value": str(2**64 - 1)}


def test_forged_provenance_is_rejected_before_native_import() -> None:
    invalid_config = create_logger(LoggerConfig(service="test", log_root=""))
    assert isinstance(invalid_config, Err)
    encoded = LogEvent(
        level="info",
        target="python.test",
        action="emit",
        fields={"sc_observability::binding::language": "forged"},
    )
    rejected = _event(encoded)
    assert isinstance(rejected, Err)
    assert rejected.error.code == "SC_OBSERVABILITY_BINDING_INVALID_INPUT"
    assert "reserved binding provenance" in rejected.error.message


def test_wrong_timeout_is_a_tagged_validation_result() -> None:
    rejected = _timeout(True)
    assert isinstance(rejected, Err)
    assert rejected.error.code == "SC_OBSERVABILITY_BINDING_INVALID_INPUT"


def test_cycle_and_wrong_event_type_are_tagged_validation_results() -> None:
    cycle: list[object] = []
    cycle.append(cycle)
    cyclic = _event(LogEvent(level="info", target="python.test", action="emit", fields={"cycle": cycle}))
    assert isinstance(cyclic, Err)
    assert cyclic.error.code == "SC_OBSERVABILITY_BINDING_INVALID_INPUT"

    wrong_type = _event(object())
    assert isinstance(wrong_type, Err)
    assert wrong_type.error.code == "SC_OBSERVABILITY_BINDING_INVALID_INPUT"


class _NeverNative:
    """Makes an unexpected public-wrapper dispatch immediately visible."""

    def log(self, payload: str) -> str:
        raise AssertionError(f"native log was called with {payload}")

    def query(self, payload: str) -> str:
        raise AssertionError(f"native query was called with {payload}")

    def health(self) -> str:
        raise AssertionError("native health was called")

    def flush(self, timeout: str) -> str:
        raise AssertionError(f"native flush was called with {timeout}")

    def shutdown(self, timeout: str) -> str:
        raise AssertionError(f"native shutdown was called with {timeout}")

    def wait_stopped(self, timeout: str) -> str:
        raise AssertionError(f"native wait_stopped was called with {timeout}")

    def elevate_level(self, level: str, source: str) -> str:
        raise AssertionError(f"native elevate_level was called with {level}/{source}")

    def reset_level(self, source: str) -> str:
        raise AssertionError(f"native reset_level was called with {source}")


def test_public_input_failures_are_tagged_before_native_dispatch() -> None:
    owned = Logger(_NeverNative())
    attached = AttachedLogger(_NeverNative())
    forged_event = LogEvent(
        level="info",
        target="python.test",
        action="emit",
        fields={"sc_observability::binding::language": "forged"},
    )

    rejected = (
        owned.log(forged_event),
        attached.log(forged_event),
        owned.query(cast(Any, object())),
        attached.query(cast(Any, object())),
        owned.flush(True),
        attached.flush(True),
        owned.shutdown(-1),
        owned.wait_stopped(60_001),
        owned.elevate_level(cast(Any, "invalid")),
        owned.elevate_level("info", cast(Any, "invalid")),
        owned.reset_level(cast(Any, "invalid")),
    )
    assert all(isinstance(result, Err) for result in rejected)
