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
