"""In-Blender test runner for the uHub 3D Blender extension.

Run: blender -b --factory-startup -P blender/tests/run_in_blender.py
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
        print(f"PASS {label}")
    else:
        print(f"FAIL {label}")
        FAILURES.append(label)


def stage_package():
    """Copy the extension plus the shared core into the packaged layout."""
    tests_dir = os.path.dirname(os.path.abspath(__file__))
    plugin_root = os.path.dirname(os.path.dirname(tests_dir))  # uhub3d/plugin -> uhub3d
    product_root = os.path.dirname(plugin_root)
    staging = tempfile.mkdtemp(prefix="uhub3d-blender-test-")
    shutil.copytree(
        os.path.join(plugin_root, "blender", "uhub3d"),
        os.path.join(staging, "uhub3d"),
    )
    shutil.copytree(
        os.path.join(plugin_root, "common", "uhub"),
        os.path.join(staging, "uhub3d", "uhub"),
    )
    sys.path.insert(0, staging)
    fixtures = os.path.join(product_root, "app", "src-tauri", "tests", "fixtures")
    return fixtures


def build_library(fixtures):
    root = tempfile.mkdtemp(prefix="uhub3d-lib-")
    chair_dir = os.path.join(root, "3D", "Furniture", "Chair")
    os.makedirs(chair_dir)
    shutil.copy(os.path.join(fixtures, "cube_2mat.fbx"), os.path.join(chair_dir, "Chair.fbx"))
    shutil.copy(os.path.join(fixtures, "red.png"), os.path.join(chair_dir, "Chair_Seat_BaseColor.png"))
    shutil.copy(os.path.join(fixtures, "gray.tga"), os.path.join(chair_dir, "Chair_Seat_Normal.tga"))
    return root, chair_dir


def model_job(root, chair_dir, name="Chair"):
    return {
        "type": "import",
        "jobId": "job-model",
        "renderer": "cycles",
        "libraryRoot": root,
        "assetDir": chair_dir,
        "asset": {
            "schema": 1,
            "id": "chair",
            "type": "model",
            "name": name,
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
                        {"kind": "Normal", "texture": "Chair_Seat_Normal.tga", "normalY": "DirectX", "strength": 1},
                        {"kind": "Roughness", "value": 0.5},
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


def main():
    import math

    import bpy

    fixtures = stage_package()
    from uhub3d import importer
    from uhub3d.uhub.client import JobError

    root, chair_dir = build_library(fixtures)

    # --- Model job -------------------------------------------------
    message = importer.run_job(model_job(root, chair_dir))
    check("model job returns message", "Chair" in message)
    seat = bpy.data.materials.get("Seat")
    check("material Seat exists", seat is not None)
    if seat is not None:
        nodes = seat.node_tree.nodes
        principled = next((n for n in nodes if n.type == "BSDF_PRINCIPLED"), None)
        check("Seat has Principled", principled is not None)
        if principled is not None:
            base = principled.inputs.get("Base Color")
            linked = bool(base and base.links)
            check("Seat Base Color linked", linked)
            if linked:
                source = base.links[0].from_node
                check("Seat Base Color from image", source.type == "TEX_IMAGE")
                if source.type == "TEX_IMAGE":
                    check(
                        "Seat Base Color sRGB",
                        source.image.colorspace_settings.name == "sRGB",
                    )
        check(
            "Separate Color node for DirectX normal",
            any(n.type == "SEPARATE_COLOR" for n in nodes),
        )
    check("material Legs exists", bpy.data.materials.get("Legs") is not None)
    mesh_objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    check("model imported a mesh object", len(mesh_objects) >= 1)

    # --- Re-export the same model ---------------------------------
    objects_before = len([o for o in bpy.context.scene.objects if o.type == "MESH"])
    importer.run_job(model_job(root, chair_dir))
    objects_after = len([o for o in bpy.context.scene.objects if o.type == "MESH"])
    check("reexport_same_model adds a second object", objects_after == objects_before + 1)
    seat_copies = [m for m in bpy.data.materials if m.name == "Seat" or m.name.startswith("Seat.")]
    built = [
        m
        for m in seat_copies
        if m.node_tree and any(n.type == "BSDF_PRINCIPLED" for n in m.node_tree.nodes)
    ]
    check("reexport_same_model both Seat materials built", len(built) >= 2)

    # --- Material job ----------------------------------------------
    paint_dir = os.path.join(root, "Materials", "Paint", "RedPaint")
    os.makedirs(paint_dir)
    material_job = {
        "type": "import",
        "jobId": "job-material",
        "renderer": "cycles",
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
    paint = bpy.data.materials.get("RedPaint")
    check("material job creates RedPaint", paint is not None)
    if paint is not None:
        check("RedPaint has fake user", paint.use_fake_user)
        check(
            "RedPaint has Principled",
            any(n.type == "BSDF_PRINCIPLED" for n in paint.node_tree.nodes),
        )

    # --- HDRI job ----------------------------------------------------
    hdri_dir = os.path.join(root, "HDRI", "Sky", "Sky")
    os.makedirs(hdri_dir)
    shutil.copy(os.path.join(fixtures, "sky.hdr"), os.path.join(hdri_dir, "Sky.hdr"))
    hdri_job = {
        "type": "import",
        "jobId": "job-hdri",
        "renderer": "cycles",
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
            "hdri": {"file": "Sky.hdr", "width": 512, "height": 256, "rotation": 45.0, "intensity": 2.5},
        },
    }
    importer.run_job(hdri_job)
    world = bpy.context.scene.world
    check("world exists", world is not None and world.node_tree is not None)
    if world is not None and world.node_tree is not None:
        env = next((n for n in world.node_tree.nodes if n.type == "TEX_ENVIRONMENT"), None)
        check("world has Environment Texture", env is not None)
        if env is not None:
            check("world image is Sky.hdr", env.image is not None and env.image.name.startswith("Sky"))
        background = next((n for n in world.node_tree.nodes if n.type == "BACKGROUND"), None)
        check(
            "world Background strength is intensity",
            background is not None
            and abs(background.inputs["Strength"].default_value - 2.5) < 1e-4,
        )
        mapping = next((n for n in world.node_tree.nodes if n.type == "MAPPING"), None)
        check(
            "world Mapping rotation Z is 45 degrees",
            mapping is not None
            and abs(mapping.inputs["Rotation"].default_value[2] - math.radians(45.0)) < 1e-4,
        )

    # --- Unicode library root ---------------------------------------
    uni_root = os.path.join(tempfile.gettempdir(), "Bibliothèque Ü")
    os.makedirs(uni_root, exist_ok=True)
    uni_dir = os.path.join(uni_root, "Materials", "Paint", "UniPaint")
    os.makedirs(uni_dir, exist_ok=True)
    uni_job = {
        "type": "import",
        "jobId": "job-unicode",
        "renderer": "eevee",
        "libraryRoot": uni_root,
        "assetDir": uni_dir,
        "asset": {
            "schema": 1,
            "id": "uni",
            "type": "material",
            "name": "UniPaint",
            "category": "Paint",
            "tags": [],
            "created": "2026-10-30T00:00:00Z",
            "thumbnail": None,
            "model": None,
            "materials": [
                {
                    "originalName": "UniPaint",
                    "name": "UniPaint",
                    "attributes": [{"kind": "BaseColor", "value": [0.1, 0.5, 0.2]}],
                }
            ],
            "hdri": None,
        },
    }
    importer.run_job(uni_job)
    check("unicode root material created", bpy.data.materials.get("UniPaint") is not None)

    # --- Outside-root job is rejected -------------------------------
    bad_job = model_job(root, chair_dir)
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

if FAILURES:
    print(f"FAILURES: {FAILURES}")
    sys.exit(1)
print("ALL PASSED")
