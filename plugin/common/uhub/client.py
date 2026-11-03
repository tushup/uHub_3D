"""Localhost bridge client and job validation, shared by all plugins.

Stdlib only. Plugins never execute code from the socket: a job is
only ever a request to import files that live inside libraryRoot.
"""

from __future__ import annotations

import json
import os
import queue
import socket
import threading


class JobError(Exception):
    """A job failed validation or the import could not run."""


def validate_job(job: dict) -> None:
    """Reject jobs whose files escape the library or do not exist.

    Only files the asset actually references are checked; unknown
    keys (view, thumbnail, future keys) are ignored and missing
    optional keys are treated as absent.
    """
    library_root = os.path.realpath(job["libraryRoot"])
    asset_dir = os.path.realpath(job["assetDir"])
    if not asset_dir.startswith(library_root + os.sep):
        raise JobError(f"Asset folder is outside the library: {asset_dir}")
    asset = job.get("asset") or {}

    def check_file(name: str | None) -> None:
        if not name:
            return
        path = os.path.realpath(os.path.join(asset_dir, name))
        if not path.startswith(asset_dir + os.sep):
            raise JobError(f"File is outside the asset folder: {name}")
        if not os.path.isfile(path):
            raise JobError(f"Missing file: {name}")

    model = asset.get("model") or {}
    check_file(model.get("file"))
    hdri = asset.get("hdri") or {}
    check_file(hdri.get("file"))
    for material in asset.get("materials") or []:
        for attr in material.get("attributes") or []:
            check_file(attr.get("texture"))


class BridgeClient:
    """Reconnecting NDJSON client. Socket work runs on a daemon
    thread; jobs are drained from the DCC main thread with poll()."""

    def __init__(
        self,
        dcc: str,
        dcc_version: str,
        renderers: list[str],
        host: str = "127.0.0.1",
        port: int = 47913,
        reconnect_s: float = 3.0,
    ) -> None:
        self.dcc = dcc
        self.dcc_version = dcc_version
        self.renderers = list(renderers)
        self.host = host
        self.port = port
        self.reconnect_s = reconnect_s
        self._jobs: queue.Queue = queue.Queue()
        self._stop = threading.Event()
        self._send_lock = threading.Lock()
        self._sock: socket.socket | None = None
        self._connected = False
        self._thread: threading.Thread | None = None

    @property
    def connected(self) -> bool:
        return self._connected

    def start(self) -> None:
        self._stop.clear()
        self._thread = threading.Thread(target=self._run, name="uhub-bridge", daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        self._close_socket()
        if self._thread is not None:
            self._thread.join(timeout=2.0)
            self._thread = None

    def poll(self) -> dict | None:
        try:
            return self._jobs.get_nowait()
        except queue.Empty:
            return None

    def reply(self, job_id: str, ok: bool, message: str) -> None:
        line = json.dumps({"type": "result", "jobId": job_id, "ok": ok, "message": message})
        with self._send_lock:
            sock = self._sock
            if sock is None:
                return
            try:
                sock.sendall((line + "\n").encode("utf-8"))
            except OSError:
                pass

    def _close_socket(self) -> None:
        sock = self._sock
        self._sock = None
        self._connected = False
        if sock is not None:
            try:
                sock.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            try:
                sock.close()
            except OSError:
                pass

    def _run(self) -> None:
        while not self._stop.is_set():
            try:
                sock = socket.create_connection((self.host, self.port), timeout=5.0)
            except OSError:
                self._stop.wait(self.reconnect_s)
                continue
            self._sock = sock
            try:
                hello = json.dumps(
                    {
                        "type": "hello",
                        "protocol": 1,
                        "dcc": self.dcc,
                        "dccVersion": self.dcc_version,
                        "pid": os.getpid(),
                        "renderers": self.renderers,
                    }
                )
                with self._send_lock:
                    sock.sendall((hello + "\n").encode("utf-8"))
                self._connected = True
                stream = sock.makefile("r", encoding="utf-8", newline="\n")
                try:
                    for line in stream:
                        if self._stop.is_set():
                            break
                        line = line.strip()
                        if not line:
                            continue
                        try:
                            message = json.loads(line)
                        except ValueError:
                            continue
                        if isinstance(message, dict) and message.get("type") == "import":
                            self._jobs.put(message)
                finally:
                    try:
                        stream.close()
                    except OSError:
                        pass
            except OSError:
                pass
            finally:
                if self._sock is sock:
                    self._sock = None
                self._connected = False
                try:
                    sock.close()
                except OSError:
                    pass
            self._stop.wait(self.reconnect_s)
