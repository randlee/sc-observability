import unittest

from validate_binding_runtime_dependencies import validate_consumer_manifest


class ConsumerEdgeFixtures(unittest.TestCase):
  def test_allowed_and_forbidden_edges(self):
    allowed = {
        "dependencies": {
            "sc-observability-binding-runtime": {"package": "sc-observability-binding-runtime"},
            "sc-observability-dto": {"package": "sc-observability-dto"},
            "sc-observability-types": {"package": "sc-observability-types"},
        }
    }
    validate_consumer_manifest(allowed, "sc-observability-tauri")

    forbidden = {"dependencies": {**allowed["dependencies"], "bad": {"package": "sc-observability"}}}
    with self.assertRaisesRegex(ValueError, "first-party edge drift"):
      validate_consumer_manifest(forbidden, "sc-observability-tauri")

  def test_resolves_workspace_aliases_in_target_and_dev_edges(self):
    workspace = {"workspace": {"dependencies": {
        "runtime_alias": {"package": "sc-observability-binding-runtime"},
        "dto_alias": {"package": "sc-observability-dto"},
        "types_alias": {"package": "sc-observability-types"},
    }}}
    manifest = {
        "dependencies": {"runtime_alias": {"workspace": True}},
        "target": {"cfg(unix)": {"dependencies": {
            "dto_alias": {"workspace": True},
        }}},
        "dev-dependencies": {"types_alias": {"workspace": True}},
    }
    validate_consumer_manifest(manifest, "sc-observability-tauri", workspace)

  def test_rejects_renamed_workspace_edge(self):
    workspace = {"workspace": {"dependencies": {
        "runtime_alias": {"package": "sc-observability"},
        "dto_alias": {"package": "sc-observability-dto"},
        "types_alias": {"package": "sc-observability-types"},
    }}}
    manifest = {
        "dependencies": {"runtime_alias": {"workspace": True}},
        "target": {"cfg(unix)": {"dependencies": {
            "dto_alias": {"workspace": True},
        }}},
        "dev-dependencies": {"types_alias": {"workspace": True}},
    }
    with self.assertRaisesRegex(ValueError, "first-party edge drift"):
      validate_consumer_manifest(manifest, "sc-observability-tauri", workspace)
