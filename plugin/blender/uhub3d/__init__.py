"""uHub 3D Blender extension: receive library assets over the bridge."""

from __future__ import annotations

import bpy

from . import importer
from .uhub.client import BridgeClient

_client: BridgeClient | None = None


class UHUB_Preferences(bpy.types.AddonPreferences):
    bl_idname = __package__

    port: bpy.props.IntProperty(
        name="Bridge port",
        default=47913,
        min=1,
        max=65535,
    )

    def draw(self, context):
        layout = self.layout
        layout.prop(self, "port")
        if _client is not None and _client.connected:
            layout.label(text="Status: connected to uHub 3D")
        else:
            layout.label(text="Status: not connected (uHub 3D not reachable)")


def _tick():
    if _client is None:
        return None
    job = _client.poll()
    if job is not None:
        try:
            message = importer.run_job(job)
            _client.reply(job.get("jobId", ""), True, message)
        except Exception as error:  # reported back to the app as a failed job
            _client.reply(job.get("jobId", ""), False, str(error))
    return 0.2


def _preferences_port() -> int:
    try:
        prefs = bpy.context.preferences.addons[__package__].preferences
        return int(prefs.port)
    except (KeyError, AttributeError, TypeError, ValueError):
        return 47913


def register():
    global _client
    bpy.utils.register_class(UHUB_Preferences)
    _client = BridgeClient("blender", bpy.app.version_string, ["cycles", "eevee"], port=_preferences_port())
    _client.start()
    if not bpy.app.timers.is_registered(_tick):
        bpy.app.timers.register(_tick, first_interval=0.2, persistent=True)


def unregister():
    global _client
    if bpy.app.timers.is_registered(_tick):
        bpy.app.timers.unregister(_tick)
    if _client is not None:
        _client.stop()
        _client = None
    bpy.utils.unregister_class(UHUB_Preferences)
