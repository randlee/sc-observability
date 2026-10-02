"""Import the repository's sanity and QA JSONL histories through Telemetry.

The importer deliberately owns no transport policy.  It translates source rows into
the neutral submission document accepted by the installed Python facade, and only
advances a source cursor after its admission receipt is returned.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from time import sleep
from typing import Any, Iterable, Mapping

import yaml

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr


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
        from datetime import datetime

        datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        return None
    return f"{head}.{(fraction or '').ljust(9, '0')}Z"


def _attrs(**values: object) -> dict[str, object]:
    return {name: value for name, value in values.items() if value is not None}


def _resource(config: Mapping[str, object]) -> dict[str, object]:
    return {
        "attributes": _attrs(**{"service.name": config.get("service"), "team": config.get("team")}),
        "dropped_attributes_count": 0,
        "entity_refs": [],
        "schema_url": None,
    }


def _scope() -> dict[str, object]:
    return {
        "name": "sc-observability.sanity",
        "version": None,
        "attributes": {},
        "dropped_attributes_count": 0,
        "schema_url": None,
    }


def _stable_row(row: Mapping[str, object]) -> str:
    return hashlib.sha256(json.dumps(row, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def _record_key(source: "Source", row: Mapping[str, object]) -> str:
    identity = row.get("run_id") if isinstance(row.get("run_id"), str) else _stable_row(row)
    reviewer = _reviewer(source, row) or ""
    return f"{source.kind}:{source.path}:{identity}:{reviewer}"


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
    if not isinstance(run_id, str):
        return None
    return hashlib.sha256(run_id.encode()).hexdigest()[:32]


@dataclass(frozen=True)
class Source:
    path: str
    kind: str
    phase: str | None = None
    reviewer: str | None = None


@dataclass
class Report:
    admitted: int = 0
    duplicates: int = 0
    skipped_invalid: int = 0
    failures: list[tuple[str, str]] = field(default_factory=list)


def load_config(path: Path) -> dict[str, object]:
    try:
        value = yaml.safe_load(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, yaml.YAMLError) as error:
        raise ValueError("could not read importer configuration") from error
    if not isinstance(value, dict):
        raise ValueError("importer configuration must be a mapping")
    sources(value)
    return value


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
            or item["kind"] not in {"sanity", "qa", "finding-counts"}
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
    attrs = _attrs(
        **{
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
            "review.error.code": row.get("error", {}).get("code") if isinstance(row.get("error"), Mapping) else None,
            "review.error.message": row.get("error", {}).get("message") if isinstance(row.get("error"), Mapping) else None,
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


def map_row(source: Source, row: Mapping[str, object], config: Mapping[str, object]) -> dict[str, object] | None:
    """Map one source row. Invalid mandatory source fields produce no submission."""
    if source.kind not in {"sanity", "qa", "finding-counts"}:
        return None
    time = _timestamp(row.get("completed_at") or row.get("snapshot_at"))
    verdict = row.get("verdict")
    if time is None or (source.kind != "finding-counts" and not isinstance(verdict, str)):
        return None
    attrs = _common(source, row, config)
    input: dict[str, object] = {"version": 1, "record_key": _record_key(source, row), "resource": _resource(config), "scope": _scope(), "logs": [], "spans": [], "metrics": []}
    if source.kind == "finding-counts":
        input["logs"] = [{"time": time, "body": "finding-counts", "attributes": attrs}]
        metrics = []
        for field, name in (("open", "sc.qa.findings.open"), ("tot", "sc.qa.findings.total")):
            if isinstance(row.get(field), int):
                metrics.append({"name": name, "description": None, "unit": None, "metadata": [], "data": {"kind": "gauge", "data": {"points": [{"attributes": attrs, "start_time": None, "time": time, "value": {"kind": "int", "data": row[field]}, "exemplars": [], "flags": 0}]}}})
        input["metrics"] = metrics
        return input
    input["logs"] = [{"time": time, "body": str(verdict), "attributes": attrs}]
    start = _timestamp(row.get("started_at"))
    if source.kind == "sanity" and start is not None:
        span: dict[str, object] = {"name": "sanity.review", "start_time": start, "end_time": time, "attributes": attrs}
        trace = _trace_id(row.get("run_id"))
        if trace is not None:
            span["trace_id"] = trace
        if isinstance(row.get("error"), Mapping):
            span["status"] = {"code": "error", "message": str(row["error"].get("message", ""))}
        input["spans"] = [span]
    return input


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
                    submission = map_row(source, row, self.config) if isinstance(row, Mapping) else None
                    if submission is None:
                        report.skipped_invalid += 1
                        checkpoint_error = self._save_checkpoint(
                            state, key, next_offset, fingerprint, source_stat.st_ino
                        )
                        if checkpoint_error:
                            report.failures.append(("checkpoint", checkpoint_error))
                            break
                        continue
                    try:
                        result = self.telemetry.emit(submission)
                    except Exception:
                        report.failures.append(("telemetry", "EMIT_EXCEPTION"))
                        break
                    if isinstance(result, TelemetryErr):
                        if result.error.kind == "submission":
                            report.skipped_invalid += 1
                            checkpoint_error = self._save_checkpoint(
                                state, key, next_offset, fingerprint, source_stat.st_ino
                            )
                            if checkpoint_error:
                                report.failures.append(("checkpoint", checkpoint_error))
                                break
                            continue
                        report.failures.append((result.error.kind, result.error.code))
                        remediation = result.error.remediation
                        retryable = isinstance(remediation, Mapping) and remediation.get("kind") == "recoverable"
                        if retryable:
                            break
                        checkpoint_error = self._save_checkpoint(
                            state, key, next_offset, fingerprint, source_stat.st_ino
                        )
                        if checkpoint_error:
                            report.failures.append(("checkpoint", checkpoint_error))
                            break
                        continue
                    if not isinstance(result, Ok):
                        report.failures.append(("telemetry", "UNEXPECTED_RESULT"))
                        break
                    if result.value.duplicate:
                        report.duplicates += 1
                    else:
                        report.admitted += 1
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
            result.admitted += one.admitted
            result.duplicates += one.duplicates
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
        report = Report(failures=[("config", "CONFIG_LOAD")])
        print(json.dumps({"admitted": report.admitted, "duplicates": report.duplicates,
                          "skipped_invalid": report.skipped_invalid, "failures": report.failures}))
        return 2
    opened = Telemetry.open(config=args.config)
    if isinstance(opened, TelemetryErr):
        print(json.dumps({"admitted": 0, "duplicates": 0, "skipped_invalid": 0,
                          "failures": [[opened.error.kind, opened.error.code]]}))
        return 2 if opened.error.kind == "config" else 1
    importer = Importer(Path(args.config), opened.value, config=config)
    failed = False
    try:
        while True:
            report = importer.run_once()
            if args.mode == "import":
                flushed = opened.value.flush()
                if isinstance(flushed, TelemetryErr):
                    report.failures.append((flushed.error.kind, flushed.error.code))
            print(json.dumps({"admitted": report.admitted, "duplicates": report.duplicates,
                              "skipped_invalid": report.skipped_invalid, "failures": report.failures}))
            failed = failed or bool(report.failures)
            if args.mode == "import":
                return 1 if report.failures else 0
            sleep(max(0.01, args.poll_interval))
    except KeyboardInterrupt:
        return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
