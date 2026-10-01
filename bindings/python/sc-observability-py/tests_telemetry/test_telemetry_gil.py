from __future__ import annotations

import json
from pathlib import Path
import threading

from sc_observability import Ok
from sc_observability.telemetry import Telemetry


GOLDENS = Path(__file__).parents[4] / "crates/sc-observability-types/tests/fixtures/otlp_submission/golden"


def test_flush_releases_the_gil_for_the_test_double(tmp_path: Path) -> None:
    result = Telemetry._with_test_double(
        store_path=tmp_path / "store", endpoint="http://127.0.0.1:4318", service_name="d30-gil",
        script_json=json.dumps({"flush_delay_ms": 500}),
    )
    assert isinstance(result, Ok)
    telemetry = result.value
    input_document = json.loads((GOLDENS / "logs" / "input.json").read_text())
    assert isinstance(telemetry.emit(input_document), Ok)
    started = threading.Event()
    progressed = threading.Event()

    def worker() -> None:
        started.wait()
        progressed.set()

    thread = threading.Thread(target=worker)
    thread.start()
    started.set()
    telemetry.flush()
    thread.join()
    assert progressed.is_set()
