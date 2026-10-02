"""Real-process fixtures for the Wave 5 telemetry composition tests.

The tests deliberately build and run the public artifacts.  They do not import
the source checkout or use the private test-double constructor: a passing test
therefore exercises the wheel, the installed CLI, durable SQLite storage, and
the OTLP/JSON encoder together.
"""
from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from collections import defaultdict
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import pytest


ROOT = Path(__file__).resolve().parents[2]
GOLDENS = ROOT / "crates/sc-observability-types/tests/fixtures/otlp_submission/golden"


class CaptureCollector:
    """A loopback OTLP/JSON server with per-path responses and retained bodies."""

    def __init__(self, port: int = 0, *, bound_socket: socket.socket | None = None) -> None:
        self.requests: dict[str, list[dict[str, Any]]] = defaultdict(list)
        self.statuses: dict[str, int] = defaultdict(lambda: 200)
        self._lock = threading.Lock()
        self._blocked: dict[str, threading.Event] = {}
        collector = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self) -> None:  # noqa: N802 - stdlib callback name
                length = int(self.headers.get("Content-Length", "0"))
                raw = self.rfile.read(length)
                try:
                    body = json.loads(raw)
                except json.JSONDecodeError:
                    body = {"_raw": raw.decode("utf-8", errors="replace")}
                with collector._lock:
                    collector.requests[self.path].append(body)
                    status = collector.statuses[self.path]
                    unblock = collector._blocked.get(self.path)
                if unblock is not None:
                    unblock.wait()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", "2")
                self.end_headers()
                try:
                    self.wfile.write(b"{}")
                except (BrokenPipeError, ConnectionResetError):
                    # A recovery test can intentionally terminate the process
                    # owning this request before its held response is released.
                    pass

            def log_message(self, _format: str, *_args: object) -> None:
                return

        if bound_socket is None:
            self._server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
            self._needs_activate = False
        else:
            self._server = ThreadingHTTPServer(bound_socket.getsockname(), Handler, bind_and_activate=False)
            self._server.socket.close()
            self._server.socket = bound_socket
            self._server.server_address = bound_socket.getsockname()
            self._needs_activate = True
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)

    @classmethod
    def from_socket(cls, bound_socket: socket.socket) -> CaptureCollector:
        """Adopt an already-bound socket and begin listening only on ``start``."""
        return cls(bound_socket=bound_socket)

    @property
    def endpoint(self) -> str:
        host, port = self._server.server_address[:2]
        return f"http://{host}:{port}"

    def start(self) -> None:
        if self._needs_activate:
            self._server.server_activate()
            self._needs_activate = False
        self._thread.start()

    def stop(self) -> None:
        self._server.shutdown()
        self._thread.join(timeout=5)
        self._server.server_close()

    def clear(self) -> None:
        with self._lock:
            self.requests.clear()

    def block(self, path: str) -> None:
        """Hold subsequent responses on ``path`` until ``unblock`` is called."""
        with self._lock:
            self._blocked[path] = threading.Event()

    def unblock(self, path: str) -> None:
        """Release a response previously held by ``block``."""
        with self._lock:
            blocked = self._blocked.pop(path, None)
        if blocked is not None:
            blocked.set()

    def wait_for(self, path: str, *, count: int = 1, timeout: float = 10) -> list[dict[str, Any]]:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            with self._lock:
                received = list(self.requests[path])
            if len(received) >= count:
                return received
            time.sleep(0.05)
        raise AssertionError(f"collector did not receive {count} request(s) for {path}: {self.requests}")


@pytest.fixture(scope="session")
def installed_artifacts(tmp_path_factory: pytest.TempPathFactory) -> dict[str, Path]:
    """Build the release-config wheel and install the public command once."""
    root = tmp_path_factory.mktemp("installed-telemetry")
    venv = root / "venv"
    subprocess.run([sys.executable, "-m", "venv", str(venv)], check=True)
    python = venv / "bin" / "python"
    subprocess.run(
        [str(python), "-m", "pip", "install", "--upgrade", "pip==25.3", "maturin==1.10.2"],
        check=True,
    )
    wheel_dir = root / "wheel"
    subprocess.run(
        [str(python), "-m", "maturin", "build", "--release", "--manifest-path",
         str(ROOT / "bindings/python/sc-observability-py/Cargo.toml"),
         "--features", "otlp-telemetry", "--out", str(wheel_dir)],
        cwd=ROOT,
        check=True,
    )
    wheels = sorted(wheel_dir.glob("*.whl"))
    assert len(wheels) == 1, f"expected one wheel, found {wheels}"
    subprocess.run([str(python), "-m", "pip", "install", str(wheels[0])], check=True)
    cli_root = root / "cli"
    subprocess.run(
        ["cargo", "install", "--locked", "--path", "crates/sc-otel-cli", "--root", str(cli_root),
         "--force"],
        cwd=ROOT,
        check=True,
    )
    return {"root": root, "python": python, "cli": cli_root / "bin" / "sc-otel"}


@pytest.fixture
def collector() -> Iterator[CaptureCollector]:
    server = CaptureCollector()
    server.start()
    try:
        yield server
    finally:
        server.stop()


@pytest.fixture
def telemetry_config(tmp_path: Path, collector: CaptureCollector) -> Path:
    config = tmp_path / "telemetry.yaml"
    config.write_text(
        "\n".join((
            "service: telemetry-e2e",
            "otlp:",
            f"  endpoint: {collector.endpoint}",
            "  timeout_ms: 1000",
            "store:",
            "  path: store.sqlite",
            "  max_bytes: 10485760",
            "",
        )),
        encoding="utf-8",
    )
    return config


def run_installed_python(artifacts: dict[str, Path], script: str, *, cwd: Path, input: str = "") -> subprocess.CompletedProcess[str]:
    """Run a plain file, never ``python -c`` and never a caller event loop."""
    path = cwd / "installed_frontend.py"
    path.write_text(script, encoding="utf-8")
    return subprocess.run([str(artifacts["python"]), str(path)], cwd=cwd, input=input,
                          text=True, capture_output=True, check=False)


def run_cli(artifacts: dict[str, Path], *args: str, cwd: Path, input: str = "") -> subprocess.CompletedProcess[str]:
    return subprocess.run([str(artifacts["cli"]), *args], cwd=cwd, input=input,
                          text=True, capture_output=True, check=False)


def canonical_json(value: object) -> str:
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


def normalise_system_generated_fields(envelope: dict[str, Any], input: dict[str, Any]) -> dict[str, Any]:
    """Replace only D29's documented invocation-generated values.

    Separate public processes necessarily select different current timestamps
    and random trace/span IDs.  The SubmissionInput owns explicit values; no
    caller-provided field is normalized here.
    """
    result = json.loads(json.dumps(envelope))
    input_by_signal = {"logs": input.get("logs", []), "spans": input.get("spans", [])}
    for signal, records in input_by_signal.items():
        for record, submitted in zip(result.get(signal, []), records):
            value = record["record"]
            for field in ("observed_time", "trace_id", "span_id"):
                if field not in submitted and field in value:
                    value[field] = f"$GENERATED_{field.upper()}"
    return result


def reserve_loopback_sockets(count: int) -> list[socket.socket]:
    """Reserve distinct loopback ports until the caller deliberately releases them."""
    sockets: list[socket.socket] = []
    try:
        for _ in range(count):
            reserved = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            reserved.bind(("127.0.0.1", 0))
            sockets.append(reserved)
        return sockets
    except BaseException:
        for reserved in sockets:
            reserved.close()
        raise


def loopback_endpoint(reserved: socket.socket) -> str:
    host, port = reserved.getsockname()[:2]
    return f"http://{host}:{port}"


@pytest.fixture
def dead_collector_endpoint() -> Iterator[str]:
    """Keep a bound, non-listening socket so the endpoint remains unavailable."""
    reserved, = reserve_loopback_sockets(1)
    try:
        yield loopback_endpoint(reserved)
    finally:
        reserved.close()


class PinnedViewer(dict[str, str]):
    """A hash-pinned viewer that tests may stop and restart on its same ports."""

    def __init__(self, binary: str, manifest: dict[str, str], state: Path) -> None:
        self.binary = binary
        self.manifest = manifest
        self.state = state
        self.harness = ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py"
        self._reservations = reserve_loopback_sockets(3)
        self.http, self.grpc, self.ui = (int(port.getsockname()[1]) for port in self._reservations)
        assert len({self.http, self.grpc, self.ui}) == 3
        super().__init__(otlp=f"http://127.0.0.1:{self.http}", rpc=f"http://127.0.0.1:{self.ui}/rpc")

    def start(self) -> None:
        # The harness owns the listeners, so release the deterministic
        # reservations immediately before it is invoked.
        for reserved in self._reservations:
            reserved.close()
        self._reservations = []
        started = subprocess.run(
            [sys.executable, str(self.harness), "start", "--binary", self.binary,
             "--version", self.manifest["version"], "--binary-sha256", self.manifest["binary_sha256"],
             "--state-dir", str(self.state), "--http", str(self.http), "--grpc", str(self.grpc), "--ui", str(self.ui)],
            check=False, text=True, capture_output=True,
        )
        assert started.returncode == 0, started.stdout + started.stderr

    def stop(self) -> None:
        stopped = subprocess.run(
            [sys.executable, str(self.harness), "stop", "--state-dir", str(self.state), "--remove-state"],
            check=False, text=True, capture_output=True,
        )
        assert stopped.returncode == 0, stopped.stdout + stopped.stderr

    def restart(self) -> None:
        self.stop()
        self.start()


@pytest.fixture
def pinned_viewer(tmp_path: Path) -> Iterator[PinnedViewer]:
    """Start only the caller-provided, hash-pinned desktop viewer binary.

    The workflow downloads it using the repository verifier; local developers
    opt in by exporting the same binary path.  This avoids an unpinned network
    download from a test while ensuring the CI test owns its process and state.
    """
    binary = os.environ.get("TELEMETRY_E2E_VIEWER_BINARY")
    if not binary:
        message = "set TELEMETRY_E2E_VIEWER_BINARY to run pinned viewer readback"
        if os.environ.get("GITHUB_ACTIONS", "").lower() == "true":
            pytest.fail(f"{message}; CI must not skip viewer readback", pytrace=False)
        pytest.skip(message)
    manifest = json.loads((ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/release.json").read_text())
    viewer = PinnedViewer(binary, manifest, tmp_path / "viewer-state")
    viewer.start()
    try:
        yield viewer
    finally:
        if viewer.state.exists():
            viewer.stop()


def rpc(url: str, method: str, params: list[object]) -> object:
    payload = json.dumps({"jsonrpc": "2.0", "id": "d32", "method": method,
                          "params": params}).encode("utf-8")
    request = urllib.request.Request(url, data=payload, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=10) as response:
        decoded = json.loads(response.read())
    assert "error" not in decoded, decoded
    return decoded["result"]
