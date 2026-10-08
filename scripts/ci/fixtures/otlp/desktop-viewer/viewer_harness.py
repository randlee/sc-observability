#!/usr/bin/env python3
"""Own an isolated CI viewer process or probe the user's installed service.

The desktop service is never stopped or cleaned up by this tool. Only a
process started with ``start`` and recorded in the selected state directory is
owned by the harness.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path
from typing import Any

DEFAULT_HTTP = 4318
DEFAULT_GRPC = 4317
DEFAULT_UI = 8000
READY_SECONDS = 30
QUERY_SECONDS = 120
SERVICE = "sc-observability-d9"
RESTART_MARKER = "viewer.stopped.json"


class HarnessError(RuntimeError):
    pass


def _is_windows() -> bool:
    """Small platform seam for fixture-only Windows branch coverage."""
    return sys.platform == "win32"


def _request(url: str, payload: bytes | None = None, *, timeout: float = 2,
             content_type: str = "application/json") -> tuple[int, bytes]:
    headers = {"Content-Type": content_type} if payload is not None else {}
    req = urllib.request.Request(url, data=payload, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()


def rpc(base: str, method: str, params: list[Any], timeout: float = 10) -> Any:
    request_id = str(uuid.uuid4())
    body = json.dumps({"jsonrpc": "2.0", "id": request_id,
                       "method": method, "params": params}).encode()
    status, raw = _request(base.rstrip("/") + "/rpc", body, timeout=timeout)
    if status != 200:
        raise HarnessError(f"RPC {method} returned HTTP {status}: {raw[:500]!r}")
    response = json.loads(raw)
    if response.get("id") != request_id or "error" in response:
        raise HarnessError(f"RPC {method} failed: {response.get('error', response)!r}")
    return response.get("result")


def _port_available(host: str, port: int, *, reuse_address: bool = False) -> bool:
    family = socket.AF_INET6 if ":" in host else socket.AF_INET
    with socket.socket(family, socket.SOCK_STREAM) as sock:
        # Windows SO_REUSEADDR permits binding a port with a live listener.
        if reuse_address and not _is_windows():
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            sock.bind((host, port))
        except OSError:
            return False
    return True


def _tcp_listener(host: str, port: int, timeout: float = 2) -> bool:
    family = socket.AF_INET6 if ":" in host else socket.AF_INET
    with socket.socket(family, socket.SOCK_STREAM) as sock:
        sock.settimeout(timeout)
        try:
            sock.connect((host, port))
        except OSError:
            return False
    return True


_WIN = None


def _win32() -> Any:
    """Load the small Win32 surface needed for owned-process validation."""
    global _WIN
    if _WIN is not None:
        return _WIN
    import ctypes
    from ctypes import wintypes

    class UnicodeString(ctypes.Structure):
        _fields_ = [("Length", ctypes.c_ushort), ("MaximumLength", ctypes.c_ushort),
                    ("Buffer", ctypes.c_void_p)]

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    ntdll = ctypes.WinDLL("ntdll")
    shell32 = ctypes.WinDLL("shell32", use_last_error=True)
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel32.WaitForSingleObject.restype = wintypes.DWORD
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL
    kernel32.LocalFree.argtypes = [ctypes.c_void_p]
    kernel32.LocalFree.restype = ctypes.c_void_p
    ntdll.NtQueryInformationProcess.argtypes = [
        wintypes.HANDLE, ctypes.c_ulong, ctypes.c_void_p, ctypes.c_ulong,
        ctypes.POINTER(ctypes.c_ulong)]
    ntdll.NtQueryInformationProcess.restype = ctypes.c_long
    shell32.CommandLineToArgvW.argtypes = [wintypes.LPCWSTR, ctypes.POINTER(ctypes.c_int)]
    shell32.CommandLineToArgvW.restype = ctypes.POINTER(wintypes.LPWSTR)
    _WIN = type("Win32", (), {"ctypes": ctypes, "kernel32": kernel32, "ntdll": ntdll,
                               "shell32": shell32, "UnicodeString": UnicodeString})
    return _WIN


_SYNCHRONIZE = 0x00100000
_PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
_WAIT_TIMEOUT = 0x102
_PROCESS_COMMAND_LINE_INFORMATION = 60


def _open_process_win(pid: int) -> tuple[Any, int]:
    win = _win32()
    handle = win.kernel32.OpenProcess(
        _SYNCHRONIZE | _PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    return handle, win.ctypes.get_last_error()


def _terminate(pid: int, *, force: bool) -> None:
    """Stop an owned process; Windows maps both stages to TerminateProcess."""
    if _is_windows():
        os.kill(pid, signal.SIGTERM)
    else:
        os.kill(pid, signal.SIGKILL if force else signal.SIGTERM)


def _command_args_win(pid: int) -> list[str] | None:
    """Return a live process command line using the Windows process API."""
    if pid <= 0:
        return None
    win = _win32()
    ctypes = win.ctypes
    handle, _ = _open_process_win(pid)
    if not handle:
        return None
    try:
        if win.kernel32.WaitForSingleObject(handle, 0) != _WAIT_TIMEOUT:
            return None
        needed = ctypes.c_ulong(0)
        win.ntdll.NtQueryInformationProcess(
            handle, _PROCESS_COMMAND_LINE_INFORMATION, None, 0, ctypes.byref(needed))
        if needed.value < ctypes.sizeof(win.UnicodeString):
            return None
        buffer = ctypes.create_string_buffer(needed.value)
        status = win.ntdll.NtQueryInformationProcess(
            handle, _PROCESS_COMMAND_LINE_INFORMATION, buffer, needed.value, ctypes.byref(needed))
        if status < 0:
            return None
        text = win.UnicodeString.from_buffer(buffer)
        if not text.Length or not text.Buffer:
            return None
        command_line = ctypes.wstring_at(text.Buffer, text.Length // 2)
    finally:
        win.kernel32.CloseHandle(handle)
    count = ctypes.c_int(0)
    argv = win.shell32.CommandLineToArgvW(command_line, ctypes.byref(count))
    if not argv:
        return None
    try:
        return [argv[index] for index in range(count.value)] or None
    finally:
        win.kernel32.LocalFree(ctypes.cast(argv, ctypes.c_void_p))


def _command_args(pid: int) -> list[str] | None:
    """Return command arguments only for a live, non-zombie process.

    This is the sole liveness authority. POSIX zombies intentionally return
    None, so they are treated as stopped rather than signalable.
    """
    if _is_windows():
        return _command_args_win(pid)
    if sys.platform.startswith("linux"):
        try:
            stat = Path(f"/proc/{pid}/stat").read_text().split()
            if len(stat) > 2 and stat[2] == "Z":
                return None
            raw = Path(f"/proc/{pid}/cmdline").read_bytes()
            return [part.decode() for part in raw.split(b"\0") if part] or None
        except OSError:
            return None
    try:
        result = subprocess.run(["ps", "-p", str(pid), "-o", "stat=,command="],
                                check=False, capture_output=True, text=True)
    except OSError:
        return None
    fields = result.stdout.strip().split(None, 1)
    if not fields or fields[0].startswith("Z"):
        return None
    try:
        return shlex.split(fields[1]) if len(fields) > 1 else None
    except ValueError:
        return None


def _command_line(pid: int) -> str | None:
    """Compatibility helper for status display and older local callers."""
    args = _command_args(pid)
    return " ".join(args) if args else None


def _owned(state: Path) -> tuple[int, dict[str, Any]]:
    pid_file = state / "viewer.pid"
    meta_file = state / "viewer.json"
    if not pid_file.is_file() or not meta_file.is_file():
        raise HarnessError(f"no harness-owned instance recorded in {state}")
    pid = int(pid_file.read_text().strip())
    metadata = json.loads(meta_file.read_text())
    try:
        if int(metadata["pid"]) != pid:
            raise HarnessError(f"refusing to signal PID {pid}: pid file and metadata disagree")
        binary = str(metadata["binary"])
        database = str(metadata["database"])
    except (KeyError, TypeError, ValueError) as error:
        raise HarnessError(f"refusing to signal PID {pid}: incomplete process metadata") from error
    args = _command_args(pid)
    db_index = args.index("--db") + 1 if args and "--db" in args else -1
    executable = args[0] if args else ""
    try:
        executable_matches = (os.path.normcase(str(Path(executable).resolve()))
                              == os.path.normcase(str(Path(binary).resolve())))
    except (OSError, RuntimeError):
        executable_matches = False
    if (not args or not executable_matches or db_index < 1 or db_index >= len(args)
            or args[db_index] != database):
        raise HarnessError(
            f"refusing to signal PID {pid}: process identity does not match recorded binary and database")
    return pid, metadata


def _owned_database(state: Path, metadata: dict[str, Any]) -> Path:
    """Return the only database path this harness may delete."""
    database = state / "viewer.duckdb"
    try:
        recorded = Path(metadata["database"]).expanduser().resolve()
    except (KeyError, OSError, TypeError, ValueError) as error:
        raise HarnessError("refusing cleanup: viewer metadata has no valid database path") from error
    if database.is_symlink() or recorded != database:
        raise HarnessError(
            f"refusing cleanup: recorded database {recorded} does not resolve to owned path {database}")
    return database


def _restart_metadata(state: Path, binary: Path, binary_sha256: str,
                      version: str | None) -> tuple[Path, dict[str, Any]]:
    """Validate the explicit restart-only state retained by one owned stop."""
    marker = state / RESTART_MARKER
    try:
        metadata = json.loads(marker.read_text())
        recorded_binary = Path(metadata["binary"]).expanduser().resolve()
        recorded_sha256 = str(metadata["sha256"])
        recorded_version = metadata["version"]
        database = Path(metadata["database"]).expanduser().resolve()
    except (OSError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        raise HarnessError(f"state directory is not a stopped owned viewer: {state}") from error
    expected_database = (state / "viewer.duckdb").resolve()
    if (recorded_binary != binary or recorded_sha256.lower() != binary_sha256.lower()
            or recorded_version != version or database != expected_database or not database.is_file()):
        raise HarnessError("restart state does not match the requested binary, version, and database")
    permitted = {
        RESTART_MARKER,
        "viewer.duckdb",
        "viewer.duckdb.wal",
        "viewer.log",
    }
    unexpected = [entry.name for entry in state.iterdir() if entry.name not in permitted]
    if unexpected:
        raise HarnessError(f"restart state contains unexpected files: {unexpected}")
    return marker, metadata


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def start(args: argparse.Namespace) -> None:
    binary = Path(args.binary).expanduser().resolve(strict=True)
    state = Path(args.state_dir).expanduser().resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise HarnessError(f"viewer binary is not executable: {binary}")
    actual_hash = _sha256(binary)
    if actual_hash.lower() != args.binary_sha256.lower():
        raise HarnessError(f"viewer binary SHA-256 mismatch: expected {args.binary_sha256}, got {actual_hash}")
    if args.version:
        version = subprocess.run([str(binary), "--version"], check=False,
                                 capture_output=True, text=True, timeout=10)
        if version.returncode != 0 or args.version not in (version.stdout + version.stderr):
            raise HarnessError(f"viewer version mismatch; expected {args.version!r}: "
                               f"{version.stdout}{version.stderr}")
    restart = None
    reuse_state = getattr(args, "reuse_state", False)
    if state.exists() and (not state.is_dir() or any(state.iterdir())):
        if not reuse_state:
            raise HarnessError(f"state directory is not empty: {state}")
        restart = _restart_metadata(state, binary, actual_hash, args.version)
    elif reuse_state:
        raise HarnessError(f"restart state does not exist: {state}")
    ports = ((args.host, args.http), (args.host, args.grpc), (args.host, args.ui))
    if len({port for _, port in ports}) != 3 or any(
            not _port_available(h, p, reuse_address=restart is not None) for h, p in ports):
        raise HarnessError("one or more selected ports are occupied; choose explicit free ports; "
                           "the harness will not stop the existing listener")
    state_created = not state.exists()
    state.mkdir(parents=True, exist_ok=True)
    database = (state / "viewer.duckdb").resolve()
    command = [str(binary), "--host", args.host, "--http", str(args.http),
               "--grpc", str(args.grpc), "--browser-port", str(args.ui),
               "--open-browser=false", "--db", str(database), "--db-max-size", "2GB"]
    log_path = state / "viewer.log"
    log = None
    try:
        log = log_path.open("ab")
        detach: dict[str, Any] = ({"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
                                  if _is_windows() else {"start_new_session": True})
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                   stdout=log, stderr=subprocess.STDOUT, **detach)
        if restart is not None:
            restart[0].unlink()
    except BaseException:
        if log is not None:
            log.close()
            log = None
        log_path.unlink(missing_ok=True)
        if state_created:
            try:
                state.rmdir()
            except OSError:
                pass
        raise
    try:
        metadata = {"pid": process.pid, "binary": str(binary), "sha256": actual_hash,
                    "version": args.version, "database": str(database),
                    "host": args.host, "http": args.http, "grpc": args.grpc,
                    "ui": args.ui, "started_at": time.time()}
        (state / "viewer.json").write_text(json.dumps(metadata, indent=2) + "\n")
        (state / "viewer.pid").write_text(f"{process.pid}\n")
        deadline = time.monotonic() + READY_SECONDS
        ui_url = f"http://{args.host}:{args.ui}/"
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise HarnessError(f"viewer exited during startup; inspect {state / 'viewer.log'}")
            try:
                _request(ui_url, timeout=1)
                print(json.dumps({"status": "ready", **metadata, "ui_url": ui_url}))
                return
            except (OSError, TimeoutError, urllib.error.URLError):
                time.sleep(0.25)
        raise HarnessError(f"viewer did not become ready within {READY_SECONDS}s; inspect {state / 'viewer.log'}")
    except BaseException:
        try:
            if process.poll() is None:
                try:
                    process.terminate()
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    try:
                        process.kill()
                    except ProcessLookupError:
                        pass
                    process.wait(timeout=5)
        finally:
            if log is not None:
                log.close()
                log = None
            (state / "viewer.pid").unlink(missing_ok=True)
            (state / "viewer.json").unlink(missing_ok=True)
            if restart is not None:
                restart[0].write_text(json.dumps(restart[1], indent=2) + "\n")
            else:
                (state / "viewer.log").unlink(missing_ok=True)
                database.unlink(missing_ok=True)
                Path(str(database) + ".wal").unlink(missing_ok=True)
            if state_created:
                try:
                    state.rmdir()
                except OSError:
                    pass
        raise
    finally:
        if log is not None:
            log.close()


def status(args: argparse.Namespace) -> None:
    pid, metadata = _owned(Path(args.state_dir).expanduser().resolve())
    if _command_args(pid) is None:
        raise HarnessError(f"owned viewer PID {pid} is not running")
    _request(f"http://{metadata['host']}:{metadata['ui']}/", timeout=2)
    print(json.dumps({"status": "ready", **metadata,
                      "ui_url": f"http://{metadata['host']}:{metadata['ui']}/"}))


def stop(args: argparse.Namespace) -> None:
    state = Path(args.state_dir).expanduser().resolve()
    pid, metadata = _owned(state)
    database = _owned_database(state, metadata) if args.remove_state else None
    try:
        _terminate(pid, force=False)
    except ProcessLookupError:
        pass
    except OSError:
        if not _is_windows() or _command_args(pid) is not None:
            raise
    else:
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            if _command_args(pid) is None:
                break
            time.sleep(0.1)
        else:
            if _command_args(pid) is not None:
                # Revalidate ownership immediately before escalation in case the
                # PID exited and was reused while the graceful deadline elapsed.
                try:
                    checked_pid, _ = _owned(state)
                except HarnessError:
                    if _command_args(pid) is not None:
                        raise
                    checked_pid = pid
                if checked_pid != pid:
                    raise HarnessError(f"refusing to force-stop changed viewer PID {pid}")
                try:
                    _terminate(pid, force=True)
                except ProcessLookupError:
                    pass
                except OSError:
                    if not _is_windows() or _command_args(pid) is not None:
                        raise
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if _command_args(pid) is None:
                        if not _is_windows():
                            try:
                                os.waitpid(pid, os.WNOHANG)
                            except ChildProcessError:
                                pass
                        break
                    time.sleep(0.05)
                else:
                    raise HarnessError(f"owned viewer PID {pid} did not stop after SIGKILL")
    (state / "viewer.pid").unlink(missing_ok=True)
    (state / "viewer.json").unlink(missing_ok=True)
    if args.remove_state:
        # Remove only this tool's database and its log, never a configured desktop DB.
        assert database is not None
        database.unlink(missing_ok=True)
        Path(str(database) + ".wal").unlink(missing_ok=True)
        (state / "viewer.log").unlink(missing_ok=True)
        (state / RESTART_MARKER).unlink(missing_ok=True)
        state.rmdir()
    else:
        (state / RESTART_MARKER).write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps({"status": "stopped", "pid": pid, "state_dir": str(state),
                      "removed_owned_state": args.remove_state}))


def _attr(name: str, value: str) -> dict[str, Any]:
    return {"key": name, "value": {"stringValue": value}}


def _find_value(value: Any, needle: Any) -> bool:
    if value == needle:
        return True
    if isinstance(value, dict):
        return any(_find_value(k, needle) or _find_value(v, needle) for k, v in value.items())
    if isinstance(value, list):
        return any(_find_value(item, needle) for item in value)
    return str(value) == str(needle)


def _wait_rpc(base: str, method: str, params: list[Any], needle: str,
              deadline: float) -> Any:
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            result = rpc(base, method, params, timeout=5)
            if _find_value(result, needle):
                return result
        except (OSError, HarnessError, ValueError) as error:
            last_error = error
        time.sleep(0.5)
    raise HarnessError(f"{method} did not return expected value {needle!r} within "
                       f"{QUERY_SECONDS}s; last error: {last_error}")


def probe(args: argparse.Namespace) -> None:
    """Send bounded OTLP/HTTP JSON data and prove the records are queryable."""
    base = args.base.rstrip("/")
    otlp_http = args.otlp_http.rstrip("/")
    run_id = args.run_id or str(uuid.uuid4())
    if len(run_id) > 64 or not all(c.isalnum() or c in "-_" for c in run_id):
        raise HarnessError("run id must be at most 64 ASCII letters, digits, '-' or '_'")
    start_ns = time.time_ns()
    end_ns = start_ns + 60_000_000_000
    trace_id = uuid.uuid4().hex
    span_id = uuid.uuid4().hex[:16]
    metric_name = "sc_observability.d9.probe"
    resource_attrs = [_attr("service.name", SERVICE), _attr("test.backend", "setup-probe")]
    logs = {"resourceLogs": [{"resource": {"attributes": resource_attrs},
             "scopeLogs": [{"scope": {"name": "sc-observability-d9"},
             "logRecords": [{"timeUnixNano": str(start_ns), "severityNumber": 9,
             "severityText": "INFO", "body": {"stringValue": f"d9-log-{run_id}"},
             "attributes": [_attr("test.signal", "logs"), _attr("test.run_id", run_id)], "traceId": trace_id,
             "spanId": span_id}]}]}]}
    traces = {"resourceSpans": [{"resource": {"attributes": resource_attrs},
               "scopeSpans": [{"scope": {"name": "sc-observability-d9"}, "spans": [{
               "traceId": trace_id, "spanId": span_id, "name": f"d9-span-{run_id}",
               "kind": 1, "startTimeUnixNano": str(start_ns),
               "endTimeUnixNano": str(start_ns + 1_000_000), "attributes": [
               _attr("test.signal", "traces"), _attr("test.run_id", run_id)]}]}]}]}
    metrics = {"resourceMetrics": [{"resource": {"attributes": resource_attrs},
                "scopeMetrics": [{"scope": {"name": "sc-observability-d9"},
                "metrics": [{"name": metric_name, "unit": "1", "gauge": {
                "dataPoints": [{"timeUnixNano": str(start_ns), "asInt": "42",
                "attributes": [_attr("test.case", "setup-probe")] }]}}]}]}]}
    expected = {"logs": f"d9-log-{run_id}", "traces": f"d9-span-{run_id}",
                "metrics": metric_name}
    results: dict[str, Any] = {}
    deadline = time.monotonic() + QUERY_SECONDS
    protobuf_status: dict[str, int] = {}
    for signal_name in ("logs", "traces", "metrics"):
        status_code, raw = _request(f"{otlp_http}/v1/{signal_name}", b"",
                                    timeout=10, content_type="application/x-protobuf")
        if status_code not in (200, 202):
            raise HarnessError(f"OTLP/HTTP protobuf {signal_name} returned {status_code}: {raw[:500]!r}")
        protobuf_status[signal_name] = status_code
    grpc_host, grpc_port = args.grpc_target.rsplit(":", 1)
    if not _tcp_listener(grpc_host.strip("[]"), int(grpc_port)):
        raise HarnessError(f"OTLP gRPC listener is unavailable at {args.grpc_target}")
    for signal_name, payload in (("logs", logs), ("traces", traces), ("metrics", metrics)):
        status_code, raw = _request(f"{otlp_http}/v1/{signal_name}",
                                    json.dumps(payload).encode(), timeout=10)
        if status_code not in (200, 202):
            raise HarnessError(f"OTLP/HTTP JSON {signal_name} returned {status_code}: {raw[:500]!r}")
    # Restrict retrieval to this run's 61-second window, then compare exact
    # unique log/span content. The metric series stays bounded across runs.
    low, high = str(start_ns - 1_000_000_000), str(end_ns)
    log_query = {"id": "d9-run-id", "type": "condition", "query": {
        "field": {"name": "test.run_id", "searchScope": "attribute",
                  "attributeScope": "log"},
        "fieldOperator": "=", "value": run_id}}
    logs_result = _wait_rpc(base, "searchLogs", [low, high, log_query], expected["logs"], deadline)
    results["logs"] = logs_result
    candidates = logs_result if isinstance(logs_result, list) else []
    log_id = next((row.get("id") for row in candidates if isinstance(row, dict)
                   and _find_value(row, expected["logs"])), None)
    if not log_id:
        raise HarnessError("searchLogs returned the exact body preview but no log row id")
    results["log_detail"] = rpc(base, "getLog", [log_id])
    if not _find_value(results["log_detail"], expected["logs"]):
        raise HarnessError("getLog did not return the exact synthetic log body")
    spans_result = _wait_rpc(base, "searchSpans", [trace_id], expected["traces"], deadline)
    results["traces"] = spans_result
    if not _find_value(results["log_detail"], trace_id):
        raise HarnessError("getLog did not return the trace ID used by searchSpans")
    metric_query = {"id": "d9-metric-name", "type": "condition", "query": {
        "field": {"name": "name", "searchScope": "field"},
        "fieldOperator": "=", "value": metric_name}}
    metrics_result = _wait_rpc(base, "searchMetricSummaries", [low, high, metric_query],
                               expected["metrics"], deadline)
    results["metric_summaries"] = metrics_result
    summaries = metrics_result if isinstance(metrics_result, list) else []
    stream_id = next((row.get("id") or row.get("streamID") or row.get("streamId")
                      for row in summaries if isinstance(row, dict)
                      and _find_value(row, expected["metrics"])), None)
    if not stream_id:
        raise HarnessError("metric summary matched the probe but returned no stream identifier")
    results["metric_detail"] = rpc(base, "getMetric", [stream_id, low, high])
    if not _find_value(results["metric_detail"], "42") and not _find_value(results["metric_detail"], 42):
        raise HarnessError("getMetric did not return the synthetic gauge value 42")
    print(json.dumps({"status": "pass", "protocol": "otlp-http-json",
                      "capabilities": {"otlp_http_json": "three-signal-record-probe-passed",
                                       "otlp_http_protobuf": protobuf_status,
                                       "otlp_grpc": "listener-reachable; production RPC probe deferred to exporter qualification"},
                      "service": SERVICE, "run_id": run_id, "trace_id": trace_id,
                      "expected": expected, "results": results}, indent=2))


def run_ci(args: argparse.Namespace) -> None:
    """Run the isolated lifecycle and guarantee cleanup after probe failure."""
    state = Path(args.state_dir).expanduser().resolve()
    launch = argparse.Namespace(binary=args.binary, binary_sha256=args.binary_sha256,
                                version=args.version, state_dir=str(state), host=args.host,
                                http=args.http, grpc=args.grpc, ui=args.ui)
    started = False
    try:
        start(launch)
        started = True
        status(argparse.Namespace(state_dir=str(state)))
        probe(argparse.Namespace(
            base=f"http://{args.host}:{args.ui}",
            otlp_http=f"http://{args.host}:{args.http}",
            grpc_target=f"{args.host}:{args.grpc}", run_id=args.run_id))
    finally:
        if started and (state / "viewer.pid").is_file():
            stop(argparse.Namespace(state_dir=str(state), timeout=10, remove_state=True))


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    launch = commands.add_parser("start", help="start and own an isolated CI instance")
    launch.add_argument("--binary", required=True)
    launch.add_argument("--binary-sha256", required=True)
    launch.add_argument("--version")
    launch.add_argument("--state-dir", default=str(Path(tempfile.gettempdir()) /
                                                     "sc-observability-d9-viewer"))
    launch.add_argument("--host", default="127.0.0.1")
    launch.add_argument("--http", type=int, default=DEFAULT_HTTP)
    launch.add_argument("--grpc", type=int, default=DEFAULT_GRPC)
    launch.add_argument("--ui", type=int, default=DEFAULT_UI)
    launch.add_argument("--reuse-state", action="store_true",
                        help="restart only a matching instance previously stopped by this harness")
    launch.set_defaults(func=start)
    check = commands.add_parser("status", help="show only this harness-owned instance")
    check.add_argument("--state-dir", required=True)
    check.set_defaults(func=status)
    shutdown = commands.add_parser("stop", help="stop only this harness-owned instance")
    shutdown.add_argument("--state-dir", required=True)
    shutdown.add_argument("--timeout", type=float, default=10)
    shutdown.add_argument("--remove-state", action="store_true")
    shutdown.set_defaults(func=stop)
    verify = commands.add_parser("probe", help="send and query synthetic three-signal data")
    verify.add_argument("--base", default="http://127.0.0.1:8000")
    verify.add_argument("--otlp-http", default="http://127.0.0.1:4318")
    verify.add_argument("--grpc-target", default="127.0.0.1:4317")
    verify.add_argument("--run-id")
    verify.set_defaults(func=probe)
    ci = commands.add_parser("ci", help="run an isolated probe and always clean up its instance")
    ci.add_argument("--binary", required=True)
    ci.add_argument("--binary-sha256", required=True)
    ci.add_argument("--version", required=True)
    ci.add_argument("--state-dir", required=True)
    ci.add_argument("--host", default="127.0.0.1")
    ci.add_argument("--http", type=int, default=DEFAULT_HTTP)
    ci.add_argument("--grpc", type=int, default=DEFAULT_GRPC)
    ci.add_argument("--ui", type=int, default=DEFAULT_UI)
    ci.add_argument("--run-id")
    ci.set_defaults(func=run_ci)
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        args.func(args)
    except (HarnessError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"viewer harness: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
