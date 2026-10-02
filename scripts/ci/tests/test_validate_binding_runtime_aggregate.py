"""aggregate() validates every packaged-consumer artifact it is given."""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import validate_binding_runtime as gate  # noqa: E402
from _hashing import digest  # noqa: E402


def write_platforms(directory: Path) -> None:
    for system in ("Darwin", "Linux", "Windows"):
        profiles = {}
        for profile in ("debug", "release"):
            log = directory / f"{system.lower()}-{profile}.log"
            log.write_text(f"{system} {profile}\n", encoding="utf-8")
            profiles[profile] = {
                "status": "passed", "cases": gate.cases(),
                "log": log.name, "sha256": digest(log),
            }
        (directory / f"{system.lower()}.json").write_text(json.dumps({
            "platform": system, "runtime_source_sha256": gate.source_digest(),
            "profiles": profiles,
        }), encoding="utf-8")


def write_consumer(path: Path, denied: bool = True) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({
        "runtime_source_sha256": gate.source_digest(), "status": "passed",
        "isolation_probes": {"network": {"denied": denied}},
        "archives": [gate.PACKAGE], "consumer_output": "BINDING_CONSUMER_OK runtime core+bridge",
    }), encoding="utf-8")
    return path


class AggregateConsumerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.platforms = self.root / "platforms"
        self.platforms.mkdir()
        write_platforms(self.platforms)

    def test_three_valid_consumers_pass_and_message_names_all(self):
        ubuntu = write_consumer(self.root / "ubuntu" / "consumer.json")
        macos = write_consumer(self.root / "macos" / "consumer.json")
        windows = write_consumer(self.root / "windows" / "consumer.json")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            gate.aggregate(self.platforms, [ubuntu, macos, windows])
        text = out.getvalue()
        self.assertIn("3 isolated packaged consumer(s)", text)
        for consumer in (ubuntu, macos, windows):
            self.assertIn(str(consumer), text)

    def test_third_consumer_with_failed_isolation_check_fails(self):
        ubuntu = write_consumer(self.root / "ubuntu" / "consumer.json")
        macos = write_consumer(self.root / "macos" / "consumer.json")
        windows = write_consumer(self.root / "windows" / "consumer.json", denied=False)
        with self.assertRaisesRegex(RuntimeError, "isolated consumer proof incomplete"):
            gate.aggregate(self.platforms, [ubuntu, macos, windows])

    def test_second_consumer_with_failed_isolation_check_fails(self):
        ubuntu = write_consumer(self.root / "ubuntu" / "consumer.json")
        macos = write_consumer(self.root / "macos" / "consumer.json", denied=False)
        with self.assertRaisesRegex(RuntimeError, "isolated consumer proof incomplete"):
            gate.aggregate(self.platforms, [ubuntu, macos])

    def test_first_consumer_with_skipped_status_fails(self):
        ubuntu = write_consumer(self.root / "ubuntu" / "consumer.json")
        proof = json.loads(ubuntu.read_text(encoding="utf-8"))
        proof["status"] = "skipped"
        ubuntu.write_text(json.dumps(proof), encoding="utf-8")
        macos = write_consumer(self.root / "macos" / "consumer.json")
        with self.assertRaisesRegex(RuntimeError, "isolated consumer proof incomplete"):
            gate.aggregate(self.platforms, [ubuntu, macos])


if __name__ == "__main__":
    unittest.main()
