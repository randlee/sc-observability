#!/usr/bin/env python3
"""Read-only, injectable pre-claim gates for dev, sanity, and QA work."""
from __future__ import annotations

import argparse
import json
import os
import re
from pathlib import Path
import subprocess
import sys
from typing import Any, Callable

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "atm-beads/scripts"))
from plan_contract import model_matches  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
from worktree_status import is_clean  # noqa: E402

Runner = Callable[..., subprocess.CompletedProcess[str]]
PRIMARY = Path(__file__).resolve().parents[4]
VALIDATE_PLAN = str(PRIMARY / ".claude/skills/atm-beads/scripts/validate-plan")


def run(runner: Runner, *args: str) -> str:
    result = runner(list(args), capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip() or "command failed")
    return result.stdout.strip()


def run_json(runner: Runner, *args: str) -> Any:
    return json.loads(run(runner, *args))


def metadata(bead: dict[str, Any]) -> dict[str, Any]:
    value = bead.get("metadata")
    return value if isinstance(value, dict) else {}


def refusal_for_difficulty(bead: dict[str, Any], members: list[dict[str, Any]], identity: str) -> str | None:
    difficulty = metadata(bead).get("difficulty")
    member = next((row for row in members if identity in (row.get("name"), row.get("identity"), row.get("id"))), None)
    if not isinstance(difficulty, str) or not model_matches(str((member or {}).get("model") or ""), difficulty):
        return "DIFFICULTY_MISMATCH"
    return None


def members_for(runner: Runner) -> list[dict[str, Any]]:
    members = run_json(runner, "atm", "members", "--json")
    return members if isinstance(members, list) else members.get("members", [])


def claimable(bead: dict[str, Any], identity: str) -> bool:
    return bead.get("status") == "open" and (not bead.get("assignee") or bead.get("assignee") == identity)


def has_prior_pass(history: Any) -> bool:
    """Dolt snapshots freeze only an actual closed PASS, even after reopening.

    The close protocol writes `PASS at <commit>` (legacy `PASS` also counts).
    Description/notes and speculative verdict metadata are never evidence.
    """
    if not isinstance(history, list):
        raise ValueError("bd history must return Dolt snapshots")
    for row in history:
        if not isinstance(row, dict) or not isinstance(row.get("Issue"), dict):
            raise ValueError("bd history snapshot has no Issue")
        issue = row["Issue"]
        reason = issue.get("close_reason")
        if (issue.get("status") == "closed" and isinstance(reason, str)
                and re.match(r"^PASS(?:$|\s+at\s+[0-9a-fA-F]{7,40}(?=$|[\s;:,]))", reason.strip())):
            return True
    return False


def git_dir(args: argparse.Namespace) -> list[str]:
    worktree = getattr(args, "worktree", "")
    return ["-C", worktree] if worktree else []


def dev_gate(args: argparse.Namespace, runner: Runner, identity: str) -> str:
    if runner([VALIDATE_PLAN, "--root", args.root, "--scope", args.bead], capture_output=True, text=True, cwd=str(PRIMARY)).returncode:
        return "PLAN_INVALID"
    ready = run_json(runner, "bd", "ready", "-n", "0", "--json")
    if not any(row.get("id") == args.bead for row in ready):
        return "NOT_READY"
    bead = run_json(runner, "bd", "show", args.bead, "--json")[0]
    if not claimable(bead, identity):
        return "UNCLAIMABLE"
    if metadata(bead).get("pr_target") not in (None, args.pr_target):
        return "PR_TARGET_MISMATCH"
    if (reason := refusal_for_difficulty(bead, members_for(runner), identity)):
        return reason
    if runner(["git", *git_dir(args), "merge-base", "--is-ancestor", f"origin/{args.pr_target}", "HEAD"], capture_output=True, text=True).returncode:
        return "WRONG_BASE"
    return "READY"


def direct_target(base: str) -> bool:
    return base == "develop" or (base.startswith("integrate/phase-") and len(base) > len("integrate/phase-"))


def sanity_stack_gate(args: argparse.Namespace, runner: Runner) -> str:
    """Consume the canonical view once; unknown evidence never means coherent."""
    result = runner([sys.executable, args.stack_view, "--json"], cwd=args.worktree,
                    capture_output=True, text=True)
    if args.stack_report:
        Path(args.stack_report).write_text(result.stdout + result.stderr)
    # The canonical tool uses exit 2 both for an empty view and errors. Only
    # its exact empty-view diagnostic permits the direct-PR exception.
    empty = (result.returncode == 2 and not result.stdout.strip()
             and result.stderr.startswith("gh-stack-view: no open gh stack found")
             and len(result.stderr.strip().splitlines()) == 1)
    if empty:
        return "READY" if direct_target(args.pr_target) else "STACK_REQUIRED"
    if result.returncode not in (0, 1):
        return "STACK_UNVERIFIED"
    report = json.loads(result.stdout)
    if not isinstance(report, dict) or not isinstance(report.get("stacks"), list):
        return "STACK_UNVERIFIED"
    if report.get("skipped_worktrees"):
        return "STACK_UNVERIFIED"
    if not all(isinstance(stack, dict) and isinstance(stack.get("rows"), list)
               and all(isinstance(row, dict) for row in stack["rows"])
               for stack in report["stacks"]):
        return "STACK_UNVERIFIED"
    matches = [(stack, row) for stack in report["stacks"] for row in stack["rows"]
               if str(row.get("pr")) == str(args.pr_number)]
    if not matches:
        return "READY" if direct_target(args.pr_target) else "STACK_REQUIRED"
    if len(matches) != 1:
        return "STACK_UNVERIFIED"
    stack, row = matches[0]
    if (row.get("branch") != args.branch or row.get("pr_base") != args.pr_target
            or row.get("pr_head") != args.commit or row.get("merged") is not False):
        return "STACK_INCOHERENT"
    if stack.get("coherent") is not True or stack.get("problems") != []:
        return "STACK_INCOHERENT"
    for layer in stack["rows"]:
        if layer.get("merged") is True:
            continue
        # The view allows the bottom layer to lag a moving trunk. Preserve
        # that rule: frozen layers are never rebased just to catch up.
        behind = layer.get("behind_trunk") is True
        if (layer.get("merged") is not False or not layer.get("head")
                or layer.get("head") != layer.get("origin")
                or layer.get("head") != layer.get("pr_head")
                or layer.get("origin_ok") is not True
                or not (layer.get("base_ok") is True or behind)
                or not (layer.get("needs_rebase") is False or behind)
                or layer.get("mergeable") != "MERGEABLE"
                or not layer.get("pr_base")):
            return "STACK_UNVERIFIED"
    return "READY"


def sanity_gate(args: argparse.Namespace, runner: Runner, identity: str) -> str:
    if not args.pr_number or not args.commit:
        return "PR_REQUIRED"
    if not args.worktree or not args.branch or not args.checked_bead or not args.stack_view:
        return "GATE_CANNOT_RUN"
    checked = run_json(runner, "bd", "show", args.checked_bead, "--json")[0]
    if metadata(checked).get("pr_target") != args.pr_target:
        return "PR_TARGET_MISMATCH"
    pr = json.loads(run_in_worktree(runner, args, "gh", "pr", "view", args.pr_number,
                                   "--json", "baseRefName,headRefOid,headRefName,state"))
    if not isinstance(pr, dict):
        return "GATE_CANNOT_RUN"
    if (pr.get("baseRefName") != args.pr_target or pr.get("headRefOid") != args.commit
            or pr.get("headRefName") != args.branch or pr.get("state") != "OPEN"):
        return "STALE_BASE"
    if (reason := sanity_stack_gate(args, runner)) != "READY":
        return reason
    run_in_worktree(runner, args, "git", "fetch", "origin")
    if (run_in_worktree(runner, args, "git", "rev-parse", "HEAD") != args.commit
            or run_in_worktree(runner, args, "git", "branch", "--show-current") != args.branch
            or run_in_worktree(runner, args, "git", "rev-parse", f"origin/{args.branch}") != args.commit):
        return "HEAD_MISMATCH"
    if not run_in_worktree(runner, args, "git", "log", "--format=%H", f"origin/{args.pr_target}..{args.commit}"):
        return "ZERO_DELTA"
    if not is_clean(run_in_worktree(runner, args, "git", "status", "--porcelain", "--untracked-files=all")):
        return "DIRTY_TREE"
    if has_prior_pass(run_json(runner, "bd", "history", args.bead, "--json")):
        return "SANITY_FROZEN"
    return "READY"


def run_in_worktree(runner: Runner, args: argparse.Namespace, *command: str) -> str:
    result = runner(list(command), cwd=args.worktree, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip() or "command failed")
    return result.stdout.strip()


def qa_gate(args: argparse.Namespace, runner: Runner, identity: str) -> str:
    if not args.pr_number:
        return "PR_REQUIRED"
    qa_bead = run_json(runner, "bd", "show", args.bead, "--json")[0]
    if metadata(qa_bead).get("pr_target") not in (None, args.pr_target):
        return "PR_TARGET_MISMATCH"
    checked = args.checked_bead or metadata(qa_bead).get("checked_bead")
    checked_bead = run_json(runner, "bd", "show", str(checked), "--json")[0] if checked else qa_bead
    pass_commit = metadata(checked_bead).get("sanity_pass_commit") or metadata(qa_bead).get("sanity_pass_commit")
    pr = run_json(runner, "gh", "pr", "view", args.pr_number, "--json", "baseRefName,headRefOid")
    if pr.get("baseRefName") != args.pr_target:
        return "PR_TARGET_MISMATCH"
    if not pass_commit or pr.get("headRefOid") != pass_commit:
        return "SANITY_STALE"
    if run(runner, "git", "rev-parse", "HEAD") != pr.get("headRefOid"):
        return "QA_HEAD_MISMATCH"
    return "READY"


def evaluate(args: argparse.Namespace, runner: Runner = subprocess.run) -> str:
    identity = args.identity or os.environ.get("ATM_IDENTITY", "")
    try:
        if args.kind == "dev":
            return dev_gate(args, runner, identity)
        if args.kind == "sanity":
            return sanity_gate(args, runner, identity)
        return qa_gate(args, runner, identity)
    except (RuntimeError, OSError, IndexError, TypeError, ValueError, KeyError):
        return "GATE_CANNOT_RUN"


def main(argv: list[str] | None = None, runner: Runner = subprocess.run) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=("dev", "sanity", "qa"))
    parser.add_argument("--root", default="")
    parser.add_argument("--bead", required=True)
    parser.add_argument("--pr-target", required=True)
    parser.add_argument("--identity", default="")
    parser.add_argument("--pr-number", default="")
    parser.add_argument("--commit", default="")
    parser.add_argument("--checked-bead", default="")
    parser.add_argument("--worktree", default="")
    parser.add_argument("--branch", default="")
    parser.add_argument("--stack-view", default="", help="installed canonical gh_stack_view.py")
    parser.add_argument("--stack-report", default="", help="save canonical view evidence outside the worktree")
    args = parser.parse_args(argv)
    if args.kind == "dev" and not args.root:
        parser.error("--root is required for dev")
    code = evaluate(args, runner)
    print(code)
    return 0 if code == "READY" else (2 if code == "GATE_CANNOT_RUN" else 5)


if __name__ == "__main__":
    raise SystemExit(main())
