"""Display-only local-time helpers for importer commands."""
from __future__ import annotations

from datetime import datetime
from zoneinfo import ZoneInfo


def format_local(ts: str, tz: str) -> str:
    """Render a stored UTC timestamp in a requested timezone without storing local time."""
    instant = datetime.fromisoformat(ts.replace("Z", "+00:00"))
    return instant.astimezone(ZoneInfo(tz)).isoformat()
