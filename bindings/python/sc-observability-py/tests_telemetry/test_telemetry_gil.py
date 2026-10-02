from pathlib import Path
import threading
from sc_observability import Ok
from telemetry_test_support import open_test_double


def test_flush_releases_the_gil_while_native_flush_is_blocked(tmp_path: Path) -> None:
    result = open_test_double(store_path=tmp_path / "store", endpoint="http://127.0.0.1:4318", service_name="gil")
    assert isinstance(result, Ok)
    telemetry = result.value
    native = telemetry._native
    native._gate_arm()
    completed = threading.Event()
    results = []

    def flush() -> None:
        try:
            results.append(telemetry.flush())
        finally:
            completed.set()

    worker = threading.Thread(target=flush, daemon=True)
    worker.start()
    try:
        assert native._gate_wait_entered(), "native flush never entered the gate"
        # This Python assertion must run before release, while flush still owns the gate.
        # Removing py.detach forces the native watchdog to expire before Python runs.
        assert native._gate_is_blocked(), "Python made no progress while native flush was blocked"
        assert not completed.is_set()
    finally:
        native._gate_release()
        worker.join(timeout=6)
    assert not worker.is_alive()
    assert completed.is_set()
    assert len(results) == 1 and isinstance(results[0], Ok)
