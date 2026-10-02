from __future__ import annotations

from display import format_local


def test_format_local_is_display_only_for_two_zones() -> None:
    assert format_local("2026-01-01T00:00:00Z", "America/Los_Angeles").endswith("-08:00")
    assert format_local("2026-07-01T00:00:00Z", "Europe/Berlin").endswith("+02:00")
