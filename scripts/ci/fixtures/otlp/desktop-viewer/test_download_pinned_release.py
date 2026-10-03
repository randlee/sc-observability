"""Safety tests for pinned viewer release downloads."""

from __future__ import annotations

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
        entry = {"artifact_url": "https://example.test/release.tgz",
                 "artifact_sha256": "a" * 64, "binary_sha256": "b" * 64,
                 "binary_name": "viewer", "archive": "tar.gz"}
        entry.update(overrides)
        return entry

    def test_manifest_requires_https_and_hex_sha256(self) -> None:
        lock = {"platforms": {"linux_amd64": self._entry(artifact_url="http://example.test/r.tgz")}}
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            downloader._validate_manifest(lock)
        lock = {"platforms": {"linux_amd64": self._entry(binary_sha256="not-a-digest")}}
        with self.assertRaisesRegex(ValueError, "hexadecimal"):
            downloader._validate_manifest(lock)

    def test_every_entry_is_validated_including_non_host(self) -> None:
        host = downloader._host_platform()
        other = "windows_amd64" if host != "windows_amd64" else "linux_amd64"
        lock = {"platforms": {host: self._entry(),
                              other: self._entry(artifact_sha256="tampered")}}
        with self.assertRaisesRegex(ValueError, f"{other}: artifact_sha256"):
            downloader._validate_manifest(lock)

    def test_shipped_manifest_is_valid_and_covers_three_platforms(self) -> None:
        lock = json.loads(downloader.LOCK.read_text())
        downloader._validate_manifest(lock)
        self.assertEqual(set(lock["platforms"]),
                         {"darwin_arm64", "linux_amd64", "windows_amd64"})

    def test_host_platform_maps_machine_names(self) -> None:
        for machine, system, expected in (
                ("x86_64", "Linux", "linux_amd64"), ("AMD64", "Windows", "windows_amd64"),
                ("aarch64", "Linux", "linux_arm64"), ("arm64", "Darwin", "darwin_arm64")):
            with mock.patch.object(downloader.platform, "machine", return_value=machine), \
                    mock.patch.object(downloader.platform, "system", return_value=system):
                self.assertEqual(downloader._host_platform(), expected)

    def test_unknown_host_exits_two(self) -> None:
        with mock.patch.object(downloader, "_host_platform", return_value="plan9_mips"):
            self.assertEqual(downloader.main(), 2)

    def test_zip_and_tar_extraction_verify_binary_digest(self) -> None:
        content = b"viewer-bytes"
        digest = hashlib.sha256(content).hexdigest()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = root / "a.zip"
            with zipfile.ZipFile(archive, "w") as bundle:
                bundle.writestr("README.md", "x")
                bundle.writestr("viewer.exe", content)
            entry = self._entry(archive="zip", binary_name="viewer.exe", binary_sha256=digest)
            out = root / "out.exe"
            self.assertEqual(downloader._extract_binary(archive, out, entry), digest)
            self.assertEqual(out.read_bytes(), content)
            with self.assertRaisesRegex(SystemExit, "binary SHA-256 mismatch"):
                downloader._extract_binary(archive, root / "bad.exe",
                                           dict(entry, binary_sha256="0" * 64))
            tarball = root / "a.tar.gz"
            payload = root / "viewer"
            payload.write_bytes(content)
            with tarfile.open(tarball, "w:gz") as bundle:
                bundle.add(payload, arcname="viewer")
            tar_entry = self._entry(archive="tar.gz", binary_name="viewer", binary_sha256=digest)
            self.assertEqual(downloader._extract_binary(tarball, root / "t", tar_entry), digest)

    def test_output_path_gets_exe_on_windows(self) -> None:
        with tempfile.TemporaryDirectory() as temp, \
                mock.patch.object(downloader, "_is_windows", return_value=True):
            self.assertEqual(downloader._output_path(str(Path(temp) / "viewer")).name,
                             "viewer.exe")

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
            link.symlink_to(target)
            with self.assertRaisesRegex(SystemExit, "symlink output"):
                downloader._output_path(str(link))
            self.assertEqual(target.read_bytes(), b"keep")

    def test_main_stages_release_in_output_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp) / "destination"
            output = parent / "viewer"
            staged_paths: list[Path] = []

            def download(_url: str, archive_path: Path, _digest: str) -> str:
                staged_paths.append(archive_path)
                archive_path.write_bytes(b"archive")
                return "a" * 64

            def extract(_archive_path: Path, binary_path: Path, _entry: dict[str, str]) -> str:
                staged_paths.append(binary_path)
                binary_path.write_bytes(b"verified viewer")
                return "b" * 64

            with mock.patch.object(downloader, "_host_platform", return_value="linux_amd64"), \
                    mock.patch.object(downloader, "_download_archive", side_effect=download), \
                    mock.patch.object(downloader, "_extract_binary", side_effect=extract), \
                    mock.patch.object(downloader.sys, "argv", ["download_pinned_release.py", str(output)]):
                self.assertEqual(downloader.main(), 0)

            self.assertEqual(output.read_bytes(), b"verified viewer")
            self.assertEqual(len(staged_paths), 2)
            for path in staged_paths:
                self.assertEqual(path.parent.parent, parent.resolve())


if __name__ == "__main__":
    unittest.main()
