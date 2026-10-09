"""Real-process fixtures for the telemetry end-to-end qualification.

The acceptance oracle is a locally started, hash-pinned official OpenTelemetry
Collector (``otelcol-contrib``) whose ``file`` exporter writes every received
log, span and metric to disk.  The tests build and run the public artifacts
(the ``sc-otel`` executable, the installed Python wheel and the native example
binary) and read the Collector's exported files back.  Nothing here stands in
for the Collector.

Synchronization is by content: a test waits on the Collector's files under one
bounded watchdog deadline and never sleeps to let an export happen.
"""
from __future__ import annotations

import hashlib
import json
import os
import platform
import queue
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import Any

import pytest


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = Path(__file__).with_name("collector-release.json")
BUILD_TIMEOUT_SECONDS = 20 * 60
INSTALL_TIMEOUT_SECONDS = 2 * 60
PROCESS_TIMEOUT_SECONDS = 60
COLLECTOR_START_TIMEOUT_SECONDS = 30
COLLECTOR_EXPORT_TIMEOUT_SECONDS = 30
COLLECTOR_STOP_TIMEOUT_SECONDS = 10
POLL_SECONDS = 0.05
MACHINES = {"x86_64": "amd64", "amd64": "amd64", "aarch64": "arm64", "arm64": "arm64"}
SIGNALS = ("logs", "traces", "metrics")
VIEWER_TIMEOUT_SECONDS = 30
VIEWER_MACHINES = MACHINES
VIEWER_HARNESS = ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py"


def run_process(command: list[str], *, timeout: float, **kwargs: Any) -> subprocess.CompletedProcess[str]:
    """Run an e2e subprocess; a timeout fails the test naming the command and its output."""
    try:
        return subprocess.run(command, timeout=timeout, **kwargs)
    except subprocess.TimeoutExpired as error:
        pytest.fail(
            f"subprocess timed out after {timeout}s: {error.cmd!r}\n"
            f"stdout:\n{error.output or ''}\nstderr:\n{error.stderr or ''}",
        )


def host_platform() -> str:
    machine = platform.machine().lower()
    return f"{platform.system().lower()}_{MACHINES.get(machine, machine)}"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


# ---------------------------------------------------------------------------
# Official Collector


def attributes(entries: list[dict[str, Any]] | None) -> dict[str, Any]:
    """Flatten OTLP/JSON ``KeyValue`` entries to a plain mapping with typed values."""
    result: dict[str, Any] = {}
    for entry in entries or []:
        value = entry["value"]
        (kind, raw), = value.items()
        result[entry["key"]] = int(raw) if kind == "intValue" else raw
    return result


class OfficialCollector:
    """One official Collector process: OTLP/HTTP receiver, ``file`` exporters per signal.

    The receiver binds port 0, so the operating system picks a free port and no
    port is ever chosen-then-released.  The address is read from the
    Collector's own ``Starting HTTP server`` log line.
    """

    def __init__(self, binary: Path, work: Path) -> None:
        self.binary = binary
        self.work = work
        self.files = {signal: work / f"{signal}.jsonl" for signal in SIGNALS}
        self.config = work / "collector.yaml"
        self.stderr_path = work / "collector.stderr.log"
        self._lines: queue.Queue[str | None] = queue.Queue()
        self._process: subprocess.Popen[str] | None = None
        self._reader: threading.Thread | None = None
        self.endpoint = ""

    def _write_config(self) -> None:
        exporters = "\n".join(
            f"  file/{signal}:\n    path: {json.dumps(str(self.files[signal]))}" for signal in SIGNALS
        )
        pipelines = "\n".join(
            f"    {signal}: {{receivers: [otlp], exporters: [file/{signal}]}}" for signal in SIGNALS
        )
        self.config.write_text(
            "receivers:\n  otlp:\n    protocols:\n      http:\n        endpoint: 127.0.0.1:0\n"
            f"exporters:\n{exporters}\n"
            "service:\n  telemetry:\n    metrics:\n      level: none\n    logs:\n      encoding: json\n"
            f"  pipelines:\n{pipelines}\n",
            encoding="utf-8",
        )

    def start(self) -> None:
        self._write_config()
        for path in self.files.values():
            path.write_text("", encoding="utf-8")
        self._process = subprocess.Popen(
            [str(self.binary), "--config", str(self.config)],
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, encoding="utf-8",
        )
        process = self._process

        def pump() -> None:
            assert process.stderr is not None
            with self.stderr_path.open("w", encoding="utf-8") as log:
                for line in process.stderr:
                    log.write(line)
                    log.flush()
                    self._lines.put(line)
            self._lines.put(None)

        self._reader = threading.Thread(target=pump, daemon=True)
        self._reader.start()
        deadline = time.monotonic() + COLLECTOR_START_TIMEOUT_SECONDS
        ready = False
        while not ready:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                self.stop()
                pytest.fail(f"collector did not become ready within {COLLECTOR_START_TIMEOUT_SECONDS}s: "
                            f"{self.stderr_path.read_text(encoding='utf-8')}")
            try:
                line = self._lines.get(timeout=remaining)
            except queue.Empty:
                continue
            if line is None:
                self.stop()
                pytest.fail(f"collector exited during startup: {self.stderr_path.read_text(encoding='utf-8')}")
            if '"Starting HTTP server"' in line:
                self.endpoint = f"http://{json.loads(line)['endpoint']}"
            ready = "Everything is ready" in line
        assert self.endpoint, "collector became ready without reporting its OTLP/HTTP address"

    def stop(self) -> None:
        """Terminate and reap the process on every path."""
        process, self._process = self._process, None
        if process is None:
            return
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=COLLECTOR_STOP_TIMEOUT_SECONDS)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if self._reader is not None:
            self._reader.join(timeout=COLLECTOR_STOP_TIMEOUT_SECONDS)

    def exported(self, signal: str) -> list[dict[str, Any]]:
        """Every complete export request the ``file`` exporter has written for ``signal``."""
        text = self.files[signal].read_text(encoding="utf-8")
        complete = text[: text.rfind("\n") + 1]
        return [json.loads(line) for line in complete.splitlines() if line]

    def wait_for(self, signal: str, ready: Callable[[list[dict[str, Any]]], bool]) -> list[dict[str, Any]]:
        """Poll the exported file until ``ready`` accepts it, under one watchdog deadline."""
        deadline = time.monotonic() + COLLECTOR_EXPORT_TIMEOUT_SECONDS
        while True:
            requests = self.exported(signal)
            if ready(requests):
                return requests
            if time.monotonic() >= deadline:
                raise AssertionError(
                    f"collector {signal} export did not satisfy the wait within "
                    f"{COLLECTOR_EXPORT_TIMEOUT_SECONDS}s; exported so far: {json.dumps(requests)[:4000]}"
                )
            time.sleep(POLL_SECONDS)

    # Flattened views of the exported requests.

    def log_records(self) -> list[dict[str, Any]]:
        return [
            {"resource": attributes(resource["resource"].get("attributes")), "scope": scope["scope"],
             "record": record}
            for request in self.exported("logs") for resource in request.get("resourceLogs", [])
            for scope in resource.get("scopeLogs", []) for record in scope.get("logRecords", [])
        ]

    def spans(self) -> list[dict[str, Any]]:
        return [
            {"resource": attributes(resource["resource"].get("attributes")), "scope": scope["scope"],
             "span": span}
            for request in self.exported("traces") for resource in request.get("resourceSpans", [])
            for scope in resource.get("scopeSpans", []) for span in scope.get("spans", [])
        ]

    def metrics(self) -> list[dict[str, Any]]:
        return [
            {"resource": attributes(resource["resource"].get("attributes")), "scope": scope["scope"],
             "metric": metric}
            for request in self.exported("metrics") for resource in request.get("resourceMetrics", [])
            for scope in resource.get("scopeMetrics", []) for metric in scope.get("metrics", [])
        ]

    def wait_for_service(self, service: str, *, logs: int = 0, spans: int = 0, metrics: int = 0) -> None:
        """Wait until the named service has at least this many records of each signal."""
        if logs:
            self.wait_for("logs", lambda _: sum(r["resource"].get("service.name") == service
                                                for r in self.log_records()) >= logs)
        if spans:
            self.wait_for("traces", lambda _: sum(s["resource"].get("service.name") == service
                                                  for s in self.spans()) >= spans)
        if metrics:
            self.wait_for("metrics", lambda _: sum(m["resource"].get("service.name") == service
                                                   for m in self.metrics()) >= metrics)

    def for_service(self, service: str) -> dict[str, list[dict[str, Any]]]:
        return {
            "logs": [r for r in self.log_records() if r["resource"].get("service.name") == service],
            "spans": [s for s in self.spans() if s["resource"].get("service.name") == service],
            "metrics": [m for m in self.metrics() if m["resource"].get("service.name") == service],
        }


@pytest.fixture(scope="session")
def collector_binary() -> Path:
    """The caller-provided official Collector, verified against the pinned manifest.

    The workflow downloads the release asset recorded in ``collector-release.json``;
    local developers export the extracted binary.  No test downloads anything.
    """
    binary = os.environ.get("TELEMETRY_E2E_COLLECTOR_BINARY")
    if not binary:
        message = "set TELEMETRY_E2E_COLLECTOR_BINARY to the pinned otelcol-contrib binary"
        if os.environ.get("GITHUB_ACTIONS", "").lower() == "true" or os.environ.get("CI"):
            pytest.fail(f"{message}; CI must not skip the Collector qualification", pytrace=False)
        pytest.skip(message)
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    host = os.environ.get("TELEMETRY_E2E_COLLECTOR_PLATFORM", host_platform())
    entry = manifest["platforms"].get(host)
    if entry is None:
        pytest.fail(f"the pinned Collector manifest has no entry for {host}", pytrace=False)
    path = Path(binary)
    actual = sha256_file(path)
    if actual != entry["binary_sha256"]:
        pytest.fail(f"{path} sha256 {actual} does not match the pinned {manifest['version']} "
                    f"{host} binary {entry['binary_sha256']}", pytrace=False)
    version = run_process([str(path), "--version"], timeout=PROCESS_TIMEOUT_SECONDS,
                          capture_output=True, text=True, check=True).stdout
    assert manifest["version"] in version, version
    return path


@pytest.fixture
def collector(collector_binary: Path, tmp_path: Path) -> Iterator[OfficialCollector]:
    work = tmp_path / "collector"
    work.mkdir()
    instance = OfficialCollector(collector_binary, work)
    try:
        instance.start()
        yield instance
    finally:
        instance.stop()


# ---------------------------------------------------------------------------
# Unavailable endpoint


class RefusingListener:
    """A bound, listening socket that closes every accepted connection at once.

    The port stays bound for the listener's whole life, so no other process can
    take it and the refusal is deterministic: a client always connects and is
    always cut off before any response.
    """

    def __init__(self) -> None:
        self._socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._socket.bind(("127.0.0.1", 0))
        self._socket.listen(16)
        self._socket.settimeout(0.1)
        self._stop = threading.Event()
        self.accepted = 0
        self.endpoint = f"http://127.0.0.1:{self._socket.getsockname()[1]}"
        self._thread = threading.Thread(target=self._serve, daemon=True)

    def _serve(self) -> None:
        while not self._stop.is_set():
            try:
                connection, _ = self._socket.accept()
            except TimeoutError:
                continue
            except OSError:
                return
            self.accepted += 1
            connection.close()

    def __enter__(self) -> RefusingListener:
        self._thread.start()
        return self

    def __exit__(self, *_: object) -> None:
        self._stop.set()
        self._thread.join(timeout=5)
        self._socket.close()


@pytest.fixture
def refusing_endpoint() -> Iterator[RefusingListener]:
    with RefusingListener() as listener:
        yield listener


# ---------------------------------------------------------------------------
# Built artifacts


def _cargo_executable(package: str, binary: str) -> Path:
    built = run_process(
        ["cargo", "build", "--locked", "-p", package, "--message-format=json-render-diagnostics"],
        cwd=ROOT, timeout=BUILD_TIMEOUT_SECONDS, capture_output=True, text=True, check=False,
    )
    assert built.returncode == 0, built.stdout + built.stderr
    for line in built.stdout.splitlines():
        message = json.loads(line)
        if (message.get("reason") == "compiler-artifact" and message["target"]["name"] == binary
                and message.get("executable")):
            return Path(message["executable"])
    raise AssertionError(f"cargo built no executable named {binary}")


@pytest.fixture(scope="session")
def native_example() -> Path:
    """The native example binary (sync client, Tokio providers, logger composition).

    ``TELEMETRY_E2E_NATIVE_EXAMPLE`` selects a prebuilt binary.
    """
    prebuilt = os.environ.get("TELEMETRY_E2E_NATIVE_EXAMPLE")
    if prebuilt:
        return Path(prebuilt)
    return _cargo_executable("otlp-native-example", "otlp-native-example")


@pytest.fixture(scope="session")
def installed_artifacts(tmp_path_factory: pytest.TempPathFactory) -> dict[str, Path]:
    """The installed wheel's interpreter and the installed ``sc-otel`` command.

    ``TELEMETRY_E2E_PYTHON`` and ``TELEMETRY_E2E_CLI`` select prebuilt installed
    artifacts; otherwise the release wheel is built and installed into a fresh
    virtual environment and the command is installed with ``cargo install``.
    """
    python_override = os.environ.get("TELEMETRY_E2E_PYTHON")
    cli_override = os.environ.get("TELEMETRY_E2E_CLI")
    if python_override and cli_override:
        return {"python": Path(python_override), "cli": Path(cli_override)}
    root = tmp_path_factory.mktemp("installed-telemetry")
    windows = os.name == "nt"
    python = Path(python_override) if python_override else None
    if python is None:
        venv = root / "venv"
        run_process([sys.executable, "-m", "venv", str(venv)], timeout=INSTALL_TIMEOUT_SECONDS, check=True)
        python = venv / ("Scripts/python.exe" if windows else "bin/python")
        run_process([str(python), "-m", "pip", "install", "--upgrade", "pip==25.3", "maturin==1.10.2"],
                    timeout=INSTALL_TIMEOUT_SECONDS, check=True)
        wheel_dir = root / "wheel"
        run_process(
            [str(python), "-m", "maturin", "build", "--release", "--manifest-path",
             str(ROOT / "bindings/python/sc-observability-py/Cargo.toml"),
             "--features", "otlp-telemetry", "--out", str(wheel_dir)],
            cwd=ROOT, timeout=BUILD_TIMEOUT_SECONDS, check=True,
        )
        wheels = sorted(wheel_dir.glob("*.whl"))
        assert len(wheels) == 1, f"expected one wheel, found {wheels}"
        run_process([str(python), "-m", "pip", "install", str(wheels[0])],
                    timeout=INSTALL_TIMEOUT_SECONDS, check=True)
    if cli_override:
        cli = Path(cli_override)
    else:
        cli_root = root / "cli"
        run_process(
            ["cargo", "install", "--locked", "--path", "crates/sc-otel-cli", "--root", str(cli_root), "--force"],
            cwd=ROOT, timeout=BUILD_TIMEOUT_SECONDS, check=True,
        )
        cli = cli_root / "bin" / ("sc-otel.exe" if windows else "sc-otel")
    return {"python": python, "cli": cli}


def run_cli(artifacts: dict[str, Path], *args: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    return run_process([str(artifacts["cli"]), *args], cwd=cwd, text=True, capture_output=True,
                       check=False, timeout=PROCESS_TIMEOUT_SECONDS)


# The installed-wheel driver: a plain file (never ``python -c``), request on stdin.
PYTHON_DRIVER = """\
import json
import sys
from sc_observability import Err, Ok, Telemetry

request = json.load(sys.stdin)
telemetry = Telemetry(request["endpoint"], service_name=request["service"], timeout_s=request["timeout_s"])
result = getattr(telemetry, request["operation"])(*request["args"], **request["kwargs"])
if isinstance(result, Ok):
    print(json.dumps({"ok": True, "value": result.value}))
else:
    assert isinstance(result, Err), result
    print(json.dumps({"ok": False, "kind": result.error.kind, "code": result.error.code}))
"""


def run_python(artifacts: dict[str, Path], cwd: Path, request: dict[str, Any]) -> dict[str, Any]:
    """Call the installed ``Telemetry`` once and return its tagged outcome."""
    driver = cwd / "installed_frontend.py"
    driver.write_text(PYTHON_DRIVER, encoding="utf-8")
    completed = run_process(
        [str(artifacts["python"]), str(driver)], cwd=cwd, input=json.dumps(request),
        text=True, capture_output=True, check=False, timeout=PROCESS_TIMEOUT_SECONDS,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    return json.loads(completed.stdout)


# ---------------------------------------------------------------------------
# Pinned desktop viewer


def _viewer_host_platform() -> str:
    """Match the downloader's host selection for explicit local viewer opt-in."""
    machine = platform.machine().lower()
    return f"{platform.system().lower()}_{VIEWER_MACHINES.get(machine, machine)}"


def _viewer_start_command(
    binary: str, version: str, binary_sha256: str, state: Path, http: int, grpc: int, ui: int,
    *, reuse_state: bool,
) -> list[str]:
    command = [
        sys.executable, str(VIEWER_HARNESS),
        "start", "--binary", binary, "--version", version, "--binary-sha256", binary_sha256,
        "--state-dir", str(state), "--http", str(http), "--grpc", str(grpc), "--ui", str(ui),
    ]
    if reuse_state:
        command.append("--reuse-state")
    return command


class PinnedViewer(dict[str, str]):
    """A hash-pinned viewer that tests may stop and restart.

    The harness asks the viewer to bind ephemeral ports and reports the bound
    ports in its ready JSON, so no port is chosen and released before use.
    Every start, including a restart, publishes the ports it was given.
    """

    def __init__(self, binary: str, version: str, binary_sha256: str, state: Path) -> None:
        self.binary = binary
        self.version = version
        self.binary_sha256 = binary_sha256
        self.state = state
        self.http = self.grpc = self.ui = 0
        super().__init__()

    def start(self, *, reuse_state: bool = False) -> None:
        command = _viewer_start_command(
            self.binary, self.version, self.binary_sha256, self.state, 0, 0, 0,
            reuse_state=reuse_state,
        )
        started = run_process(
            command,
            check=False, text=True, capture_output=True, timeout=VIEWER_TIMEOUT_SECONDS,
        )
        assert started.returncode == 0, started.stdout + started.stderr
        ready = json.loads(started.stdout.strip().splitlines()[-1])
        self.http, self.grpc, self.ui = (int(ready[name]) for name in ("http", "grpc", "ui"))
        self.update(otlp=f"http://127.0.0.1:{self.http}", rpc=f"http://127.0.0.1:{self.ui}/rpc")

    def stop(self) -> None:
        stopped = run_process(
            [sys.executable, str(VIEWER_HARNESS), "stop", "--state-dir", str(self.state)],
            check=False, text=True, capture_output=True, timeout=VIEWER_TIMEOUT_SECONDS,
        )
        assert stopped.returncode == 0, stopped.stdout + stopped.stderr

    def restart(self) -> None:
        self.stop()
        self.start(reuse_state=True)


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
        if os.environ.get("GITHUB_ACTIONS", "").lower() == "true" or os.environ.get("CI"):
            pytest.fail(f"{message}; CI must not skip viewer readback", pytrace=False)
        pytest.skip(message)
    manifest = json.loads((ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/release.json").read_text())
    host = os.environ.get("TELEMETRY_E2E_VIEWER_PLATFORM", _viewer_host_platform())
    entry = manifest["platforms"].get(host)
    if entry is None:
        pytest.fail(f"pinned viewer has no manifest entry for {host}", pytrace=False)
    receipt_sha256 = os.environ.get("TELEMETRY_E2E_VIEWER_BINARY_SHA256")
    if receipt_sha256 and receipt_sha256 != entry["binary_sha256"]:
        pytest.fail("pinned viewer downloader receipt does not match host manifest", pytrace=False)
    viewer = PinnedViewer(binary, manifest["version"], entry["binary_sha256"], tmp_path / "viewer-state")
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
