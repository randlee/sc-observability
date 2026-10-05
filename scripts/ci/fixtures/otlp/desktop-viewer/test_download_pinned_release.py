"""Safety tests for pinned viewer release downloads."""

from __future__ import annotations

import errno
import hashlib
import json
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

import download_pinned_release as downloader


class ChunkedResponse:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.max_read = 0

    def __enter__(self) -> "ChunkedResponse":
        return self

    def __exit__(self, *_args: object) -> None:
        return None

    def geturl(self) -> str:
        return "https://example.test/archive.tgz"

    def read(self, size: int = -1) -> bytes:
        self.max_read = max(self.max_read, size)
        if not self.data:
            return b""
        block, self.data = self.data[:size], self.data[size:]
        return block


class PinnedReleaseDownloadTests(unittest.TestCase):
    def _entry(self, **overrides: str) -> dict[str, str]:
        entry = {
            "artifact_url": "https://example.test/release.tgz",
            "artifact_sha256": "a" * 64,
            "binary_sha256": "b" * 64,
            "binary_name": "viewer",
            "archive": "tar.gz",
        }
        entry.update(overrides)
        return entry

    def test_main_stages_on_destination_volume_before_atomic_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp).resolve() / "destination"
            output = parent / "viewer"
            real_replace = downloader.os.replace

            def download(_url: str, archive: Path, _digest: str) -> str:
                archive.write_bytes(b"archive")
                return "a" * 64

            def extract(_archive: Path, binary: Path, _lock: dict) -> str:
                binary.write_bytes(b"verified viewer")
                return "b" * 64

            def replace_on_destination_volume(source: Path, target: Path) -> None:
                # Model a destination mount distinct from the system temp volume.
                if source.parent.parent.resolve() != parent:
                    raise OSError(errno.EXDEV, "cross-device link")
                real_replace(source, target)

            with mock.patch.object(downloader.platform, "system", return_value="Darwin"), \
                    mock.patch.object(downloader.platform, "machine", return_value="arm64"), \
                    mock.patch.object(downloader, "_download_archive", side_effect=download), \
                    mock.patch.object(downloader, "_extract_binary", side_effect=extract), \
                    mock.patch.object(downloader.os, "replace", side_effect=replace_on_destination_volume), \
                    mock.patch.object(downloader.sys, "argv", ["download_pinned_release.py", str(output)]):
                self.assertEqual(downloader.main(), 0)
            self.assertEqual(output.read_bytes(), b"verified viewer")
            self.assertEqual(list(parent.iterdir()), [output])

    def test_manifest_requires_https_and_hex_sha256(self) -> None:
        lock = {"platforms": {"linux_amd64": self._entry(
            artifact_url="http://example.test/release.tgz")}}
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            downloader._validate_manifest(lock)
        lock = {"platforms": {"linux_amd64": self._entry(binary_sha256="not-a-digest")}}
        with self.assertRaisesRegex(ValueError, "hexadecimal"):
            downloader._validate_manifest(lock)

    def test_shipped_manifest_covers_all_supported_platforms(self) -> None:
        lock = json.loads(downloader.LOCK.read_text())
        downloader._validate_manifest(lock)
        self.assertEqual(set(lock["platforms"]),
                         {"darwin_arm64", "linux_amd64", "windows_amd64"})

    def test_host_platform_normalizes_architecture_aliases(self) -> None:
        for machine, system, expected in (
                ("x86_64", "Linux", "linux_amd64"),
                ("AMD64", "Windows", "windows_amd64"),
                ("arm64", "Darwin", "darwin_arm64")):
            with mock.patch.object(downloader.platform, "machine", return_value=machine), \
                    mock.patch.object(downloader.platform, "system", return_value=system):
                self.assertEqual(downloader._host_platform(), expected)

    def test_zip_and_tar_extraction_verify_binary_digest(self) -> None:
        content = b"viewer-bytes"
        digest = hashlib.sha256(content).hexdigest()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = root / "viewer.zip"
            with zipfile.ZipFile(archive, "w") as bundle:
                bundle.writestr("viewer.exe", content)
            entry = self._entry(archive="zip", binary_name="viewer.exe", binary_sha256=digest)
            output = root / "viewer.exe"
            self.assertEqual(downloader._extract_binary(archive, output, entry), digest)
            self.assertEqual(output.read_bytes(), content)
            tarball = root / "viewer.tar.gz"
            payload = root / "viewer"
            payload.write_bytes(content)
            with tarfile.open(tarball, "w:gz") as bundle:
                bundle.add(payload, arcname="viewer")
            tar_entry = self._entry(binary_sha256=digest)
            self.assertEqual(downloader._extract_binary(tarball, root / "tar-viewer", tar_entry), digest)

    def test_download_streams_with_timeout_and_checks_digest(self) -> None:
        content = b"pinned archive content"
        response = ChunkedResponse(content)
        expected = hashlib.sha256(content).hexdigest()
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / "archive.tgz"
            with mock.patch.object(downloader.urllib.request, "urlopen",
                                   return_value=response) as urlopen:
                actual = downloader._download_archive("https://example.test/archive.tgz",
                                                      target, expected)
            self.assertEqual(actual, expected)
            self.assertEqual(target.read_bytes(), content)
            self.assertEqual(urlopen.call_args.kwargs["timeout"], downloader.DOWNLOAD_TIMEOUT)
            self.assertLessEqual(response.max_read, downloader.CHUNK_SIZE)

    def test_download_refuses_redirect_away_from_https(self) -> None:
        class HttpRedirect(ChunkedResponse):
            def geturl(self) -> str:
                return "http://example.test/archive.tgz"

        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / "archive.tgz"
            with mock.patch.object(downloader.urllib.request, "urlopen",
                                   return_value=HttpRedirect(b"payload")):
                with self.assertRaisesRegex(ValueError, "redirected away from HTTPS"):
                    downloader._download_archive("https://example.test/archive.tgz",
                                                 target, "0" * 64)

    def test_refuses_symlink_output_without_touching_target(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "keep.bin"
            target.write_bytes(b"keep")
            link = root / "viewer"
            try:
                link.symlink_to(target)
            except OSError:
                # Windows can prohibit symlink creation without Developer Mode
                # or the necessary privilege. Still exercise the refusal path.
                with mock.patch.object(
                    Path,
                    "is_symlink",
                    autospec=True,
                    side_effect=lambda candidate: candidate == link,
                ):
                    with self.assertRaisesRegex(SystemExit, "symlink output"):
                        downloader._output_path(str(link))
            else:
                with self.assertRaisesRegex(SystemExit, "symlink output"):
                    downloader._output_path(str(link))
            self.assertEqual(target.read_bytes(), b"keep")

    def test_windows_output_path_appends_exe_suffix(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            requested = Path(temp) / "viewer"
            with mock.patch.object(downloader.platform, "system", return_value="Windows"):
                self.assertEqual(downloader._output_path(str(requested)),
                                 requested.parent.resolve() / "viewer.exe")
                self.assertEqual(downloader._output_path(str(requested.with_suffix(".exe"))),
                                 requested.parent.resolve() / "viewer.exe")
                self.assertEqual(downloader._output_path(str(requested.with_suffix(".EXE"))),
                                 requested.parent.resolve() / "viewer.EXE")

    def test_main_refuses_unknown_platform_before_selecting_an_output_path(self) -> None:
        with mock.patch.object(downloader, "_host_platform", return_value="plan9_mips"), \
                mock.patch.object(downloader.sys, "argv", ["download_pinned_release.py", "viewer"]), \
                mock.patch.object(downloader, "_output_path") as output_path:
            self.assertEqual(downloader.main(), 2)
        output_path.assert_not_called()


if __name__ == "__main__":
    unittest.main()
