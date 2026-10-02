from os import PathLike
from sc_observability.telemetry import Telemetry, TelemetryResult, _factory

def open_test_double(*, store_path: str | PathLike[str], endpoint: str, service_name: str,
                     script_json: str | None = None) -> TelemetryResult[Telemetry]:
    return _factory(Telemetry, "_open_test_double", None, store_path, endpoint, service_name, script_json)
