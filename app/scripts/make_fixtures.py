"""Generate model/image fixtures for the Rust tests.

Runs as: blender -b --factory-startup -P scripts/make_fixtures.py -- <out_dir>
Creates a cube (8 verts) whose two faces use material Red (with red.png in
Base Color) and the rest use Blue, exports FBX/OBJ, and writes red.png,
gray.tga, big.png, sky.hdr and broken.fbx into <out_dir>.
"""

import os
import sys

import bpy


def out_dir_from_argv() -> str:
    argv = sys.argv
    sep = argv.index("--")
    return os.path.abspath(argv[sep + 1])


def save_image(img, path: str, file_format: str) -> None:
    img.file_format = file_format
    img.filepath_raw = path
    img.save()


def make_solid_image(name: str, width: int, height: int, rgba, path: str, file_format: str):
    img = bpy.data.images.new(name, width=width, height=height)
    img.pixels.foreach_set(list(rgba) * (width * height))
    save_image(img, path, file_format)
    return img


def main() -> None:
    out = out_dir_from_argv()
    os.makedirs(out, exist_ok=True)

    bpy.ops.wm.read_factory_settings(use_empty=True)

    red_img = make_solid_image(
        "red.png", 64, 64, (1.0, 0.0, 0.0, 1.0), os.path.join(out, "red.png"), "PNG"
    )
    make_solid_image(
        "gray.tga", 64, 64, (0.5, 0.5, 0.5, 1.0), os.path.join(out, "gray.tga"), "TARGA"
    )
    make_solid_image(
        "big.png", 1024, 1024, (0.25, 0.5, 0.75, 1.0), os.path.join(out, "big.png"), "PNG"
    )

    # HDR sky: 512x256 float gradient.
    sky = bpy.data.images.new("sky", width=512, height=256, float_buffer=True)
    pixels = []
    for y in range(256):
        for x in range(512):
            v = 0.2 + (x / 511.0) * 1.8 + (y / 255.0) * 0.5
            pixels.extend((v, v, v, 1.0))
    sky.pixels.foreach_set(pixels)
    save_image(sky, os.path.join(out, "sky.hdr"), "HDR")

    # Cube with two materials: two faces Red, the rest Blue.
    bpy.ops.mesh.primitive_cube_add(size=2)
    cube = bpy.context.active_object

    red = bpy.data.materials.new("Red")
    red.use_nodes = True
    principled = red.node_tree.nodes["Principled BSDF"]
    tex_node = red.node_tree.nodes.new("ShaderNodeTexImage")
    tex_node.image = red_img
    red.node_tree.links.new(tex_node.outputs["Color"], principled.inputs["Base Color"])

    blue = bpy.data.materials.new("Blue")
    blue.use_nodes = True
    blue.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (
        0.0,
        0.0,
        1.0,
        1.0,
    )

    cube.data.materials.append(red)
    cube.data.materials.append(blue)
    for i, poly in enumerate(cube.data.polygons):
        poly.material_index = 0 if i < 2 else 1

    bpy.ops.export_scene.fbx(filepath=os.path.join(out, "cube_2mat.fbx"), path_mode="ABSOLUTE")
    bpy.ops.wm.obj_export(
        filepath=os.path.join(out, "cube_2mat.obj"),
        export_materials=True,
        path_mode="ABSOLUTE",
    )

    with open(os.path.join(out, "broken.fbx"), "wb") as fh:
        fh.write(bytes(100))

    print("FIXTURES OK:", sorted(os.listdir(out)))


if __name__ == "__main__":
    main()
