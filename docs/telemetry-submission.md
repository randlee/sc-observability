# Telemetry submission

Wave 5 telemetry uses a durable local store before it attempts OTLP delivery.
Submission is therefore **at least once**: after an interrupted or uncertain
delivery the retained record may be sent again when the process recovers.

## Python

The installed wheel returns tagged results rather than raising delivery errors.
Handle `Ok` and `TelemetryErr` explicitly, and retain the shutdown result when
the context exits.

```python
from sc_observability import Ok
from sc_observability.telemetry import Telemetry

opened = Telemetry.open(config="telemetry.yaml")
if not isinstance(opened, Ok):
    print(opened.error.code)
else:
    with opened.value as telemetry:
        submitted = telemetry.emit({"version": 1, "logs": [{"body": "started"}]})
        if isinstance(submitted, Ok):
            telemetry.flush_submission(submitted.value.submission_id, timeout_s=10)
    # `last_shutdown` is an Ok FlushReport or a TelemetryErr; it is never raised.
    print(opened.value.last_shutdown)
```

## CLI

`sc-otel` accepts the same `SubmissionInput` document as the Python facade.

```sh
sc-otel --config telemetry.yaml validate --stdin < event.json
sc-otel --config telemetry.yaml emit --stdin < event.json
sc-otel --config telemetry.yaml flush --timeout 10
sc-otel --config telemetry.yaml status
```

| Exit code | Meaning |
| --- | --- |
| 0 | The command completed and all selected records were delivered or validated. |
| 1 | Unexpected internal error; inspect stderr for the diagnostic. |
| 2 | Command-line usage error. |
| 3 | Submission validation error; inspect the JSON `error.code`. |
| 4 | Configuration or unsupported-combination error. |
| 5 | Admission failed before a durable record was created. |
| 6 | The submission was admitted but delivery did not complete; retain the store and run `flush` after recovery. |
| 7 | A signal exhausted its delivery attempts and was retained as a terminal failure. |

## Configuration precedence

For both front ends, explicit `store_path` / `--store` and `endpoint` /
`--endpoint` values take precedence over `telemetry.yaml`. The endpoint then
falls back to `OTEL_EXPORTER_OTLP_ENDPOINT`; `store_path` has no environment
fallback and must come from an explicit value or `store.path` in the file.
`store.path` is resolved relative to the configuration file, keeping each
application's durable queue isolated.

The supported signal and backend combinations are listed in the
[OTLP capability matrix](architecture.md#otlp-capability-matrix).
The pinned desktop viewer is a readback tool for its supported logs, spans,
Gauge, Sum, and explicit Histogram forms; profiles, Summary, exemplars, and
ExponentialHistogram are verified against collector capture instead.
