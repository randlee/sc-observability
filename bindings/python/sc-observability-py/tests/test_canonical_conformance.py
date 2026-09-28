"""Generated Python models consume the shared canonical-envelope corpus."""
from __future__ import annotations

import json
from pathlib import Path
import unittest

from sc_observability import generated


class CanonicalConformanceTests(unittest.TestCase):
    def test_generated_models_consume_canonical_envelope_cases(self) -> None:
        path = (
            Path(__file__).resolve().parents[4]
            / "bindings/conformance/v1/conversion-cases.json"
        )
        cases = json.loads(path.read_text(encoding="utf-8"))
        canonical_cases = [
            case for case in cases if case.get("operation") == "canonical_envelope"
        ]
        self.assertTrue(canonical_cases)
        for case in canonical_cases:
            with self.subTest(case=case["id"]):
                if case["result"] == "decoded":
                    decoded = generated.from_wire(
                        "OutputCanonicalWireEnvelopeAdmissionDto", case["expected"]
                    )
                    self.assertIsInstance(
                        decoded, generated.OutputCanonicalWireEnvelopeError
                    )
                    error = decoded.error
                    if case["id"] == "canonical-envelope-unknown-error-kind":
                        self.assertIsInstance(
                            error, generated.OutputCanonicalFailureUnknownRemote
                        )
                        self.assertEqual(error.remote_kind, "future_export_failure")
                    elif case["id"] == "canonical-envelope-typed-queue-full-context":
                        self.assertIsInstance(
                            error, generated.OutputCanonicalFailureQueueFull
                        )
                        self.assertEqual(error.cause, "bounded cause")
                        self.assertEqual(error.docs, "https://example.test/recovery")
                        self.assertIn("depth", error.details)
                else:
                    with self.assertRaises(ValueError):
                        generated.from_wire(
                            "InputCanonicalWireEnvelopeAdmissionDto", case["value"]
                        )
                    failure = generated.from_wire(
                        "OutputFailure", case["expected_error"]
                    )
                    self.assertIsInstance(failure, generated.OutputFailureValidation)


if __name__ == "__main__":
    unittest.main()
