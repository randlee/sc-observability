"""Generated Python models consume the shared canonical-envelope corpus."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from sc_observability import generated


def corpus_path(test_file: Path) -> Path:
    """Use bundled test data when relocated, or the checkout's shared corpus."""
    directory = test_file.resolve().parent
    bundled = directory / "conformance/v1/conversion-cases.json"
    if bundled.is_file():
        return bundled
    for parent in directory.parents:
        shared = parent / "bindings/conformance/v1/conversion-cases.json"
        if shared.is_file():
            return shared
    raise FileNotFoundError(f"canonical conformance corpus missing beside {test_file}")


class CanonicalConformanceTests(unittest.TestCase):
    def test_relocated_suite_uses_bundled_corpus(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tests = Path(temporary) / "tests"
            bundled = tests / "conformance/v1/conversion-cases.json"
            bundled.parent.mkdir(parents=True)
            bundled.write_bytes(corpus_path(Path(__file__)).read_bytes())
            self.assertEqual(corpus_path(tests / "test_canonical_conformance.py"), bundled.resolve())

    def test_missing_corpus_fails_explicitly(self) -> None:
        # The filesystem root cannot have enough parents for the old parents[4].
        with self.assertRaisesRegex(FileNotFoundError, "conformance corpus missing"):
            corpus_path(Path(Path.cwd().anchor) / "missing-python-suite.py")

    def test_generated_models_consume_canonical_envelope_cases(self) -> None:
        path = corpus_path(Path(__file__))
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

    def test_generated_span_signal_models_use_tagged_variant_names(self) -> None:
        for prefix in ("Input", "Output"):
            for variant in ("Started", "Event", "Ended"):
                self.assertTrue(hasattr(generated, f"{prefix}SpanSignal{variant}"))
            for index in range(3):
                self.assertFalse(hasattr(generated, f"{prefix}SpanSignal{index}"))


if __name__ == "__main__":
    unittest.main()
