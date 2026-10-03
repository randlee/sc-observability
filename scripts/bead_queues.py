#!/usr/bin/env python3
"""Read-only summary of actionable bead work queues joined with live ATM tasks.

Scope, then type: beads are scoped to the parent-child descendants of a root
bead (default obs-phase-<phase>), then bucketed by issue_type and stage label.
A row lands in an action queue only when its prerequisite is met (sanity:
checked bead closed; QA: paired sanity closed PASS); otherwise it is counted
as not ready with the blocking bead named. Pairing uses bead metadata and
dependency links, never bead-id name patterns.

Usage: scripts/bead_queues.py [--root obs-phase-d | --phase d] [--days 14]
                              [--all] [--json] [--as team-lead] [--team sc-obs]
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib
from datetime import datetime, timezone

SANITY_STAGES = {"stage:dev-sanity", "stage:sanity"}
FINDING_STAGES = {"stage:finding", "stage:fix"}
DEV_STAGES = {"stage:dev", "stage:implementation", "stage:integration"}
PASS_SHA_RE = re.compile(r"^PASS\b.*?\b([0-9a-f]{7,40})\b")

QUEUES = [
    ("in_flight", "In flight (ATM task open)", "assignee"),
    ("claimed_no_task", "Claimed (in_progress), no ATM task", "lead (verify)"),
    ("needs_sanity", "Dev complete -> needs sanity dispatch", "lead -> sanity"),
    ("needs_qa", "Sanity PASS -> needs QA dispatch", "lead -> quality-mgr"),
    ("no_qa_record", "Sanity PASS -> no QA record", "lead (verify)"),
    ("needs_sanity_fix", "Sanity FAIL -> needs dev fix dispatch", "lead -> dev"),
    ("needs_qa_fix", "QA finding -> needs fix dispatch", "lead -> dev"),
    ("minor_backlog", "QA finding, minor (backlog)", "lead (schedule)"),
    ("needs_dev", "Dev ready -> needs dispatch", "lead -> dev"),
    ("ready_other", "Other ready, no task", "lead"),
    ("not_ready", "Not ready (prerequisite open)", "-"),
]
HIDDEN = {"not_ready"}


def run_json(cmd: list[str]):
    out = subprocess.run(cmd, capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"{' '.join(cmd)} failed: {out.stderr.strip()}")
    return json.loads(out.stdout or "[]")


def git_ok(*args: str) -> bool:
    return subprocess.run(["git", *args], capture_output=True).returncode == 0


def git_out(*args: str) -> str | None:
    out = subprocess.run(["git", *args], capture_output=True, text=True)
    return out.stdout.strip() if out.returncode == 0 else None


def default_team() -> str | None:
    """$ATM_TEAM, else [atm] default_team from the repo's .atm.toml."""
    if os.environ.get("ATM_TEAM"):
        return os.environ["ATM_TEAM"]
    root = git_out("rev-parse", "--show-toplevel")
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


def ts(value: str | None) -> datetime | None:
    return datetime.fromisoformat(value.replace("Z", "+00:00")) if value else None


def age(value: str | None, now: datetime) -> str:
    t = ts(value)
    if t is None:
        return "-"
    mins = int((now - t).total_seconds() // 60)
    if mins < 60:
        return f"{mins}m"
    if mins < 48 * 60:
        return f"{mins // 60}h{mins % 60:02d}"
    return f"{mins // 1440}d"


def checked_of(bead: dict) -> str | None:
    m = meta(bead)
    return m.get("checked_bead") or m.get("dev_bead")


def is_sanity_finding(bead: dict) -> bool:
    m = meta(bead)
    return bool(m.get("sanity_finding")) or str(m.get("reviewer", "")).startswith("sc-sanity")


def is_pass(bead: dict) -> bool:
    return bead["status"] == "closed" and str(bead.get("close_reason") or "").startswith("PASS")


def pass_sha(bead: dict) -> str | None:
    m = PASS_SHA_RE.match(str(bead.get("close_reason") or ""))
    return m.group(1) if m else meta(bead).get("commit")


class Ancestry:
    """Tag a PASS head against its branch tip and the landing refs (origin/*, no fetch)."""

    def __init__(self, landed_refs: list[str]):
        self.landed = [r for r in landed_refs if git_ok("rev-parse", "--verify", "-q", r)]
        self.cache: dict[tuple, str] = {}

    def tag(self, sha: str | None, branch: str | None) -> str:
        if not sha:
            return "no-pass-sha"
        key = (sha, branch)
        if key in self.cache:
            return self.cache[key]
        full = git_out("rev-parse", "--verify", "-q", f"{sha}^{{commit}}")
        if not full:
            result = "sha-unknown"
        elif any(git_ok("merge-base", "--is-ancestor", full, r) for r in self.landed):
            result = "landed"
        else:
            tip = git_out("rev-parse", "--verify", "-q", f"origin/{branch}") if branch else None
            if tip is None:
                result = "no-branch"
            elif tip == full:
                result = "open-tip"
            elif git_ok("merge-base", "--is-ancestor", full, tip):
                result = "behind-tip"
            else:
                result = "diverged"
        self.cache[key] = result
        return result


def descendants(root: str, beads: list[dict]) -> set[str]:
    children: dict[str, list[str]] = {}
    for b in beads:
        if b.get("parent"):
            children.setdefault(b["parent"], []).append(b["id"])
    seen: set[str] = set()
    stack = [root]
    while stack:
        for c in children.get(stack.pop(), []):
            if c not in seen:
                seen.add(c)
                stack.append(c)
    return seen


def build(args) -> dict:
    now = datetime.now(timezone.utc)
    beads = run_json(["bd", "list", "--all", "-n", "0", "--json"])
    ready = {b["id"] for b in run_json(["bd", "ready", "-n", "0", "--json"])}
    atm = ["atm", "task", "list", "--all", "--json", "--as", args.as_actor]
    if args.team:
        atm += ["--team", args.team]
    tasks = {t["task_id"]: t for t in run_json(atm)}
    by_id = {b["id"]: b for b in beads}

    root = args.root or (f"obs-phase-{args.phase}" if args.phase else None)
    if root and root in by_id:
        scope = descendants(root, beads)
        scope_desc = f"descendants of {root}"
    elif args.phase:
        label = f"phase-{args.phase}"
        scope = {b["id"] for b in beads if label in (b.get("labels") or [])}
        scope_desc = f"label {label} (root {root} not found)"
    else:
        scope = set(by_id)
        scope_desc = "all beads"
    if root and root in by_id and not scope and args.phase:
        label = f"phase-{args.phase}"
        scope = {b["id"] for b in beads if label in (b.get("labels") or [])}
        scope_desc = f"label {label} (empty closure under {root})"

    phase = (root or "").removeprefix("obs-phase-") if root else args.phase
    landed_refs = args.landed_ref or [r for r in (f"origin/integrate/phase-{phase}" if phase else None,
                                                  "origin/develop") if r]
    ancestry = Ancestry(landed_refs)

    # Metadata and dependency indexes for pairing.
    open_children: dict[str, list[str]] = {}
    for b in beads:
        if b.get("parent") and b["status"] != "closed":
            open_children.setdefault(b["parent"], []).append(b["id"])
    sanity_for: dict[str, list[dict]] = {}
    for b in beads:
        if stage(b) in SANITY_STAGES and checked_of(b):
            sanity_for.setdefault(checked_of(b), []).append(b)
    qa_beads = [b for b in beads if stage(b) == "stage:qa"]

    def ancestors(bid: str | None) -> set[str]:
        out: set[str] = set()
        while bid and bid in by_id and bid not in out:
            out.add(bid)
            bid = by_id[bid].get("parent")
        return out

    def same_commit(a: str | None, b: str | None) -> bool:
        return bool(a and b and len(min(a, b, key=len)) >= 7 and (a.startswith(b) or b.startswith(a)))

    def qa_records_for(sanity: dict) -> list[dict]:
        chk = checked_of(sanity)
        lineage = ancestors(chk)
        sha = pass_sha(sanity)
        out = []
        for q in qa_beads:
            m = meta(q)
            linked = {d["depends_on_id"] for d in q.get("dependencies") or []}
            if (m.get("sanity_bead") == sanity["id"] or (chk and m.get("checked_bead") == chk)
                    or meta(sanity).get("qa_bead") == q["id"]
                    or sanity["id"] in linked or (chk and chk in linked)
                    or (chk and q.get("parent") == chk)
                    # QA of an ancestor finding pinned to this PASS head covers it.
                    or (m.get("checked_bead") in lineage and same_commit(m.get("commit"), sha))):
                out.append(q)
        return out

    def paired_sanity(qa: dict) -> dict | None:
        m = meta(qa)
        if m.get("sanity_bead") in by_id:
            return by_id[m["sanity_bead"]]
        cands = sanity_for.get(m.get("checked_bead") or "", [])
        linked = [by_id[d["depends_on_id"]] for d in qa.get("dependencies") or []
                  if d["depends_on_id"] in by_id and stage(by_id[d["depends_on_id"]]) in SANITY_STAGES]
        cands = linked or cands
        if not cands:
            return None
        return max(cands, key=lambda s: (s["status"] == "closed", s.get("closed_at") or "", s.get("created_at") or ""))

    def open_blockers(b: dict) -> list[str]:
        return [d["depends_on_id"] for d in b.get("dependencies") or []
                if d.get("type") == "blocks" and d.get("issue_id") == b["id"]
                and by_id.get(d["depends_on_id"], {}).get("status") != "closed"]

    queues: dict[str, list[dict]] = {k: [] for k, _, _ in QUEUES}

    def row(b: dict, **extra) -> dict:
        r = {
            "id": b["id"],
            "stage": stage(b).removeprefix("stage:"),
            "type": b.get("issue_type"),
            "status": b["status"],
            "assignee": b.get("assignee") or "-",
            "priority": b.get("priority"),
            "updated": age(b.get("updated_at"), now),
            "title": b.get("title", ""),
        }
        r.update(extra)
        return r

    def not_ready(b: dict, reason: str) -> None:
        queues["not_ready"].append(row(b, reason=reason))

    for tid, t in tasks.items():
        b = by_id.get(tid)
        if b is None or tid not in scope:
            continue
        queues["in_flight"].append(row(
            b, task_state=t.get("state"), task_assignee=t.get("assignee"),
            assigned=age(t.get("assigned_at"), now), reminders=t.get("reminder_count", 0)))

    for b in beads:
        bid = b["id"]
        if bid not in scope or b["status"] == "closed" or bid in tasks:
            continue
        if b.get("issue_type") == "epic":
            continue
        s = stage(b)
        if b["status"] == "in_progress":
            queues["claimed_no_task"].append(row(b, claimed=age(b.get("started_at") or b.get("updated_at"), now)))
            continue
        blockers = open_blockers(b) if bid not in ready else []
        if blockers:
            not_ready(b, "blocked by " + ",".join(blockers))
            continue
        if b["status"] != "open":
            not_ready(b, f"status {b['status']}")
            continue
        if open_children.get(bid):
            not_ready(b, "open children " + ",".join(open_children[bid][:3])
                      + ("…" if len(open_children[bid]) > 3 else ""))
            continue
        if s in SANITY_STAGES:
            chk = by_id.get(checked_of(b) or "")
            if chk is None:
                not_ready(b, "no paired dev bead (metadata dev_bead/checked_bead)")
            elif chk["status"] != "closed":
                not_ready(b, f"blocked by {chk['id']} ({chk['status']})")
            else:
                queues["needs_sanity"].append(row(
                    b, checked=chk["id"], checked_closed=age(chk.get("closed_at"), now)))
        elif s == "stage:qa":
            san = paired_sanity(b)
            if san is None:
                not_ready(b, "no paired sanity bead")
            elif not is_pass(san):
                not_ready(b, f"blocked by {san['id']} ({san['status']}, not PASS)")
            else:
                sha = pass_sha(san)
                queues["needs_qa"].append(row(
                    b, sanity=san["id"], pass_sha=(sha or "-")[:10],
                    ancestry=ancestry.tag(sha, meta(san).get("branch") or meta(b).get("branch"))))
        elif s in FINDING_STAGES:
            if is_sanity_finding(b):
                key = "needs_sanity_fix"
            elif str(meta(b).get("severity", "")).lower() == "minor":
                key = "minor_backlog"
            else:
                key = "needs_qa_fix"
            queues[key].append(row(
                b, reviewer=meta(b).get("reviewer", "-"),
                severity=meta(b).get("severity", "-"), parent=b.get("parent") or "-"))
        elif s in DEV_STAGES:
            queues["needs_dev"].append(row(b, difficulty=meta(b).get("difficulty", "-")))
        else:
            queues["ready_other"].append(row(b))

    # Latest closed PASS sanity per checked bead that has no paired QA record.
    latest: dict[str, dict] = {}
    for b in beads:
        if b["id"] in scope and stage(b) in SANITY_STAGES and b["status"] == "closed" and checked_of(b):
            cur = latest.get(checked_of(b))
            if cur is None or (b.get("closed_at") or "") > (cur.get("closed_at") or ""):
                latest[checked_of(b)] = b
    for chk, san in latest.items():
        if not is_pass(san):
            continue
        closed = ts(san.get("closed_at"))
        if closed and (now - closed).days > args.days:
            continue
        if qa_records_for(san):
            continue
        sha = pass_sha(san)
        queues["no_qa_record"].append(row(
            san, checked=chk, closed=age(san.get("closed_at"), now), pass_sha=(sha or "-")[:10],
            ancestry=ancestry.tag(sha, meta(san).get("branch") or meta(by_id.get(chk, {})).get("branch"))))

    for q in queues.values():
        q.sort(key=lambda r: (r.get("priority") if r.get("priority") is not None else 9, r["id"]))
    return {
        "generated_at": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "scope": scope_desc,
        "landed_refs": ancestry.landed,
        "queues": queues,
    }


def short(text: str, n: int) -> str:
    return text if len(text) <= n else text[: n - 1] + "…"


COLUMNS = {
    "in_flight": [("id", 46), ("task_assignee", 12), ("task_state", 8), ("assigned", 7), ("reminders", 4), ("status", 11)],
    "claimed_no_task": [("id", 46), ("assignee", 12), ("stage", 11), ("claimed", 7)],
    "needs_sanity": [("id", 46), ("assignee", 12), ("checked", 36), ("checked_closed", 7)],
    "needs_qa": [("id", 46), ("sanity", 36), ("pass_sha", 10), ("ancestry", 11)],
    "no_qa_record": [("id", 46), ("checked", 34), ("closed", 6), ("pass_sha", 10), ("ancestry", 11)],
    "needs_sanity_fix": [("id", 46), ("assignee", 12), ("parent", 36), ("updated", 7)],
    "needs_qa_fix": [("id", 46), ("assignee", 12), ("reviewer", 22), ("severity", 9), ("updated", 7)],
    "minor_backlog": [("id", 46), ("assignee", 12), ("reviewer", 22), ("updated", 7)],
    "needs_dev": [("id", 46), ("assignee", 12), ("difficulty", 8), ("updated", 7)],
    "ready_other": [("id", 46), ("assignee", 12), ("type", 8), ("stage", 12), ("updated", 7)],
    "not_ready": [("id", 46), ("assignee", 12), ("stage", 11), ("reason", 60)],
}


def render(report: dict, show_all: bool) -> None:
    q = report["queues"]
    print(f"Bead queues · {report['scope']} · {report['generated_at']}")
    print(f"landed refs: {', '.join(report['landed_refs']) or 'none'}\n")
    print(f"  {'queue':42} {'count':>5}  next actor")
    for key, label, actor in QUEUES:
        print(f"  {label:42} {len(q[key]):>5}  {actor}")
    for key, label, _ in QUEUES:
        rows = q[key]
        if not rows or (key in HIDDEN and not show_all):
            continue
        print(f"\n== {label} ({len(rows)})")
        cols = COLUMNS[key]
        print("  " + " ".join(f"{c:{w}}" for c, w in cols))
        for r in rows:
            print("  " + " ".join(f"{short(str(r.get(c, '-')), w):{w}}" for c, w in cols))
    hidden = [k for k in HIDDEN if q[k]]
    if hidden and not show_all:
        print(f"\n({sum(len(q[k]) for k in hidden)} not-ready beads hidden; --all shows them with reasons)")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--root", help="scope to parent-child descendants of this bead (default obs-phase-<phase>)")
    p.add_argument("--phase", help="phase name; root defaults to obs-phase-<phase>, label phase-<phase> is the fallback")
    p.add_argument("--days", type=int, default=14,
                   help="look-back for closed sanity PASS without a QA record (default 14)")
    p.add_argument("--landed-ref", action="append",
                   help="ref a PASS head counts as landed in (repeatable; default origin/integrate/phase-<phase>, origin/develop)")
    p.add_argument("--all", action="store_true", help="also list not-ready beads with reasons")
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
