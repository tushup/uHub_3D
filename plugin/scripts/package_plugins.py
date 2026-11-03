"""Package the DCC plugins into dist/plugins.

The Blender package is a Blender extension and is built by Blender
itself (which validates the manifest). The Unreal package is a zip
of the plugin folder with the shared core included.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile


def find_blender() -> str | None:
    env = os.environ.get("UHUB_BLENDER")
    if env and os.path.isfile(env):
        return env
    found = shutil.which("blender")
    if found:
        return found
    base = os.path.join(os.environ.get("ProgramFiles", "C:/Program Files"), "Blender Foundation")
    if os.path.isdir(base):
        versions = sorted(os.listdir(base), reverse=True)
        for version in versions:
            candidate = os.path.join(base, version, "blender.exe")
            if os.path.isfile(candidate):
                return candidate
    return None


def copy_core(source_plugin: str, python_root: str) -> None:
    plugin_dir = os.path.dirname(os.path.abspath(__file__))
    plugin_root = os.path.dirname(plugin_dir)
    core = os.path.join(plugin_root, "common", "uhub")
    shutil.copytree(core, os.path.join(python_root, "uhub"))


def build_blender(plugin_root: str, dist: str) -> str:
    blender = find_blender()
    if blender is None:
        raise SystemExit("Blender executable not found (set UHUB_BLENDER)")
    staging = tempfile.mkdtemp(prefix="uhub3d-blender-package-")
    source = os.path.join(staging, "uhub3d")
    shutil.copytree(os.path.join(plugin_root, "blender", "uhub3d"), source)
    copy_core(source, source)
    os.makedirs(dist, exist_ok=True)
    result = subprocess.run(
        [
            blender,
            "--command",
            "extension",
            "build",
            "--source-dir",
            source,
            "--output-dir",
            dist,
        ],
        capture_output=True,
        text=True,
    )
    print(result.stdout)
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr)
        raise SystemExit(f"blender extension build failed with exit code {result.returncode}")
    zips = sorted(
        os.path.join(dist, name) for name in os.listdir(dist) if name.endswith(".zip")
    )
    if not zips:
        raise SystemExit("blender extension build produced no zip")
    return zips[-1]


def build_unreal(plugin_root: str, dist: str) -> str | None:
    source_root = os.path.join(plugin_root, "unreal", "uHub3D")
    if not os.path.isdir(source_root):
        return None
    staging = tempfile.mkdtemp(prefix="uhub3d-unreal-package-")
    staged = os.path.join(staging, "uHub3D")
    shutil.copytree(source_root, staged)
    python_root = os.path.join(staged, "Content", "Python")
    os.makedirs(python_root, exist_ok=True)
    copy_core(staged, python_root)
    os.makedirs(dist, exist_ok=True)
    archive = shutil.make_archive(os.path.join(dist, "uhub3d-unreal"), "zip", staging, "uHub3D")
    return archive


def main() -> int:
    plugin_dir = os.path.dirname(os.path.abspath(__file__))
    plugin_root = os.path.dirname(plugin_dir)
    dist = os.path.join(plugin_root, "dist")
    blender_zip = build_blender(plugin_root, dist)
    print(f"Blender: {blender_zip}")
    unreal_zip = build_unreal(plugin_root, dist)
    if unreal_zip:
        print(f"Unreal: {unreal_zip}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
