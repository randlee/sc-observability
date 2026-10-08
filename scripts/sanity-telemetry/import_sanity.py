"""Import the repository's sanity and QA JSONL histories as OpenTelemetry signals.

Each source row becomes OTLP/HTTP log, span and gauge exports made through the
synchronous ``sc_observability.telemetry.Telemetry`` client.  Every call exports
before it returns; nothing is stored, retried or deduplicated by the client.  The
importer advances a source's file checkpoint only after every export for a row
returned ``Ok``, or after the row was rejected as invalid, so the checkpoint is
the only guard against sending a row twice.  An export or internal failure stops
that source without advancing its checkpoint, so the row is sent again by the
next run.

Exit status is 0 for a clean import, 1 for counted failures, and 2 for
configuration or command-usage failures.
"""
from __future__ import annotations

import argparse
import calendar
import hashlib
import json
import os
import re
import tempfile
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from time import sleep
from typing import Any, Iterable, Mapping

import yaml

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr

SOURCE_KINDS = ("sanity", "qa", "finding-counts")


def _timestamp(value: object) -> str | None:
    """Return the canonical nanosecond UTC representation without inventing time."""
    if not isinstance(value, str):
        return None
    match = re.fullmatch(
        r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?Z",
        value,
    )
    if match is None:
        return None
    head, fraction = match.groups()
    try:
        # fromisoformat validates calendar and clock ranges; the regex preserves
        # all nine fractional digits instead of silently truncating nanoseconds.
        datetime.fromisoformat(head + "+00:00")
    except ValueError:
        return None
    return f"{head}.{(fraction or '').ljust(9, '0')}Z"


def _unix_nanos(canonical: str) -> int:
    """Convert a ``_timestamp`` result to exact integer nanoseconds since the epoch."""
    head, fraction = canonical[:-1].split(".")
    seconds = calendar.timegm(datetime.fromisoformat(head).timetuple())
    return seconds * 1_000_000_000 + int(fraction)


def _attrs(**values: object) -> dict[str, object]:
    return {name: value for name, value in values.items() if value is not None}


def _reviewer(source: "Source", row: Mapping[str, object]) -> str | None:
    reviewer = row.get("reviewer")
    if not isinstance(reviewer, str) or not reviewer:
        reviewer = source.reviewer
    if not reviewer and source.kind == "sanity":
        reviewer = "sanity-llm"
    return reviewer


def _checkpoint_key(source: "Source") -> str:
    """Keep source cursors distinct by the kind, path and configured reviewer."""
    return json.dumps([source.kind, source.path, source.reviewer or ""], separators=(",", ":"))


def _trace_id(run_id: object) -> str | None:
    """Derive one shared 32-digit lowercase hex trace id per non-empty run id."""
    if not isinstance(run_id, str) or not run_id:
        return None
    trace = hashlib.sha256(run_id.encode()).hexdigest()[:32]
    return None if trace == "0" * 32 else trace


@dataclass(frozen=True)
class Source:
    path: str
    kind: str
    phase: str | None = None
    reviewer: str | None = None


@dataclass(frozen=True)
class Call:
    """One ``Telemetry`` method call: ``method`` is ``log``, ``span`` or ``metric``."""

    method: str
    args: tuple[Any, ...]
    kwargs: dict[str, Any]


@dataclass
class Report:
    exported: int = 0
    skipped_invalid: int = 0
    failures: list[tuple[str, str]] = field(default_factory=list)


def _summary(report: Report) -> str:
    return json.dumps({"exported": report.exported, "skipped_invalid": report.skipped_invalid,
                       "failures": report.failures})


def load_config(path: Path) -> dict[str, object]:
    try:
        value = yaml.safe_load(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, yaml.YAMLError) as error:
        raise ValueError("could not read importer configuration") from error
    if not isinstance(value, dict):
        raise ValueError("importer configuration must be a mapping")
    if "service" in value and value["service"] is not None and not isinstance(value["service"], str):
        raise ValueError("service must be a string")
    _endpoint(value)
    sources(value)
    return value


def _endpoint(config: Mapping[str, object]) -> str | None:
    """Return the configured OTLP/HTTP base URL; absent means the client's default."""
    otlp = config.get("otlp")
    if otlp is None:
        return None
    if not isinstance(otlp, Mapping):
        raise ValueError("otlp must be a mapping")
    endpoint = otlp.get("endpoint")
    if endpoint is not None and (not isinstance(endpoint, str) or not endpoint):
        raise ValueError("otlp.endpoint must be a non-empty string")
    return endpoint


def sources(config: Mapping[str, object]) -> list[Source]:
    raw = config.get("sources", [])
    if not isinstance(raw, list):
        raise ValueError("sources must be a list")
    result: list[Source] = []
    seen_paths: set[str] = set()
    for item in raw:
        if (
            not isinstance(item, Mapping)
            or not isinstance(item.get("path"), str)
            or not item["path"]
            or not isinstance(item.get("kind"), str)
            or item["kind"] not in SOURCE_KINDS
            or ("phase" in item and item["phase"] is not None and not isinstance(item["phase"], str))
            or ("reviewer" in item and item["reviewer"] is not None and not isinstance(item["reviewer"], str))
        ):
            raise ValueError("each source needs a path and a supported kind")
        path = item["path"]
        if path in seen_paths:
            raise ValueError("source paths must be unique")
        seen_paths.add(path)
        result.append(Source(path, item["kind"], item.get("phase"), item.get("reviewer")))
    return result


def _common(source: Source, row: Mapping[str, object], config: Mapping[str, object]) -> dict[str, object]:
    reviewer = _reviewer(source, row)
    error = row.get("error") if isinstance(row.get("error"), Mapping) else None
    attrs = _attrs(
        **{
            "team": config.get("team"),
            "phase": row.get("phase") or source.phase,
            "review.reviewer": reviewer,
            "review.verdict": row.get("verdict"),
            "vcs.commit.sha": row.get("commit"),
            "review.task": row.get("task"),
            "review.sprint": row.get("sprint"),
            "review.iteration": row.get("iteration"),
            "review.duration_s": row.get("duration_seconds"),
            "review.duration": row.get("duration"),
            "qa.tested": row.get("tested"),
            "review.findings": row.get("findings") if row.get("findings") is not None else row.get("fnd"),
            "qa.blocking": row.get("blk"),
            "qa.important": row.get("imp"),
            "qa.minor": row.get("min"),
            "qa.correction": True if row.get("correction") else None,
            "qa.corrects": row.get("corrects"),
            "review.error.code": error.get("code") if error is not None else None,
            "review.error.message": error.get("message") if error is not None else None,
        }
    )
    pr = row.get("pr_number")
    if isinstance(pr, int):
        template = config.get("github", {})
        attrs["vcs.pr.number"] = pr
        url_template = template.get("pr_url_template") if isinstance(template, Mapping) else None
        if isinstance(url_template, str) and url_template:
            attrs["pr.url"] = url_template.replace("{pr_number}", str(pr))
    return attrs


def map_row(source: Source, row: Mapping[str, object], config: Mapping[str, object]) -> list[Call] | None:
    """Map one source row to its ``Telemetry`` calls; invalid mandatory fields produce none.

    Every row yields one log whose ``event.time`` attribute carries the source
    time.  A sanity row with ``started_at`` also yields a ``sanity.review`` span;
    a finding-counts row yields ``sc.qa.findings.open``/``total`` gauges.
    """
    if source.kind not in SOURCE_KINDS:
        return None
    time = _timestamp(row.get("completed_at") or row.get("snapshot_at"))
    verdict = row.get("verdict")
    if time is None or (source.kind != "finding-counts" and not isinstance(verdict, str)):
        return None
    attrs = _common(source, row, config)
    body = "finding-counts" if source.kind == "finding-counts" else str(verdict)
    calls = [Call("log", (body,), {"attributes": {**attrs, "event.time": time}})]
    if source.kind == "finding-counts":
        for name, metric in (("open", "sc.qa.findings.open"), ("tot", "sc.qa.findings.total")):
            value = row.get(name)
            if isinstance(value, int) and not isinstance(value, bool):
                calls.append(Call("metric", (metric, "gauge", float(value)), {"attributes": dict(attrs)}))
        return calls
    start = _timestamp(row.get("started_at"))
    if source.kind == "sanity" and start is not None:
        span: dict[str, Any] = {
            "start_time_unix_nano": _unix_nanos(start),
            "end_time_unix_nano": _unix_nanos(time),
            "attributes": dict(attrs),
        }
        trace = _trace_id(row.get("run_id"))
        if trace is not None:
            span["trace_id"] = trace
        error = row.get("error")
        if isinstance(error, Mapping):
            span["ok"] = False
            span["error"] = str(error.get("message", ""))
        calls.append(Call("span", ("sanity.review",), span))
    return calls


class Importer:
    def __init__(
        self,
        config_path: Path,
        telemetry: Telemetry,
        checkpoint_path: Path | None = None,
        config: dict[str, object] | None = None,
    ) -> None:
        self.config_path, self.telemetry = config_path, telemetry
        self.config_failure: tuple[str, str] | None = None
        try:
            self.config = config if config is not None else load_config(config_path)
        except ValueError:
            self.config = {}
            self.config_failure = ("config", "CONFIG_LOAD")
        self.checkpoint_path = checkpoint_path or config_path.parent / "telemetry-state/checkpoints.json"

    def _source_path(self, source_path: str) -> Path:
        """Resolve configured sources without duplicating the conventional .sc directory."""
        path = Path(source_path)
        if path.is_absolute():
            return path
        config_directory = self.config_path.parent
        if path.parts[:1] == (config_directory.name,):
            config_directory = config_directory.parent
        return (config_directory / path).resolve()

    def _state(self) -> tuple[dict[str, dict[str, object]], str | None]:
        try:
            value = json.loads(self.checkpoint_path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return {}, None
        except UnicodeError:
            return {}, "CHECKPOINT_INVALID"
        except json.JSONDecodeError:
            return {}, "CHECKPOINT_INVALID"
        except OSError:
            return {}, "CHECKPOINT_READ"
        if not isinstance(value, dict):
            return {}, "CHECKPOINT_INVALID"
        for checkpoint in value.values():
            if (
                not isinstance(checkpoint, Mapping)
                or not isinstance(checkpoint.get("offset"), int)
                or isinstance(checkpoint.get("offset"), bool)
                or checkpoint["offset"] < 0
                or not isinstance(checkpoint.get("fingerprint"), str)
                or not isinstance(checkpoint.get("inode"), int)
            ):
                return {}, "CHECKPOINT_INVALID"
        return value, None

    def _write_state(self, state: Mapping[str, object]) -> None:
        self.checkpoint_path.parent.mkdir(parents=True, exist_ok=True)
        temporary: str | None = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                dir=self.checkpoint_path.parent,
                prefix=f".{self.checkpoint_path.name}.",
                suffix=".tmp",
                delete=False,
            ) as handle:
                temporary = handle.name
                handle.write(json.dumps(state, sort_keys=True, separators=(",", ":")))
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary, self.checkpoint_path)
        finally:
            if temporary is not None:
                try:
                    os.unlink(temporary)
                except FileNotFoundError:
                    pass

    def _save_checkpoint(
        self,
        state: dict[str, dict[str, object]],
        key: str,
        offset: int,
        fingerprint: str,
        inode: int,
    ) -> str | None:
        state[key] = {"offset": offset, "fingerprint": fingerprint, "inode": inode}
        try:
            self._write_state(state)
        except OSError:
            return "CHECKPOINT_WRITE"
        return None

    def _export(self, calls: list[Call]) -> tuple[str, str] | None:
        """Make a row's calls in order; return the first failure as ``(kind, code)``."""
        for call in calls:
            try:
                result = getattr(self.telemetry, call.method)(*call.args, **call.kwargs)
            except Exception:
                return ("telemetry", "EXPORT_EXCEPTION")
            if isinstance(result, TelemetryErr):
                return (result.error.kind, result.error.code)
            if not isinstance(result, Ok):
                return ("telemetry", "UNEXPECTED_RESULT")
        return None

    def import_source(self, source: Source) -> Report:
        report = Report()
        state, state_error = self._state()
        if state_error is not None:
            report.failures.append(("checkpoint", state_error))
            return report
        path = self._source_path(source.path)
        try:
            handle = path.open("rb")
        except OSError:
            report.failures.append(("source", "SOURCE_READ"))
            return report
        with handle:
            try:
                source_stat = os.fstat(handle.fileno())
                first_line = handle.readline()
            except OSError:
                report.failures.append(("source", "SOURCE_READ"))
                return report
            fingerprint = hashlib.sha256(first_line.split(b"\n", 1)[0]).hexdigest()
            key = _checkpoint_key(source)
            previous = state.get(key)
            if previous is not None and not isinstance(previous, Mapping):
                report.failures.append(("checkpoint", "CHECKPOINT_INVALID"))
                return report
            offset = 0
            if (
                previous is not None
                and previous.get("fingerprint") == fingerprint
                and previous.get("inode") == source_stat.st_ino
                and previous.get("offset", 0) <= source_stat.st_size
            ):
                offset = previous["offset"]
            try:
                handle.seek(offset)
                while True:
                    raw = handle.readline()
                    if not raw or not raw.endswith(b"\n"):
                        break
                    next_offset = handle.tell()
                    try:
                        row = json.loads(raw)
                    except (json.JSONDecodeError, UnicodeError):
                        row = None
                    calls = map_row(source, row, self.config) if isinstance(row, Mapping) else None
                    failure = None if calls is None else self._export(calls)
                    if calls is None or (failure is not None and failure[0] == "validation"):
                        report.skipped_invalid += 1
                    elif failure is not None:
                        report.failures.append(failure)
                        break
                    else:
                        report.exported += 1
                    checkpoint_error = self._save_checkpoint(
                        state, key, next_offset, fingerprint, source_stat.st_ino
                    )
                    if checkpoint_error:
                        report.failures.append(("checkpoint", checkpoint_error))
                        break
            except OSError:
                report.failures.append(("source", "SOURCE_READ"))
        return report

    def run_once(self) -> Report:
        result = Report()
        if self.config_failure is not None:
            result.failures.append(self.config_failure)
            return result
        try:
            configured_sources = sources(self.config)
        except ValueError:
            result.failures.append(("config", "CONFIG_INVALID"))
            return result
        for source in configured_sources:
            one = self.import_source(source)
            result.exported += one.exported
            result.skipped_invalid += one.skipped_invalid
            result.failures.extend(one.failures)
        return result


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("import", "follow"))
    parser.add_argument("--config", default=".sc/telemetry.yaml")
    parser.add_argument("--poll-interval", type=float, default=1.0)
    args = parser.parse_args(argv)
    try:
        config = load_config(Path(args.config))
    except ValueError:
        print(_summary(Report(failures=[("config", "CONFIG_LOAD")])))
        return 2
    service = config.get("service")
    telemetry = Telemetry(endpoint=_endpoint(config), service_name=service if isinstance(service, str) else None)
    importer = Importer(Path(args.config), telemetry, config=config)
    failed = False
    try:
        while True:
            report = importer.run_once()
            print(_summary(report))
            failed = failed or bool(report.failures)
            if args.mode == "import":
                return 1 if report.failures else 0
            sleep(max(0.01, args.poll_interval))
    except KeyboardInterrupt:
        return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
