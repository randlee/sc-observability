#!/usr/bin/env python3
"""Read-only summary of bead work queues joined with live ATM tasks.

Shows, per lifecycle hand-off, which beads are ready and who should act:
dev complete -> sanity, sanity PASS -> QA, sanity FAIL -> dev fix,
QA FAIL -> fix, plus in-flight tasks, refusals and beads waiting on blockers.

Usage: scripts/bead_queues.py [--phase d] [--days 14] [--json] [--all] [--as team-lead] [--team sc-obs]
"""
from __future__ import annotations

import argparse
import bisect
import json
import os
import re
import subprocess
import sys
import tomllib
from datetime import datetime, timezone

REFUSAL_RE = re.compile(r"^(?P<target>.+)-wf-(?P<code>[A-Z_]+)$")
SANITY_STAGES = {"stage:dev-sanity", "stage:sanity"}
FINDING_STAGES = {"stage:finding", "stage:fix"}

QUEUES = [
    ("in_flight", "In flight (ATM task open)", "assignee"),
    ("needs_sanity", "Dev complete -> needs sanity dispatch", "lead -> sanity"),
    ("needs_qa", "Sanity PASS -> needs QA dispatch", "lead -> quality-mgr"),
    ("needs_qa_bead", "Sanity PASS -> no QA record found", "lead (verify)"),
    ("needs_sanity_fix", "Sanity FAIL -> needs dev fix dispatch", "lead -> dev"),
    ("needs_qa_fix", "QA finding -> needs fix dispatch", "lead -> dev"),
    ("needs_dev", "Dev ready -> needs dispatch", "lead -> dev"),
    ("ready_other", "Other ready, no task", "lead"),
    ("refused", "Refused, target still open", "lead"),
    ("refused_stale", "Refusal bead open, target closed", "lead (close record)"),
    ("waiting", "Waiting on open blockers", "-"),
]


def run_json(cmd: list[str]):
    out = subprocess.run(cmd, capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"{' '.join(cmd)} failed: {out.stderr.strip()}")
    return json.loads(out.stdout or "[]")


def default_team() -> str | None:
    """$ATM_TEAM, else [atm] default_team from the repo's .atm.toml."""
    if os.environ.get("ATM_TEAM"):
        return os.environ["ATM_TEAM"]
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                          capture_output=True, text=True).stdout.strip()
    try:
        with open(os.path.join(root or ".", ".atm.toml"), "rb") as f:
            return tomllib.load(f).get("atm", {}).get("default_team")
    except (OSError, tomllib.TOMLDecodeError):
        return None


def stage(bead: dict) -> str:
    return next((l for l in bead.get("labels") or [] if l.startswith("stage:")), "-")


def meta(bead: dict) -> dict:
    m = bead.get("metadata")
    return m if isinstance(m, dict) else {}


def age(ts: str | None, now: datetime) -> str:
    if not ts:
        return "-"
    t = datetime.fromisoformat(ts.replace("Z", "+00:00"))
    mins = int((now - t).total_seconds() // 60)
    if mins < 60:
        return f"{mins}m"
    if mins < 48 * 60:
        return f"{mins // 60}h{mins % 60:02d}"
    return f"{mins // 1440}d"


def is_sanity_finding(bead: dict) -> bool:
    m = meta(bead)
    return bool(m.get("sanity_finding")) or str(m.get("reviewer", "")).startswith("sc-sanity")


def checked_of(bead: dict) -> str | None:
    m = meta(bead)
    return m.get("checked_bead") or m.get("dev_bead")


def build(args) -> dict:
    now = datetime.now(timezone.utc)
    beads = run_json(["bd", "list", "--all", "-n", "0", "--json"])
    ready = {b["id"] for b in run_json(["bd", "ready", "-n", "0", "--json"])}
    atm = ["atm", "task", "list", "--all", "--json", "--as", args.as_actor]
    if args.team:
        atm += ["--team", args.team]
    tasks = {t["task_id"]: t for t in run_json(atm)}
    by_id = {b["id"]: b for b in beads}

    phase_label = f"phase-{args.phase}" if args.phase else None

    def in_scope(b: dict) -> bool:
        if "workflow-issue" in (b.get("labels") or []) and not REFUSAL_RE.match(b["id"]):
            return False
        if phase_label and phase_label not in (b.get("labels") or []):
            target = REFUSAL_RE.match(b["id"])
            parent = by_id.get(target.group("target")) if target else None
            return bool(parent and phase_label in (parent.get("labels") or []))
        return True

    queues: dict[str, list[dict]] = {k: [] for k, _, _ in QUEUES}

    def row(b: dict, **extra) -> dict:
        r = {
            "id": b["id"],
            "stage": stage(b).removeprefix("stage:"),
            "status": b["status"],
            "assignee": b.get("assignee") or "-",
            "priority": b.get("priority"),
            "updated": age(b.get("updated_at"), now),
            "title": b.get("title", ""),
        }
        r.update(extra)
        return r

    # In flight: every open ATM task, whether or not its bead is in scope.
    for tid, t in tasks.items():
        b = by_id.get(tid)
        if b is not None and not in_scope(b):
            continue
        queues["in_flight"].append({
            "id": tid,
            "stage": stage(b).removeprefix("stage:") if b else "?",
            "status": b["status"] if b else "no-bead",
            "assignee": t.get("assignee"),
            "task_state": t.get("state"),
            "assigned": age(t.get("assigned_at"), now),
            "reminders": t.get("reminder_count", 0),
            "title": (b or {}).get("title", ""),
        })

    for b in beads:
        if b["status"] == "closed" or not in_scope(b) or b["id"] in tasks:
            continue
        s = stage(b)
        refusal = REFUSAL_RE.match(b["id"])
        if refusal:
            tgt = by_id.get(refusal.group("target"))
            key = "refused_stale" if tgt and tgt["status"] == "closed" else "refused"
            queues[key].append(row(
                b,
                code=refusal.group("code"),
                target=refusal.group("target"),
                target_status=tgt["status"] if tgt else "missing",
                target_task=tasks.get(refusal.group("target"), {}).get("state", "none"),
                created=age(b.get("created_at"), now),
            ))
            continue
        if b.get("issue_type") == "epic" or (phase_label and b["id"] == f"obs-{phase_label}"):
            continue
        if b["id"] not in ready:
            blockers = [
                d["depends_on_id"] for d in b.get("dependencies") or []
                if d.get("type") == "blocks" and d.get("issue_id") == b["id"]
                and by_id.get(d["depends_on_id"], {}).get("status") != "closed"
            ]
            open_children = [
                c["id"] for c in beads
                if c.get("parent") == b["id"] and c["status"] != "closed"
            ]
            if blockers or open_children or b["status"] != "open":
                queues["waiting"].append(row(
                    b, blocked_by=blockers, open_children=open_children))
                continue
        if s in SANITY_STAGES:
            chk = by_id.get(checked_of(b) or "")
            queues["needs_sanity"].append(row(
                b, checked=checked_of(b) or "-",
                checked_closed=age(chk.get("closed_at"), now) if chk else "-"))
        elif s == "stage:qa":
            queues["needs_qa"].append(row(b, checked=checked_of(b) or "-"))
        elif s in FINDING_STAGES:
            key = "needs_sanity_fix" if is_sanity_finding(b) else "needs_qa_fix"
            queues[key].append(row(
                b, reviewer=meta(b).get("reviewer", "-"),
                severity=meta(b).get("severity", "-"), parent=b.get("parent") or "-"))
        elif s == "stage:dev":
            queues["needs_dev"].append(row(b, difficulty=meta(b).get("difficulty", "-")))
        else:
            queues["ready_other"].append(row(b))

    # Sanity PASS whose checked bead shows no sign of QA: no stage:qa bead
    # checking it and no bead named <checked>-qa* (a QA bead or its findings).
    # Fix QA can run as a bare ATM task with no bead, so these need a look.
    qa_checked = {checked_of(b) for b in beads if stage(b) == "stage:qa"}
    qa_ids = {b["id"] for b in beads if stage(b) == "stage:qa"}
    all_ids = sorted(by_id)

    def has_qa_named(chk: str) -> bool:
        prefix = f"{chk}-qa"
        i = bisect.bisect_left(all_ids, prefix)
        return i < len(all_ids) and all_ids[i].startswith(prefix)
    for b in beads:
        if stage(b) not in SANITY_STAGES or b["status"] != "closed" or not in_scope(b):
            continue
        if not str(b.get("close_reason") or "").startswith("PASS"):
            continue
        closed = b.get("closed_at")
        if closed and (now - datetime.fromisoformat(closed.replace("Z", "+00:00"))).days > args.days:
            continue
        chk = checked_of(b)
        if meta(b).get("qa_bead") in qa_ids or chk in qa_checked or (chk and has_qa_named(chk)):
            continue
        queues["needs_qa_bead"].append(row(
            b, checked=chk or "-", closed=age(closed, now)))

    for q in queues.values():
        q.sort(key=lambda r: (r.get("priority") if r.get("priority") is not None else 9, r["id"]))
    return {"generated_at": now.strftime("%Y-%m-%dT%H:%M:%SZ"), "phase": args.phase, "queues": queues}


def short(text: str, n: int) -> str:
    return text if len(text) <= n else text[: n - 1] + "…"


COLUMNS = {
    "in_flight": [("id", 48), ("assignee", 12), ("task_state", 8), ("assigned", 7), ("reminders", 4), ("status", 11)],
    "needs_sanity": [("id", 48), ("assignee", 12), ("checked", 36), ("checked_closed", 7)],
    "needs_qa": [("id", 48), ("assignee", 12), ("checked", 36), ("updated", 7)],
    "needs_qa_bead": [("id", 48), ("checked", 36), ("closed", 7)],
    "needs_sanity_fix": [("id", 48), ("assignee", 12), ("parent", 36), ("updated", 7)],
    "needs_qa_fix": [("id", 48), ("assignee", 12), ("reviewer", 22), ("severity", 9), ("updated", 7)],
    "needs_dev": [("id", 48), ("assignee", 12), ("difficulty", 8), ("updated", 7)],
    "ready_other": [("id", 48), ("assignee", 12), ("stage", 12), ("updated", 7)],
    "refused_stale": [("code", 30), ("target", 44), ("created", 7)],
    "refused": [("code", 30), ("target", 44), ("target_status", 11), ("target_task", 8), ("created", 7)],
    "waiting": [("id", 48), ("assignee", 12), ("stage", 11), ("blocked_by", 40)],
}


def render(report: dict, show_all: bool) -> None:
    q = report["queues"]
    phase = f"phase-{report['phase']}" if report["phase"] else "all phases"
    print(f"Bead queues · {phase} · {report['generated_at']}\n")
    print(f"  {'queue':42} {'count':>5}  next actor")
    for key, label, actor in QUEUES:
        print(f"  {label:42} {len(q[key]):>5}  {actor}")
    for key, label, _ in QUEUES:
        rows = q[key]
        if not rows or (key == "waiting" and not show_all):
            continue
        print(f"\n== {label} ({len(rows)})")
        cols = COLUMNS[key]
        print("  " + " ".join(f"{c:{w}}" for c, w in cols))
        for r in rows:
            cells = []
            for c, w in cols:
                v = r.get(c, "-")
                if isinstance(v, list):
                    v = ",".join(v) or "-"
                cells.append(f"{short(str(v), w):{w}}")
            print("  " + " ".join(cells))
    if q["waiting"] and not show_all:
        print(f"\n({len(q['waiting'])} beads waiting on blockers hidden; --all shows them)")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--phase", help="limit to beads labelled phase-<PHASE>")
    p.add_argument("--days", type=int, default=14,
                   help="look-back for closed sanity PASS without a QA bead (default 14)")
    p.add_argument("--all", action="store_true", help="also list beads waiting on blockers")
    p.add_argument("--json", action="store_true", help="print the report as JSON")
    p.add_argument("--as", dest="as_actor", default=os.environ.get("ATM_IDENTITY") or "team-lead",
                   help="ATM identity for the read-only task list (default $ATM_IDENTITY or team-lead)")
    p.add_argument("--team", default=default_team(),
                   help="ATM team (default $ATM_TEAM, else [atm] default_team in the repo .atm.toml)")
    args = p.parse_args()
    report = build(args)
    if args.json:
        json.dump(report, sys.stdout, indent=2)
        print()
    else:
        render(report, args.all)


if __name__ == "__main__":
    main()
