"""Shared sanity cleanliness rule for git status --porcelain output."""

IGNORED_STATUS_PATHS = (".beads.gate.lock", ".sc-compose/")


def is_clean(status: str) -> bool:
    for line in status.splitlines():
        if not line:
            continue
        # Ignore only untracked generated scratch at the repository root.
        # Tracked edits/renames and similarly named files are always dirty.
        if line.startswith("?? "):
            path = line[3:]
            if any(path.startswith(ignored) if ignored.endswith("/") else path == ignored
                   for ignored in IGNORED_STATUS_PATHS):
                continue
        return False
    return True
