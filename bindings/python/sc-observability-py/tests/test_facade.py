from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "python"))

from sc_observability import Err, LogEvent, LoggerConfig, Ok, create_logger
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
