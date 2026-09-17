from __future__ import annotations

import sys
from pathlib import Path
from typing import Any, cast

sys.path.insert(0, str(Path(__file__).parents[1] / "python"))

import sc_observability
from sc_observability import (
    AttachedLogger,
    Err,
    LogEvent,
    Logger,
    LoggerConfig,
    LogQuery,
    Ok,
    create_logger,
    get_host_logger,
)
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


class _UnprintableForeignError(Exception):
    def __str__(self) -> str:
        raise RuntimeError("exception rendering must not escape")


class _ForeignNative:
    def log(self, payload: str) -> str:
        raise _UnprintableForeignError()

    def query(self, payload: str) -> str:
        raise _UnprintableForeignError()

    def health(self) -> str:
        raise _UnprintableForeignError()

    def flush(self, timeout: str) -> str:
        raise _UnprintableForeignError()

    def shutdown(self, timeout: str) -> str:
        raise _UnprintableForeignError()

    def wait_stopped(self, timeout: str) -> str:
        raise _UnprintableForeignError()

    def elevate_level(self, level: str, source: str) -> str:
        raise _UnprintableForeignError()

    def reset_level(self, source: str) -> str:
        raise _UnprintableForeignError()


class _HostileMapping(dict[str, object]):
    def items(self) -> object:
        raise _UnprintableForeignError()


class _MalformedEntriesMapping(dict[str, object]):
    def items(self) -> object:
        return [1]


class _ExplosiveEvent(LogEvent):
    def __getattribute__(self, name: str) -> object:
        if name == "level":
            raise _UnprintableForeignError()
        return super().__getattribute__(name)


class _LookupExplodes:
    def __getattribute__(self, name: str) -> object:
        if name == "health":
            raise _UnprintableForeignError()
        return super().__getattribute__(name)


def test_foreign_native_and_mapping_failures_are_tagged() -> None:
    owned = Logger(_ForeignNative())
    attached = AttachedLogger(_ForeignNative())
    valid_event = LogEvent(level="info", target="python.test", action="emit")

    results = (
        owned.log(valid_event),
        attached.log(valid_event),
        owned.query(LogQuery()),
        attached.query(LogQuery()),
        owned.health(),
        attached.health(),
        owned.flush(),
        attached.flush(),
        owned.shutdown(),
        owned.wait_stopped(),
        owned.elevate_level("debug"),
        owned.reset_level(),
        _event(LogEvent(level="info", target="python.test", action="emit", fields=_HostileMapping())),
        _event(LogEvent(level="info", target="python.test", action="emit", fields=_MalformedEntriesMapping())),
        _event(_ExplosiveEvent(level="info", target="python.test", action="emit")),
        Logger(cast(Any, _LookupExplodes())).health(),
    )
    assert all(isinstance(result, Err) for result in results)
    assert all(
        result.error.code
        in ("SC_OBSERVABILITY_BINDING_INTERNAL", "SC_OBSERVABILITY_BINDING_INVALID_INPUT")
        for result in results
    )


def test_malformed_runtime_dataclass_values_are_tagged() -> None:
    assert isinstance(create_logger(cast(Any, LoggerConfig("service", cast(Any, 42)))), Err)
    logger = Logger(_NeverNative())
    assert isinstance(logger.query(cast(Any, LogQuery(field_matches=cast(Any, None)))), Err)
    assert isinstance(logger.query(cast(Any, LogQuery(levels=cast(Any, None)))), Err)
    assert isinstance(
        logger.log(cast(Any, LogEvent(level="info", target=cast(Any, 42), action="emit"))),
        Err,
    )


class _MalformedNative:
    def log(self, payload: str) -> None:
        return None

    def query(self, payload: str) -> None:
        return None

    def health(self) -> None:
        return None

    def flush(self, timeout: str) -> None:
        return None

    def shutdown(self, timeout: str) -> None:
        return None

    def wait_stopped(self, timeout: str) -> None:
        return None

    def elevate_level(self, level: str, source: str) -> None:
        return None

    def reset_level(self, source: str) -> None:
        return None


class _BrokenExtension:
    def create_owned(self, payload: str) -> None:
        raise _UnprintableForeignError()

    def get_installed_host_logger(self) -> None:
        raise _UnprintableForeignError()


def test_malformed_native_results_and_factory_failures_are_tagged(monkeypatch: pytest.MonkeyPatch) -> None:
    owned = Logger(_MalformedNative())
    attached = AttachedLogger(_MalformedNative())
    event = LogEvent(level="info", target="python.test", action="emit")
    results = (
        owned.log(event),
        attached.log(event),
        owned.query(LogQuery()),
        attached.query(LogQuery()),
        owned.health(),
        attached.health(),
        owned.flush(),
        attached.flush(),
        owned.shutdown(),
        owned.wait_stopped(),
        owned.elevate_level("debug"),
        owned.reset_level(),
    )
    assert all(isinstance(result, Err) for result in results)

    monkeypatch.setattr(sc_observability.importlib, "import_module", lambda _: _BrokenExtension())
    assert isinstance(create_logger(LoggerConfig("service", "/tmp/factory")), Err)
    assert isinstance(get_host_logger(), Err)
