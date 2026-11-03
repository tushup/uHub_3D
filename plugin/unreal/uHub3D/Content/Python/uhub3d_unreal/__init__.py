"""uHub 3D Unreal plugin: bridge connection on the editor main thread."""

from __future__ import annotations

import time

_client = None
_last_poll = 0.0
_callback_handle = None


def start(port: int = 47913) -> None:
    global _client, _callback_handle
    import unreal

    from uhub.client import BridgeClient

    from . import importer

    if _client is not None:
        return
    _client = BridgeClient(
        "unreal",
        unreal.SystemLibrary.get_engine_version(),
        ["unreal"],
        port=port,
    )
    _client.start()

    def _tick(_delta_seconds: float) -> None:
        global _last_poll
        now = time.monotonic()
        if now - _last_poll < 0.2:
            return
        _last_poll = now
        if _client is None:
            return
        job = _client.poll()
        if job is None:
            return
        try:
            message = importer.run_job(job)
            _client.reply(job.get("jobId", ""), True, message)
        except Exception as error:  # reported back to the app as a failed job
            _client.reply(job.get("jobId", ""), False, str(error))

    try:
        _callback_handle = unreal.register_slate_post_tick_callback(_tick)
    except Exception:
        # Headless commandlets have no slate; the runner calls run_job directly.
        _callback_handle = None


def stop() -> None:
    global _client, _callback_handle
    if _callback_handle is not None:
        try:
            import unreal

            unreal.unregister_slate_post_tick_callback(_callback_handle)
        except Exception:
            pass
        _callback_handle = None
    if _client is not None:
        _client.stop()
        _client = None
