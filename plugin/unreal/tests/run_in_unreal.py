"""Headless test runner for the uHub 3D Unreal plugin.

Run with the scratch project from scripts/make_unreal_test_project.py:
  UnrealEditor-Cmd uHubTest.uproject -run=pythonscript -script=<this file> -unattended -nosplash -nullrhi
Prints ALL PASSED and exits 0, or prints failures and exits 1.
"""

import os
import shutil
import sys
import tempfile
import traceback

FAILURES = []


def check(label, condition):
    if condition:
        print(f"PASS {label}", flush=True)
    else:
        print(f"FAIL {label}", flush=True)
        FAILURES.append(label)


def main():
    import unreal

    tests_dir = os.path.dirname(os.path.abspath(__file__))
    plugin_root = os.path.dirname(os.path.dirname(tests_dir))  # uhub3d/plugin -> uhub3d
    product_root = os.path.dirname(plugin_root)
    python_root = os.path.join(plugin_root, "unreal", "uHub3D", "Content", "Python")
    for path in (python_root, os.path.join(plugin_root, "common")):
        if path not in sys.path:
            sys.path.insert(0, path)
    fixtures = os.path.join(product_root, "app", "src-tauri", "tests", "fixtures")

    from uhub3d_unreal import importer
    from uhub.client import JobError

    root = tempfile.mkdtemp(prefix="uhub3d-ue-lib-")
    chair_dir = os.path.join(root, "3D", "Furniture", "Chair")
    os.makedirs(chair_dir)
    shutil.copy(os.path.join(fixtures, "cube_2mat.fbx"), os.path.join(chair_dir, "Chair.fbx"))
    shutil.copy(os.path.join(fixtures, "red.png"), os.path.join(chair_dir, "Chair_Seat_BaseColor.png"))
    shutil.copy(os.path.join(fixtures, "gray.tga"), os.path.join(chair_dir, "Chair_Seat_Normal.tga"))
    shutil.copy(os.path.join(fixtures, "gray.tga"), os.path.join(chair_dir, "Chair_Seat_Roughness.tga"))
    shutil.copy(os.path.join(fixtures, "gray.tga"), os.path.join(chair_dir, "Chair_Seat_Opacity.tga"))

    def model_job():
        return {
            "type": "import",
            "jobId": "job-model",
            "renderer": "unreal",
            "libraryRoot": root,
            "assetDir": chair_dir,
            "asset": {
                "schema": 1,
                "id": "chair",
                "type": "model",
                "name": "Chair",
                "category": "Furniture",
                "tags": [],
                "created": "2026-10-30T00:00:00Z",
                "thumbnail": "thumbnail.png",
                "model": {"file": "Chair.fbx", "format": "fbx", "meshes": 1, "vertices": 8},
                "materials": [
                    {
                        "originalName": "Red",
                        "name": "Seat",
                        "attributes": [
                            {"kind": "BaseColor", "texture": "Chair_Seat_BaseColor.png"},
                            {"kind": "Normal", "texture": "Chair_Seat_Normal.tga", "normalY": "OpenGL", "strength": 1},
                            {"kind": "Roughness", "texture": "Chair_Seat_Roughness.tga"},
                            {"kind": "Opacity", "texture": "Chair_Seat_Opacity.tga"},
                        ],
                    },
                    {
                        "originalName": "Blue",
                        "name": "Legs",
                        "attributes": [{"kind": "BaseColor", "value": [0.2, 0.2, 0.8]}],
                    },
                ],
                "hdri": None,
            },
        }

    def load(path):
        return unreal.EditorAssetLibrary.load_asset(path)

    # --- Model job ---------------------------------------------------
    message = importer.run_job(model_job())
    check("model job returns message", "Chair" in message)
    check(
        "SM at 3D_Assets path",
        load("/Game/uHub3D/3D_Assets/Chair/SM_Chair.SM_Chair") is not None,
    )
    seat_mi = load("/Game/uHub3D/3D_Assets/Chair/Materials/MI_Seat.MI_Seat")
    check("MI_Seat exists", seat_mi is not None)
    if seat_mi is not None:
        parent = seat_mi.get_editor_property("parent")
        check(
            "MI_Seat parent is M_uHub3D_Masked (Opacity present)",
            parent is not None and "M_uHub3D_Masked" in parent.get_path_name(),
        )
    base_tex = load("/Game/uHub3D/3D_Assets/Chair/Textures/T_Chair_Seat_BaseColor.T_Chair_Seat_BaseColor")
    check("BaseColor texture exists", base_tex is not None)
    if base_tex is not None:
        check("BaseColor texture sRGB on", bool(base_tex.get_editor_property("srgb")))
    rough_tex = load("/Game/uHub3D/3D_Assets/Chair/Textures/T_Chair_Seat_Roughness.T_Chair_Seat_Roughness")
    check("Roughness texture exists", rough_tex is not None)
    if rough_tex is not None:
        check("Roughness texture sRGB off", not bool(rough_tex.get_editor_property("srgb")))
    normal_tex = load("/Game/uHub3D/3D_Assets/Chair/Textures/T_Chair_Seat_Normal.T_Chair_Seat_Normal")
    check("Normal texture exists", normal_tex is not None)
    if normal_tex is not None:
        check(
            "Normal texture TC_NORMALMAP",
            normal_tex.get_editor_property("compression_settings")
            == unreal.TextureCompressionSettings.TC_NORMALMAP,
        )
        check(
            "OpenGL Normal flip_green_channel true",
            bool(normal_tex.get_editor_property("flip_green_channel")),
        )

    # --- Re-export ----------------------------------------------------
    second = importer.run_job(model_job())
    check("reexport_same_model_skips", second.startswith("Skipped"))

    # --- Material job ---------------------------------------------------
    paint_dir = os.path.join(root, "Materials", "Paint", "RedPaint")
    os.makedirs(paint_dir)
    material_job = {
        "type": "import",
        "jobId": "job-material",
        "renderer": "unreal",
        "libraryRoot": root,
        "assetDir": paint_dir,
        "asset": {
            "schema": 1,
            "id": "paint",
            "type": "material",
            "name": "RedPaint",
            "category": "Paint",
            "tags": [],
            "created": "2026-10-30T00:00:00Z",
            "thumbnail": None,
            "model": None,
            "materials": [
                {
                    "originalName": "RedPaint",
                    "name": "RedPaint",
                    "attributes": [
                        {"kind": "BaseColor", "value": [0.72, 0.14, 0.1]},
                        {"kind": "Roughness", "value": 0.35},
                    ],
                }
            ],
            "hdri": None,
        },
    }
    importer.run_job(material_job)
    check(
        "material MI at Materials path",
        load("/Game/uHub3D/Materials/RedPaint/MI_RedPaint.MI_RedPaint") is not None,
    )

    # --- HDRI job ---------------------------------------------------------
    hdri_dir = os.path.join(root, "HDRI", "Sky", "Sky")
    os.makedirs(hdri_dir)
    shutil.copy(os.path.join(fixtures, "sky.hdr"), os.path.join(hdri_dir, "Sky.hdr"))
    hdri_job = {
        "type": "import",
        "jobId": "job-hdri",
        "renderer": "unreal",
        "libraryRoot": root,
        "assetDir": hdri_dir,
        "asset": {
            "schema": 1,
            "id": "sky",
            "type": "hdri",
            "name": "Sky",
            "category": "Sky",
            "tags": [],
            "created": "2026-10-30T00:00:00Z",
            "thumbnail": None,
            "model": None,
            "materials": [],
            "hdri": {"file": "Sky.hdr", "width": 512, "height": 256, "rotation": 0.0, "intensity": 1.0},
        },
    }
    importer.run_job(hdri_job)
    hdri_tex = load("/Game/uHub3D/HDRI/Sky/T_Sky_HDRI.T_Sky_HDRI")
    check("HDRI texture exists", hdri_tex is not None)
    if hdri_tex is not None:
        check("HDRI imports as TextureCube", isinstance(hdri_tex, unreal.TextureCube))

    # --- Outside-root job --------------------------------------------------
    bad_job = model_job()
    bad_job["assetDir"] = tempfile.mkdtemp(prefix="uhub3d-outside-")
    try:
        importer.run_job(bad_job)
        check("outside-root job raises JobError", False)
    except JobError:
        check("outside-root job raises JobError", True)


try:
    main()
except Exception:
    traceback.print_exc()
    FAILURES.append("exception")

for failure in FAILURES:
    print(f"FAILURE {failure}", flush=True)
if FAILURES:
    print("TESTS FAILED", flush=True)
    raise RuntimeError(f"uHub Unreal tests failed: {FAILURES}")
print("ALL PASSED", flush=True)
