import base64
import hashlib
import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / ".github" / "scripts"))

import _publish_retry  # noqa: E402
import npm_release  # noqa: E402


class NoNetworkMixin:
    def setUp(self):
        self.socket_patch = patch.object(socket.socket, "connect", side_effect=AssertionError("network blocked"))
        self.socket_patch.start()

    def tearDown(self):
        self.socket_patch.stop()


class InvocationShapeTests(NoNetworkMixin, unittest.TestCase):
    def test_pypi_shape_has_skip_existing_and_maps_exit_codes(self):
        command = _publish_retry.pypi_invocation("pypi", ["dist/a.whl", "dist/a.tar.gz"])
        self.assertIn("--skip-existing", command)
        with patch("_publish_retry.subprocess.run", return_value=subprocess.CompletedProcess(command, 0)) as runner:
            _publish_retry.invoke(command)
            runner.assert_called_once()
        with patch("_publish_retry.subprocess.run", return_value=subprocess.CompletedProcess(command, 1)):
            with self.assertRaisesRegex(RuntimeError, "failed"):
                _publish_retry.invoke(command)

    def test_crates_io_shape_and_exit_mapping(self):
        command = _publish_retry.crates_io_invocation("crates/example/Cargo.toml")
        self.assertEqual(command[-1], "--locked")
        with patch("_publish_retry.subprocess.run", return_value=subprocess.CompletedProcess(command, 0)):
            _publish_retry.invoke(command)
        with patch("_publish_retry.subprocess.run", return_value=subprocess.CompletedProcess(command, 2)):
            with self.assertRaises(RuntimeError):
                _publish_retry.invoke(command)

    def test_github_release_probe_shape_and_exit_mapping(self):
        command = _publish_retry.github_release_probe_invocation("owner/repo", "v1.4.0")
        self.assertEqual(command[:4], ["gh", "release", "view", "v1.4.0"])
        with patch("_publish_retry.subprocess.run", return_value=subprocess.CompletedProcess(command, 0)):
            _publish_retry.invoke(command)
        with patch("_publish_retry.subprocess.run", return_value=subprocess.CompletedProcess(command, 1)):
            with self.assertRaises(RuntimeError):
                _publish_retry.invoke(command)


class NpmOwnedPreflightTests(NoNetworkMixin, unittest.TestCase):
    def setUp(self):
        super().setUp()
        self.tempdir = tempfile.TemporaryDirectory(prefix="npm-retry-")
        self.archive = Path(self.tempdir.name) / "client-1.4.0.tgz"
        self.archive.write_bytes(b"immutable npm bytes")
        self.verified = [("@sc-observability/client", self.archive)]
        self.verify_patch = patch.object(npm_release, "verify", return_value=self.verified)
        self.verify_patch.start()

    def tearDown(self):
        self.verify_patch.stop()
        self.tempdir.cleanup()
        super().tearDown()

    def _integrity(self):
        return "sha512-" + base64.b64encode(hashlib.sha512(self.archive.read_bytes()).digest()).decode()

    def test_missing_version_publishes_once(self):
        with patch.object(npm_release, "registry_version", return_value=None), patch("npm_release.subprocess.run") as runner:
            runner.return_value.returncode = 0
            npm_release.publish({}, "v1.4.0", Path("npm-dist"), dry_run=False)
            self.assertEqual(runner.call_args.args[0][:3], ["npm", "publish", str(self.archive.resolve())])

    def test_already_present_version_skips_publish_and_succeeds(self):
        existing = {"name": "@sc-observability/client", "version": "1.4.0", "dist": {"integrity": self._integrity()}}
        with patch.object(npm_release, "registry_version", return_value=existing), patch("npm_release.subprocess.run") as runner:
            npm_release.publish({}, "v1.4.0", Path("npm-dist"), dry_run=False)
            runner.assert_not_called()

    def test_registry_error_fails_closed(self):
        with patch.object(npm_release, "registry_version", side_effect=RuntimeError("registry lookup indeterminate")), patch("npm_release.subprocess.run") as runner:
            with self.assertRaisesRegex(RuntimeError, "indeterminate"):
                npm_release.publish({}, "v1.4.0", Path("npm-dist"), dry_run=False)
            runner.assert_not_called()


if __name__ == "__main__":
    unittest.main()
