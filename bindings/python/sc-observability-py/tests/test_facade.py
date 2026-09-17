from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "python"))

from sc_observability import Err, LogEvent, LoggerConfig, create_logger
from sc_observability import _event


def test_ergonomic_event_preserves_exact_integers() -> None:
    encoded = _event(
        LogEvent(
            level="info",
            target="python.test",
            action="emit",
            fields={"minimum": -(2**63), "maximum": 2**64 - 1},
        )
    )

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
    try:
        _event(encoded)
    except ValueError as error:
        assert "reserved binding provenance" in str(error)
    else:
        raise AssertionError("forged provenance was accepted")
