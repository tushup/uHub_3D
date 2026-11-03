"""Import uHub 3D assets into the Content Browser (spec section 8)."""

from __future__ import annotations

import os

import unreal

from uhub.client import JobError, validate_job
from uhub.plan import plan_material, srgb_to_linear, strip_dup_suffix

from . import master as master_module

TARGET_NORMAL_Y = "DirectX"


def _save(path: str) -> None:
    unreal.EditorAssetLibrary.save_asset(path)


def _import_task(filename: str, dest: str, dest_name: str, factory) -> list:
    task = unreal.AssetImportTask()
    task.filename = filename
    task.destination_path = dest
    task.destination_name = dest_name
    task.automated = True
    task.replace_existing = True
    task.save = False
    if factory is not None:
        task.set_editor_property("factory", factory)
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    tools.import_asset_tasks([task])
    return [str(p) for p in task.imported_object_paths]


def import_textures(slots, dest: str, prefix: str) -> dict:
    """Import one texture per textured slot; returns kind -> texture."""
    imported = {}
    for slot in slots:
        if not slot.texture:
            continue
        name = f"{prefix}{slot.kind}"
        paths = _import_task(slot.texture, dest, name, unreal.TextureFactory())
        if not paths:
            raise JobError(f"Texture import produced nothing: {slot.texture}")
        texture = unreal.EditorAssetLibrary.load_asset(paths[0])
        if texture is None:
            raise JobError(f"Texture import produced nothing: {slot.texture}")
        texture.set_editor_property("srgb", bool(slot.srgb))
        if slot.kind == "Normal":
            texture.set_editor_property(
                "compression_settings",
                unreal.TextureCompressionSettings.TC_NORMALMAP,
            )
            texture.set_editor_property("flip_green_channel", bool(slot.flip_green))
        elif not slot.srgb:
            masks = getattr(unreal.TextureCompressionSettings, "TC_MASKS", None)
            if masks is None:
                masks = unreal.TextureCompressionSettings.TC_GRAYSCALE
            texture.set_editor_property("compression_settings", masks)
        _save(f"{dest}/{name}")
        imported[slot.kind] = texture
    return imported


def make_instance(material_name: str, slots, textures: dict, dest: str):
    masked = any(slot.kind == "Opacity" for slot in slots)
    parent = master_module.master_for(masked)
    name = f"MI_{material_name}"
    path = f"{dest}/{name}"
    if unreal.EditorAssetLibrary.does_asset_exist(path):
        return unreal.EditorAssetLibrary.load_asset(path)
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    instance = tools.create_asset(
        name,
        dest,
        unreal.MaterialInstanceConstant,
        unreal.MaterialInstanceConstantFactoryNew(),
    )
    library = unreal.MaterialEditingLibrary
    library.set_material_instance_parent(instance, parent)
    for slot in slots:
        library.set_material_instance_static_switch_parameter_value(
            instance, f"Use_{slot.kind}_Tex", slot.texture is not None
        )
        if slot.texture is not None:
            texture = textures.get(slot.kind)
            if texture is not None:
                library.set_material_instance_texture_parameter_value(
                    instance, f"{slot.kind}_Tex", texture
                )
        elif slot.value is not None:
            if slot.kind in ("BaseColor", "Emissive"):
                values = [float(c) for c in slot.value]
                while len(values) < 3:
                    values.append(0.0)
                linear = srgb_to_linear(values[:3])
                library.set_material_instance_vector_parameter_value(
                    instance,
                    f"{slot.kind}_Val",
                    unreal.LinearColor(linear[0], linear[1], linear[2], 1.0),
                )
            elif slot.kind != "Normal":
                library.set_material_instance_scalar_parameter_value(
                    instance, f"{slot.kind}_Val", float(slot.value)
                )
        if slot.kind == "Emissive" and slot.strength is not None:
            library.set_material_instance_scalar_parameter_value(
                instance, "Emissive_Strength", float(slot.strength)
            )
    _save(path)
    return instance


def import_mesh(filepath: str, dest: str, name: str):
    """FBX and OBJ through Interchange, materials/textures off (D-38)."""
    registry = unreal.AssetRegistryHelpers.get_asset_registry()
    # Search first: loading the pipeline before the registry knows it
    # logs an error that makes the commandlet exit non-zero.
    registry.search_all_assets(True)
    pipeline_path = "/Interchange/Pipelines/DefaultAssetsPipeline.DefaultAssetsPipeline"
    pipeline = unreal.EditorAssetLibrary.load_asset(pipeline_path)
    if pipeline is None:
        raise JobError("Interchange DefaultAssetsPipeline not found")
    material_pipeline = pipeline.get_editor_property("material_pipeline")
    material_pipeline.set_editor_property("import_materials", False)
    texture_pipeline = material_pipeline.get_editor_property("texture_pipeline")
    texture_pipeline.set_editor_property("import_textures", False)

    manager = unreal.InterchangeManager.get_interchange_manager_scripted()
    source = manager.create_source_data(filepath)
    params = unreal.ImportAssetParameters()
    params.set_editor_property("is_automated", True)
    params.set_editor_property("destination_name", f"SM_{name}")
    params.set_editor_property(
        "override_pipelines", [unreal.SoftObjectPath(pipeline_path)]
    )
    objects = manager.import_asset(dest, source, params)
    meshes = [obj for obj in (objects or []) if isinstance(obj, unreal.StaticMesh)]
    if not meshes:
        raise JobError(f"Mesh import produced no StaticMesh: {filepath}")
    return meshes[0]


def import_model(asset, asset_dir: str) -> str:
    name = asset["name"]
    root = f"/Game/uHub3D/3D_Assets/{name}"
    if unreal.EditorAssetLibrary.does_directory_exist(root):
        return f"Skipped {name}: already imported"
    unreal.EditorAssetLibrary.make_directory(root)
    unreal.EditorAssetLibrary.make_directory(f"{root}/Textures")
    unreal.EditorAssetLibrary.make_directory(f"{root}/Materials")

    instances = {}
    for material in asset.get("materials", []):
        slots = plan_material(material, asset_dir, TARGET_NORMAL_Y)
        textures = import_textures(slots, f"{root}/Textures", f"T_{name}_{material['name']}_")
        instance = make_instance(material["name"], slots, textures, f"{root}/Materials")
        instances[material.get("originalName", material["name"])] = instance
        instances[material["name"]] = instance

    mesh_file = os.path.join(asset_dir, asset["model"]["file"])
    mesh = import_mesh(mesh_file, root, name)
    static_materials = mesh.get_editor_property("static_materials")
    for index, static_material in enumerate(static_materials):
        slot_name = str(static_material.get_editor_property("material_slot_name"))
        instance = instances.get(strip_dup_suffix(slot_name)) or instances.get(slot_name)
        if instance is not None:
            mesh.set_material(index, instance)
    unreal.EditorAssetLibrary.save_loaded_asset(mesh, False)
    return f"Imported {name}"


def import_material(asset, asset_dir: str) -> str:
    name = asset["name"]
    root = f"/Game/uHub3D/Materials/{name}"
    if unreal.EditorAssetLibrary.does_directory_exist(root):
        return f"Skipped {name}: already imported"
    unreal.EditorAssetLibrary.make_directory(root)
    unreal.EditorAssetLibrary.make_directory(f"{root}/Textures")
    materials = asset.get("materials") or []
    source = materials[0] if materials else {"name": name, "attributes": []}
    slots = plan_material(source, asset_dir, TARGET_NORMAL_Y)
    textures = import_textures(slots, f"{root}/Textures", f"T_{name}_")
    make_instance(name, slots, textures, root)
    return f"Imported material {name}"


def import_hdri(asset, asset_dir: str) -> str:
    name = asset["name"]
    root = f"/Game/uHub3D/HDRI/{name}"
    if unreal.EditorAssetLibrary.does_directory_exist(root):
        return f"Skipped {name}: already imported"
    unreal.EditorAssetLibrary.make_directory(root)
    hdri = asset.get("hdri") or {}
    paths = _import_task(
        os.path.join(asset_dir, hdri["file"]),
        root,
        f"T_{name}_HDRI",
        unreal.TextureFactory(),
    )
    if not paths:
        raise JobError(f"HDRI import produced nothing: {hdri.get('file')}")
    _save(f"{root}/T_{name}_HDRI")
    return f"Imported HDRI {name}"


def run_job(job: dict) -> str:
    validate_job(job)
    asset = job["asset"]
    asset_dir = job["assetDir"]
    asset_type = asset.get("type")
    if asset_type == "model":
        return import_model(asset, asset_dir)
    if asset_type == "material":
        return import_material(asset, asset_dir)
    if asset_type == "hdri":
        return import_hdri(asset, asset_dir)
    raise JobError(f"Unsupported asset type: {asset_type}")
