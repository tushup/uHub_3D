"""Build uHub 3D assets in Blender: models, materials and HDRIs."""

from __future__ import annotations

import math
import os

import bpy

from .uhub.client import JobError, validate_job
from .uhub.plan import plan_material, srgb_to_linear, strip_dup_suffix

# Blender's Normal Map node expects OpenGL-style green.
TARGET_NORMAL_Y = "OpenGL"


def _socket(node, name):
    sock = node.inputs.get(name)
    if sock is None:
        raise JobError(
            f"Principled socket '{name}' not found in Blender {bpy.app.version_string}; "
            f"available: {[s.name for s in node.inputs]}"
        )
    return sock


def _image_node(nodes, slot):
    node = nodes.new("ShaderNodeTexImage")
    image = bpy.data.images.load(slot.texture, check_existing=True)
    node.image = image
    image.colorspace_settings.name = "sRGB" if slot.srgb else "Non-Color"
    return node


def build_material(mat, slots, renderer: str) -> None:
    if bpy.app.version < (5, 0, 0):
        # Blender 5 always has node trees (M2 Task 16).
        mat.use_nodes = True
    node_tree = mat.node_tree
    nodes = node_tree.nodes
    nodes.clear()
    links = node_tree.links

    principled = nodes.new("ShaderNodeBsdfPrincipled")
    output = nodes.new("ShaderNodeOutputMaterial")
    links.new(principled.outputs["BSDF"], output.inputs["Surface"])

    ao_image_output = None
    base_color_output = None

    for slot in slots:
        if slot.kind == "Opacity":
            # Dithered transparency shows holes like the path tracer.
            try:
                mat.surface_render_method = "DITHERED"
            except (AttributeError, TypeError):
                pass
        if slot.texture:
            image_node = _image_node(nodes, slot)
            color = image_node.outputs["Color"]
            if slot.kind == "BaseColor":
                links.new(color, _socket(principled, "Base Color"))
                base_color_output = color
            elif slot.kind == "Metallic":
                links.new(color, _socket(principled, "Metallic"))
            elif slot.kind == "Roughness":
                links.new(color, _socket(principled, "Roughness"))
            elif slot.kind == "Specular":
                links.new(color, _socket(principled, "Specular IOR Level"))
            elif slot.kind == "Opacity":
                links.new(color, _socket(principled, "Alpha"))
            elif slot.kind == "Emissive":
                links.new(color, _socket(principled, "Emission Color"))
                _socket(principled, "Emission Strength").default_value = (
                    slot.strength if slot.strength is not None else 1.0
                )
            elif slot.kind == "Normal":
                normal_node = nodes.new("ShaderNodeNormalMap")
                normal_node.inputs["Strength"].default_value = (
                    slot.strength if slot.strength is not None else 1.0
                )
                if slot.flip_green:
                    separate = nodes.new("ShaderNodeSeparateColor")
                    links.new(color, separate.inputs["Color"])
                    invert = nodes.new("ShaderNodeMath")
                    invert.operation = "SUBTRACT"
                    invert.inputs[0].default_value = 1.0
                    links.new(separate.outputs["Green"], invert.inputs[1])
                    combine = nodes.new("ShaderNodeCombineColor")
                    links.new(separate.outputs["Red"], combine.inputs["Red"])
                    links.new(invert.outputs["Value"], combine.inputs["Green"])
                    links.new(separate.outputs["Blue"], combine.inputs["Blue"])
                    links.new(combine.outputs["Color"], normal_node.inputs["Color"])
                else:
                    links.new(color, normal_node.inputs["Color"])
                links.new(normal_node.outputs["Normal"], _socket(principled, "Normal"))
            elif slot.kind == "Height":
                displacement = nodes.new("ShaderNodeDisplacement")
                displacement.inputs["Scale"].default_value = (
                    slot.strength if slot.strength is not None else 0.1
                )
                links.new(color, displacement.inputs["Height"])
                links.new(displacement.outputs["Displacement"], output.inputs["Displacement"])
                if renderer == "cycles":
                    try:
                        mat.displacement_method = "BOTH"
                    except (AttributeError, TypeError):
                        pass
            elif slot.kind == "AO":
                ao_image_output = color
            continue
        if slot.value is None:
            continue
        if slot.kind == "BaseColor":
            linear = srgb_to_linear([float(c) for c in slot.value])
            _socket(principled, "Base Color").default_value = (*linear, 1.0)
        elif slot.kind == "Emissive":
            linear = srgb_to_linear([float(c) for c in slot.value])
            _socket(principled, "Emission Color").default_value = (*linear, 1.0)
            _socket(principled, "Emission Strength").default_value = (
                slot.strength if slot.strength is not None else 1.0
            )
        elif slot.kind == "Metallic":
            _socket(principled, "Metallic").default_value = float(slot.value)
        elif slot.kind == "Roughness":
            _socket(principled, "Roughness").default_value = float(slot.value)
        elif slot.kind == "Specular":
            _socket(principled, "Specular IOR Level").default_value = float(slot.value)
        elif slot.kind == "Opacity":
            _socket(principled, "Alpha").default_value = float(slot.value)

    if ao_image_output is not None and base_color_output is not None:
        # AO multiplies into Base Color (Mix in Multiply mode).
        base_socket = _socket(principled, "Base Color")
        for link in list(base_socket.links):
            links.remove(link)
        mix = nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.blend_type = "MULTIPLY"
        mix.inputs[0].default_value = 1.0
        links.new(base_color_output, mix.inputs[6])
        links.new(ao_image_output, mix.inputs[7])
        links.new(mix.outputs[2], base_socket)


def import_model(asset, asset_dir, renderer) -> str:
    model_file = asset["model"]["file"]
    filepath = os.path.join(asset_dir, model_file)
    extension = os.path.splitext(model_file)[1].lower()
    if extension == ".fbx":
        bpy.ops.import_scene.fbx(filepath=filepath)
    elif extension == ".obj":
        bpy.ops.wm.obj_import(filepath=filepath)
    else:
        raise JobError(f"Unsupported model format: {extension}")

    imported_materials = []
    for obj in bpy.context.selected_objects:
        if obj.type != "MESH":
            continue
        for slot in obj.material_slots:
            if slot.material is not None and slot.material not in imported_materials:
                imported_materials.append(slot.material)

    for mat in imported_materials:
        match = None
        for candidate in asset.get("materials", []):
            stripped = strip_dup_suffix(mat.name)
            if stripped == candidate.get("originalName") or stripped == candidate.get("name"):
                match = candidate
                break
        if match is None:
            continue
        # Assigning an existing name makes Blender add its own suffix,
        # so a second import in one session yields Seat.001.
        mat.name = match["name"]
        build_material(mat, plan_material(match, asset_dir, TARGET_NORMAL_Y), renderer)
    return f"Imported {asset['name']}"


def create_material(asset, asset_dir, renderer) -> str:
    materials = asset.get("materials") or []
    source = materials[0] if materials else {"name": asset["name"], "attributes": []}
    mat = bpy.data.materials.new(name=source.get("name", asset["name"]))
    mat.use_fake_user = True
    build_material(mat, plan_material(source, asset_dir, TARGET_NORMAL_Y), renderer)
    for obj in bpy.context.selected_objects:
        if obj.type != "MESH":
            continue
        if obj.data.materials:
            obj.data.materials[0] = mat
        else:
            obj.data.materials.append(mat)
    return f"Created material {mat.name}"


def setup_world(hdr_path: str, rotation_deg: float, intensity: float) -> str:
    scene = bpy.context.scene
    world = scene.world
    if world is None:
        world = bpy.data.worlds.new("World")
        scene.world = world
    if bpy.app.version < (5, 0, 0):
        world.use_nodes = True
    node_tree = world.node_tree
    nodes = node_tree.nodes
    nodes.clear()
    links = node_tree.links

    tex_coord = nodes.new("ShaderNodeTexCoord")
    mapping = nodes.new("ShaderNodeMapping")
    mapping.inputs["Rotation"].default_value[2] = math.radians(rotation_deg)
    environment = nodes.new("ShaderNodeTexEnvironment")
    environment.image = bpy.data.images.load(hdr_path, check_existing=True)
    background = nodes.new("ShaderNodeBackground")
    background.inputs["Strength"].default_value = intensity
    output = nodes.new("ShaderNodeOutputWorld")

    links.new(tex_coord.outputs["Generated"], mapping.inputs["Vector"])
    links.new(mapping.outputs["Vector"], environment.inputs["Vector"])
    links.new(environment.outputs["Color"], background.inputs["Color"])
    links.new(background.outputs["Background"], output.inputs["Surface"])
    return f"World set to {os.path.basename(hdr_path)}"


def run_job(job: dict) -> str:
    validate_job(job)
    asset = job["asset"]
    asset_dir = job["assetDir"]
    renderer = job.get("renderer", "cycles")
    asset_type = asset.get("type")
    if asset_type == "model":
        return import_model(asset, asset_dir, renderer)
    if asset_type == "material":
        return create_material(asset, asset_dir, renderer)
    if asset_type == "hdri":
        hdri = asset.get("hdri") or {}
        return setup_world(
            os.path.join(asset_dir, hdri["file"]),
            float(hdri.get("rotation", 0.0)),
            float(hdri.get("intensity", 1.0)),
        )
    raise JobError(f"Unsupported asset type: {asset_type}")
