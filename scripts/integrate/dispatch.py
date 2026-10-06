"""Validate integration selections, prepare the matrix, or dispatch through gh."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

SUITES = ("wheel-cli-viewer", "collector", "wheels", "tauri", "rust-consumers")
RUNNERS = {"macos": "macos-14", "windows": "windows-2022", "linux": "ubuntu-24.04"}


def selectors(value, allowed):
    selected = [part.strip() for part in value.split(",")]
    if not selected or any(item not in allowed for item in selected):
        raise ValueError(f"invalid selection {value!r}; choose comma-separated: {','.join(allowed)}")
    if len(set(selected)) != len(selected):
        raise ValueError(f"duplicate selectors: {value!r}")
    return selected


def selection(suites, platforms):
    suites = selectors(suites, SUITES)
    platforms = selectors(platforms, RUNNERS)
    matrix = {"include": [{"suite": suite, "os": platform, "runner": RUNNERS[platform]}
                          for suite in suites for platform in platforms]}
    full = set(suites) == set(SUITES) and set(platforms) == set(RUNNERS)
    coverage = ("Full selection" if full else "Partial selection (not phase-end coverage)")
    coverage += f": suites={','.join(suites)}; os={','.join(platforms)}; {len(matrix['include'])} jobs"
    return suites, platforms, matrix, coverage


def gh(*args):
    return subprocess.check_output(["gh", *args], text=True)


def dispatch(branch, suites, platforms):
    token = uuid.uuid4().hex
    gh("workflow", "run", "integration.yml", "--ref", branch,
       "-f", f"suite={','.join(suites)}", "-f", f"os={','.join(platforms)}",
       "-f", f"dispatch_id={token}")
    # Correlate our request, not the newest run (another developer may dispatch).
    for attempt in range(30):
        runs = json.loads(gh("run", "list", "--workflow", "integration.yml", "--event",
                             "workflow_dispatch", "--limit", "100", "--json", "displayTitle,url"))
        for run in runs:
            if run["displayTitle"] == f"Integration {token}":
                return run["url"]
        if attempt < 29:
            time.sleep(2)
    raise RuntimeError(f"dispatch accepted ({token}), but run URL not yet available; "
                       "use gh run list --workflow integration.yml")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("branch", nargs="?")
    parser.add_argument("--suite", default=",".join(SUITES))
    parser.add_argument("--os", default=",".join(RUNNERS))
    parser.add_argument("--matrix", action="store_true", help="validate and print matrix without dispatch")
    args = parser.parse_args(argv)
    try:
        suites, platforms, matrix, coverage = selection(args.suite, args.os)
        if args.matrix:
            encoded = json.dumps(matrix, separators=(",", ":"))
            print(encoded)
            if os.environ.get("GITHUB_OUTPUT"):
                with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
                    output.write(f"matrix={encoded}\n")
            if os.environ.get("GITHUB_STEP_SUMMARY"):
                with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as summary:
                    summary.write(coverage + "\n\nSelection is not successful execution. "
                                  "Inspect every selected job's native conclusion.\n")
        else:
            if not args.branch:
                parser.error("branch is required for dispatch")
            print(coverage, flush=True)
            print(dispatch(args.branch, suites, platforms))
        return 0
    except (ValueError, RuntimeError, OSError, subprocess.CalledProcessError) as error:
        print(f"integration: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
