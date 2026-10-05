from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest
import yaml

from import_sanity import SOURCE_KINDS, load_config, sources


ROOT = Path(__file__).parents[3]


def test_committed_config_and_state_ignore_contract() -> None:
    value = yaml.safe_load((ROOT / ".sc/telemetry.yaml").read_text())
    assert value == {
        "service": "sc-observability",
        "team": "sc-obs",
        "github": {"pr_url_template": "https://github.com/randlee/sc-observability/pull/{pr_number}"},
        "otlp": {"endpoint": "http://localhost:4318"},
        "store": {"path": ".sc/telemetry-state/store.sqlite"},
        "sources": [
            {"path": ".sc/qa-log/phase-d.jsonl", "kind": "qa", "phase": "phase-d"},
            {"path": ".sc/sanity-log/sanity-llm.jsonl", "kind": "sanity", "phase": "phase-d", "reviewer": "sanity-llm"},
            {"path": ".sc/sanity-log/phase-d.jsonl", "kind": "sanity", "phase": "phase-d"},
            {"path": ".sc/qa-log/phase-d-stats.jsonl", "kind": "finding-counts", "phase": "phase-d"},
        ],
    }
    assert subprocess.run(["git", "check-ignore", ".sc/telemetry-state/x"], cwd=ROOT).returncode == 0


def _selected_importer_schema() -> dict[str, object]:
    return json.loads((ROOT / "schema/config/sanity-importer/v1.json").read_text())


def _assert_selected_importer_schema_is_current(schema: dict[str, object]) -> None:
    metadata = schema.get("x-sc-observability")
    if not isinstance(metadata, dict) or metadata.get("contract") != "sanity-importer-yaml":
        raise AssertionError(
            "sanity importer YAML v1 contract mismatch; changed fields: /x-sc-observability/contract; "
            "selected v1 is immutable, so an intentional contract change requires "
            "schema/config/sanity-importer/v2.json rather than overwriting v1"
        )
    if metadata.get("selected_version") != 1:
        raise AssertionError(
            "sanity importer YAML v1 contract mismatch; changed fields: /x-sc-observability/selected_version; "
            "selected v1 is immutable, so an intentional contract change requires "
            "schema/config/sanity-importer/v2.json rather than overwriting v1"
        )
    properties = schema.get("properties")
    if not isinstance(properties, dict) or set(properties) != {"service", "team", "github", "otlp", "store", "sources"}:
        raise AssertionError(
            "sanity importer YAML v1 contract mismatch; changed fields: /properties; selected v1 is immutable, "
            "so an intentional contract change requires schema/config/sanity-importer/v2.json rather than overwriting v1"
        )


def test_selected_importer_v1_schema_matches_real_sources_validation(tmp_path: Path) -> None:
    schema = _selected_importer_schema()
    accepted_baseline = json.loads(
        subprocess.run(
            [
                "git",
                "show",
                "9ebc96d6607967122c898dcf91e63a5b6d861477:schema/config/sanity-importer/v1.json",
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    assert schema == accepted_baseline, (
        "sanity importer YAML v1 selected snapshot differs from accepted baseline; "
        "an intentional contract change requires schema/config/sanity-importer/v2.json rather than overwriting v1"
    )
    _assert_selected_importer_schema_is_current(schema)
    source_schema = schema["properties"]["sources"]
    source_item_schema = source_schema["items"]
    assert source_schema["default"] == []
    assert source_item_schema["required"] == ["path", "kind"]
    assert source_item_schema["properties"]["path"]["minLength"] == 1
    assert source_item_schema["properties"]["phase"]["type"] == ["string", "null"]
    assert source_item_schema["properties"]["reviewer"]["type"] == ["string", "null"]
    assert source_item_schema["properties"]["kind"]["enum"] == list(SOURCE_KINDS)
    assert source_item_schema["additionalProperties"] is True
    assert schema["additionalProperties"] is True

    path = tmp_path / "importer.yaml"
    path.write_text(
        "service: schema-test\nconsumer_extension: preserve-me\nsources:\n"
        "  - path: events.jsonl\n    kind: sanity\n    phase: phase-e\n    reviewer: sanity\n    consumer_extension: preserve-me\n"
    )
    loaded = load_config(path)
    assert loaded["consumer_extension"] == "preserve-me"
    assert sources(loaded)[0].path == "events.jsonl"

    for body in [
        "sources: not-a-list\n",
        "sources:\n  - kind: qa\n",
        "sources:\n  - path: ''\n    kind: qa\n",
        "sources:\n  - path: events.jsonl\n    kind: qa\n    phase: 1\n",
        "sources:\n  - path: events.jsonl\n    kind: qa\n    reviewer: 1\n",
        "sources:\n  - path: events.jsonl\n    kind: unsupported\n",
        "sources:\n  - path: duplicate.jsonl\n    kind: qa\n  - path: duplicate.jsonl\n    kind: sanity\n",
    ]:
        path.write_text(body)
        with pytest.raises(ValueError):
            load_config(path)


def test_importer_schema_mismatch_names_contract_version_changed_field_and_new_version_remedy() -> None:
    schema = _selected_importer_schema()
    schema["properties"].pop("sources")
    with pytest.raises(AssertionError) as error:
        _assert_selected_importer_schema_is_current(schema)
    message = str(error.value)
    assert "sanity importer YAML v1" in message
    assert "/properties" in message
    assert "immutable" in message
    assert "schema/config/sanity-importer/v2.json" in message
