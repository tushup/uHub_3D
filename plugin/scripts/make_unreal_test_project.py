"""Write a minimal Unreal test project with the uHub3D plugin copied in.

Usage: python scripts/make_unreal_test_project.py <dir>
"""

from __future__ import annotations

import json
import os
import shutil
import sys


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: make_unreal_test_project.py <dir>", file=sys.stderr)
        return 2
    target = os.path.abspath(argv[1])
    os.makedirs(target, exist_ok=True)

    uproject = {
        "FileVersion": 3,
        "EngineAssociation": "",
        "Category": "",
        "Description": "uHub 3D headless test project",
        "Plugins": [
            {"Name": "PythonScriptPlugin", "Enabled": True},
            {"Name": "EditorScriptingUtilities", "Enabled": True},
        ],
    }
    with open(os.path.join(target, "uHubTest.uproject"), "w", encoding="utf-8") as handle:
        json.dump(uproject, handle, indent=2)

    scripts_dir = os.path.dirname(os.path.abspath(__file__))
    plugin_root = os.path.dirname(scripts_dir)
    source = os.path.join(plugin_root, "unreal", "uHub3D")
    destination = os.path.join(target, "Plugins", "uHub3D")
    if os.path.isdir(destination):
        shutil.rmtree(destination)
    shutil.copytree(source, destination)
    core = os.path.join(plugin_root, "common", "uhub")
    shutil.copytree(
        core,
        os.path.join(destination, "Content", "Python", "uhub"),
        ignore=shutil.ignore_patterns("__pycache__", "*.pyc"),
    )
    print(f"Wrote {target}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
