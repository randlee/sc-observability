"""Verify the shared checkout contract, then run one independently owned suite."""
import argparse
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
if __package__:
    from .dispatch import SUITES
else:
    from dispatch import SUITES


def run(suite, source_sha, output_dir, root=ROOT):
    if suite not in SUITES:
        raise ValueError(f"unknown suite: {suite}")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", source_sha):
        raise ValueError("source-sha must be a full 40-hex commit")
    output_dir = Path(output_dir)
    if not output_dir.is_absolute():
        raise ValueError("output-dir must be absolute")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if head != source_sha.lower():
        raise ValueError(f"checkout HEAD {head} does not match source-sha {source_sha}")
    entrypoint = root / "scripts" / "integrate" / "suites" / suite / "run.py"
    if not entrypoint.is_file():
        raise ValueError(f"missing suite runner: {entrypoint}")
    output_dir.mkdir(parents=True, exist_ok=True)
    return subprocess.call([sys.executable, str(entrypoint), "--source-sha", source_sha.lower(),
                            "--output-dir", str(output_dir)], cwd=root)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=SUITES, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args(argv)
    try:
        return run(args.suite, args.source_sha, args.output_dir)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"integration: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
