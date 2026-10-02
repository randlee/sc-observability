"""Import the repository's sanity and QA JSONL histories through Telemetry.

The importer deliberately owns no transport policy.  It translates source rows into
the neutral submission document accepted by the installed Python facade, and only
advances a source cursor after its admission receipt is returned.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import dataclass, field
from pathlib import Path
from time import sleep
from typing import Any, Iterable, Mapping

import yaml

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr


def _timestamp(value: object) -> str | None:
    """Return the canonical nanosecond UTC representation without inventing time."""
    if not isinstance(value, str) or not value.endswith("Z"):
        return None
    stem = value[:-1]
    if "T" not in stem:
        return None
    if "." not in stem:
        return f"{stem}.000000000Z"
    head, fraction = stem.rsplit(".", 1)
    if not fraction.isdigit() or len(fraction) > 9:
        return None
    return f"{head}.{fraction.ljust(9, '0')}Z"


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
    reviewer = row.get("reviewer") or source.reviewer or "sanity-llm"
    return f"{source.kind}:{source.path}:{identity}:{reviewer}"


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
    value = yaml.safe_load(path.read_text())
    if not isinstance(value, dict):
        return {}
    return value


def sources(config: Mapping[str, object]) -> list[Source]:
    raw = config.get("sources", [])
    if not isinstance(raw, list):
        return []
    return [
        Source(str(item["path"]), str(item["kind"]), item.get("phase") if isinstance(item.get("phase"), str) else None,
               item.get("reviewer") if isinstance(item.get("reviewer"), str) else None)
        for item in raw if isinstance(item, Mapping) and isinstance(item.get("path"), str) and isinstance(item.get("kind"), str)
    ]


def _common(source: Source, row: Mapping[str, object], config: Mapping[str, object]) -> dict[str, object]:
    reviewer = row.get("reviewer") or source.reviewer or "sanity-llm"
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
        url = template.get("pr_url_template", "").replace("{pr_number}", str(pr)) if isinstance(template, Mapping) else ""
        attrs.update({"vcs.pr.number": pr, "pr.url": url})
    return attrs


def map_row(source: Source, row: Mapping[str, object], config: Mapping[str, object]) -> dict[str, object] | None:
    """Map one source row. Invalid mandatory source fields produce no submission."""
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
    def __init__(self, config_path: Path, telemetry: Telemetry, checkpoint_path: Path | None = None) -> None:
        self.config_path, self.config, self.telemetry = config_path, load_config(config_path), telemetry
        self.checkpoint_path = checkpoint_path or config_path.parent / ".sc/telemetry-state/checkpoints.json"

    def _state(self) -> dict[str, dict[str, object]]:
        try:
            value = json.loads(self.checkpoint_path.read_text())
            return value if isinstance(value, dict) else {}
        except (OSError, json.JSONDecodeError):
            return {}

    def _write_state(self, state: Mapping[str, object]) -> None:
        self.checkpoint_path.parent.mkdir(parents=True, exist_ok=True)
        self.checkpoint_path.write_text(json.dumps(state, sort_keys=True, separators=(",", ":")))

    def import_source(self, source: Source) -> Report:
        report, state = Report(), self._state()
        path = (self.config_path.parent / source.path).resolve()
        try:
            data = path.read_bytes()
        except OSError as error:
            report.failures.append(("source", str(error)))
            return report
        previous = state.get(source.path, {})
        first = hashlib.sha256(data.split(b"\n", 1)[0]).hexdigest()
        inode = path.stat().st_ino
        offset = previous.get("offset", 0) if isinstance(previous, Mapping) else 0
        offset = offset if (
            isinstance(offset, int)
            and offset <= len(data)
            and previous.get("fingerprint") == first
            and previous.get("inode") == inode
        ) else 0
        for raw in data[offset:].splitlines(keepends=True):
            if not raw.endswith(b"\n"):
                break
            next_offset = offset + len(raw)
            try:
                row = json.loads(raw)
            except json.JSONDecodeError:
                report.skipped_invalid += 1
                offset = next_offset
                state[source.path] = {"offset": offset, "fingerprint": first, "inode": inode}
                self._write_state(state)
                continue
            submission = map_row(source, row, self.config) if isinstance(row, Mapping) else None
            if submission is None:
                report.skipped_invalid += 1
                offset = next_offset
                state[source.path] = {"offset": offset, "fingerprint": first, "inode": inode}
                self._write_state(state)
                continue
            result = self.telemetry.emit(submission)
            if isinstance(result, TelemetryErr):
                report.failures.append((result.error.kind, result.error.code))
                break
            assert isinstance(result, Ok)
            report.admitted += 1
            report.duplicates += int(result.value.duplicate)
            offset = next_offset
            state[source.path] = {"offset": offset, "fingerprint": first, "inode": inode}
            self._write_state(state)
        return report

    def run_once(self) -> Report:
        result = Report()
        for source in sources(self.config):
            one = self.import_source(source)
            result.admitted += one.admitted; result.duplicates += one.duplicates; result.skipped_invalid += one.skipped_invalid; result.failures.extend(one.failures)
        return result


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("import", "follow"))
    parser.add_argument("--config", default=".sc/telemetry.yaml")
    parser.add_argument("--poll-interval", type=float, default=1.0)
    args = parser.parse_args(argv)
    opened = Telemetry.open(config=args.config)
    if isinstance(opened, TelemetryErr):
        print(f"{opened.error.kind}:{opened.error.code}: {opened.error.message}")
        return 4
    importer = Importer(Path(args.config), opened.value)
    while True:
        report = importer.run_once()
        print(json.dumps({"admitted": report.admitted, "duplicates": report.duplicates, "skipped_invalid": report.skipped_invalid, "failures": report.failures}))
        if args.mode == "import":
            flushed = opened.value.flush()
            if isinstance(flushed, TelemetryErr):
                print(f"{flushed.error.kind}:{flushed.error.code}")
            return 0
        sleep(max(0.01, args.poll_interval))


if __name__ == "__main__":
    raise SystemExit(main())
