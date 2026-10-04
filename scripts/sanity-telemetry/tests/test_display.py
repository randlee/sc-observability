from __future__ import annotations

from zoneinfo import ZoneInfo, reset_tzpath

from display import format_local


def test_format_local_is_display_only_for_two_zones() -> None:
    assert format_local("2026-01-01T00:00:00Z", "America/Los_Angeles").endswith("-08:00")
    assert format_local("2026-07-01T00:00:00Z", "Europe/Berlin").endswith("+02:00")


def test_format_local_uses_supplied_tzdata_without_system_zoneinfo() -> None:
    reset_tzpath([])
    ZoneInfo.clear_cache()
    try:
        assert format_local("2026-01-01T00:00:00Z", "America/Los_Angeles") == "2025-12-31T16:00:00-08:00"
        assert format_local("2026-07-01T00:00:00Z", "America/Los_Angeles") == "2026-06-30T17:00:00-07:00"
    finally:
        reset_tzpath()
        ZoneInfo.clear_cache()
