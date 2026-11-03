import json
import os
import socket
import threading
import time

import pytest

from uhub.client import BridgeClient, JobError, validate_job


def make_job(root, asset_dir, asset):
    return {
        "type": "import",
        "jobId": "job-1",
        "renderer": "cycles",
        "libraryRoot": str(root),
        "assetDir": str(asset_dir),
        "asset": asset,
    }


def material_asset(texture="Chair_Seat_BaseColor.png"):
    return {
        "schema": 1,
        "id": "x",
        "type": "model",
        "name": "Chair",
        "category": "Furniture",
        "tags": [],
        "created": "2026-10-30T00:00:00Z",
        "thumbnail": "thumbnail.png",
        "model": {"file": "Chair.fbx", "format": "fbx", "meshes": 1, "vertices": 8},
        "materials": [
            {
                "originalName": "lambert3",
                "name": "Seat",
                "attributes": [{"kind": "BaseColor", "texture": texture}],
            }
        ],
        "hdri": None,
    }


def write_asset_files(asset_dir, asset, skip=()):
    asset_dir.mkdir(parents=True, exist_ok=True)
    if asset.get("model"):
        (asset_dir / asset["model"]["file"]).write_bytes(b"fbx")
    for material in asset.get("materials", []):
        for attr in material.get("attributes", []):
            texture = attr.get("texture")
            if texture and texture not in skip:
                (asset_dir / texture).write_bytes(b"png")


def test_validate_job_rejects_outside_root(tmp_path):
    root = tmp_path / "lib"
    other = tmp_path / "other"
    asset = material_asset()
    write_asset_files(other, asset)
    with pytest.raises(JobError):
        validate_job(make_job(root, other, asset))


def test_validate_job_rejects_missing_texture(tmp_path):
    root = tmp_path / "lib"
    asset_dir = root / "3D" / "Furniture" / "Chair"
    asset = material_asset()
    write_asset_files(asset_dir, asset, skip={"Chair_Seat_BaseColor.png"})
    with pytest.raises(JobError):
        validate_job(make_job(root, asset_dir, asset))


def test_validate_job_accepts_unicode_paths(tmp_path):
    root = tmp_path / "Bibliothèque Ü"
    asset_dir = root / "3D" / "Furniture" / "Chair"
    asset = material_asset("Wood Base.png")
    write_asset_files(asset_dir, asset)
    validate_job(make_job(root, asset_dir, asset))


def test_validate_job_accepts_m1_asset(tmp_path):
    # M1-era shape: thumbnail null, no view key, plus a future key.
    root = tmp_path / "lib"
    asset_dir = root / "3D" / "Furniture" / "Fixture_Chair"
    asset = {
        "schema": 1,
        "id": "fixture",
        "type": "model",
        "name": "Fixture_Chair",
        "category": "Furniture",
        "tags": [],
        "created": "2026-10-30T00:00:00Z",
        "thumbnail": None,
        "model": {"file": "Fixture_Chair.fbx", "format": "fbx", "meshes": 2, "vertices": 24},
        "materials": [
            {
                "originalName": "Red",
                "name": "Seat",
                "attributes": [{"kind": "BaseColor", "texture": "Fixture_Chair_Seat_BaseColor.png"}],
            }
        ],
        "hdri": None,
        "futureKey": {"anything": True},
    }
    write_asset_files(asset_dir, asset)
    validate_job(make_job(root, asset_dir, asset))


def test_validate_job_ignores_unreferenced_missing_files(tmp_path):
    # A texture the asset does not reference is not checked.
    root = tmp_path / "lib"
    asset_dir = root / "Materials" / "Wood" / "Oak"
    asset_dir.mkdir(parents=True)
    asset = {
        "schema": 1,
        "id": "m",
        "type": "material",
        "name": "Oak",
        "category": "Wood",
        "tags": [],
        "created": "2026-10-30T00:00:00Z",
        "thumbnail": None,
        "model": None,
        "materials": [
            {
                "originalName": "Oak",
                "name": "Oak",
                "attributes": [{"kind": "Roughness", "value": 0.5}],
            }
        ],
        "hdri": None,
    }
    validate_job(make_job(root, asset_dir, asset))


class FakeServer:
    def __init__(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(1)
        self.port = self.sock.getsockname()[1]
        self.conn = None
        self.file = None

    def accept(self):
        self.conn, _ = self.sock.accept()
        self.file = self.conn.makefile("r", encoding="utf-8", newline="\n")
        return self.file.readline()

    def send(self, obj):
        self.conn.sendall((json.dumps(obj) + "\n").encode("utf-8"))

    def readline(self):
        return self.file.readline()

    def close(self):
        for obj in (self.file, self.conn, self.sock):
            try:
                if obj is not None:
                    obj.close()
            except OSError:
                pass


def wait_for(predicate, timeout=5.0):
    start = time.time()
    while time.time() - start < timeout:
        if predicate():
            return True
        time.sleep(0.02)
    return False


def test_client_hello_and_job():
    server = FakeServer()
    client = BridgeClient("blender", "5.1.2", ["cycles", "eevee"], port=server.port, reconnect_s=0.1)
    client.start()
    try:
        hello = json.loads(server.accept())
        assert hello["type"] == "hello"
        assert hello["protocol"] == 1
        assert hello["dcc"] == "blender"
        assert hello["dccVersion"] == "5.1.2"
        assert hello["renderers"] == ["cycles", "eevee"]
        job = {
            "type": "import",
            "jobId": "job-9",
            "renderer": "cycles",
            "libraryRoot": "C:/lib",
            "assetDir": "C:/lib/Chair",
            "asset": {"name": "Chair"},
        }
        server.send(job)
        received = None
        deadline = time.time() + 5
        while time.time() < deadline and received is None:
            received = client.poll()
            if received is None:
                time.sleep(0.02)
        assert received is not None and received["jobId"] == "job-9"
        client.reply("job-9", True, "Imported Chair")
        result = json.loads(server.readline())
        assert result == {"type": "result", "jobId": "job-9", "ok": True, "message": "Imported Chair"}
        assert client.connected
    finally:
        client.stop()
        server.close()


def test_client_socket_is_blocking_after_connect():
    # Regression: create_connection(timeout=5) leaves the timeout on the
    # socket, so an idle connection died after 5 s and reconnect-looped.
    server = FakeServer()
    client = BridgeClient("blender", "5.1.2", ["cycles"], port=server.port, reconnect_s=0.1)
    client.start()
    try:
        json.loads(server.accept())
        assert wait_for(lambda: client.connected)
        assert client._sock is not None
        assert client._sock.gettimeout() is None
    finally:
        client.stop()
        server.close()


def test_client_reconnects():
    server = FakeServer()
    client = BridgeClient("blender", "5.1.2", ["cycles"], port=server.port, reconnect_s=0.1)
    # Start the server listener only after the client has begun retrying:
    # the FakeServer socket already listens, so close and rebind to force retries.
    port = server.port
    server.close()
    client.start()
    time.sleep(0.35)
    server2 = FakeServer.__new__(FakeServer)
    server2.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server2.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    deadline = time.time() + 5
    while True:
        try:
            server2.sock.bind(("127.0.0.1", port))
            break
        except OSError:
            if time.time() > deadline:
                raise
            time.sleep(0.05)
    server2.sock.listen(1)
    server2.port = port
    server2.conn = None
    server2.file = None
    try:
        hello = json.loads(server2.accept())
        assert hello["dcc"] == "blender"
    finally:
        client.stop()
        server2.close()
