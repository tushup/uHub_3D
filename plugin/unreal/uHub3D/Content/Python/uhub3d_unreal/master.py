"""The two uHub 3D master materials, built once in /Game/uHub3D/_Core.

Every kind gets a texture parameter, a value parameter and a static
switch Use_<Kind>_Tex (Normal has a flat-normal constant instead of
a value). Strenght/normal-strength parameters are not created.
"""

from __future__ import annotations

import unreal

CORE_PATH = "/Game/uHub3D/_Core"

# kind -> (material property, texture output pin, is vector value)
_KINDS = {
    "BaseColor": (unreal.MaterialProperty.MP_BASE_COLOR, "RGB", True),
    "Metallic": (unreal.MaterialProperty.MP_METALLIC, "R", False),
    "Roughness": (unreal.MaterialProperty.MP_ROUGHNESS, "R", False),
    "Specular": (unreal.MaterialProperty.MP_SPECULAR, "R", False),
    "Normal": (unreal.MaterialProperty.MP_NORMAL, "RGB", False),
    "AO": (unreal.MaterialProperty.MP_AMBIENT_OCCLUSION, "R", False),
    "Opacity": (unreal.MaterialProperty.MP_OPACITY_MASK, "R", False),
    "Emissive": (unreal.MaterialProperty.MP_EMISSIVE_COLOR, "RGB", True),
    # Height gets parameters but is not wired.
    "Height": (None, "R", False),
}


def _create(material, expression_class, x, y):
    return unreal.MaterialEditingLibrary.create_material_expression(
        material, expression_class, x, y
    )


def _build(material, masked: bool) -> None:
    if masked:
        material.set_editor_property("blend_mode", unreal.BlendMode.BLEND_MASKED)
        material.set_editor_property("opacity_mask_clip_value", 0.333)
    library = unreal.MaterialEditingLibrary
    for index, (kind, (prop, tex_pin, is_vector)) in enumerate(_KINDS.items()):
        y = index * 220
        texture_param = _create(
            material, unreal.MaterialExpressionTextureSampleParameter2D, -600, y
        )
        texture_param.set_editor_property("parameter_name", f"{kind}_Tex")
        if kind == "Normal":
            try:
                texture_param.set_editor_property(
                    "sampler_type", unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL
                )
            except (AttributeError, TypeError):
                pass
            # Default to the engine flat normal so materials without a
            # Normal map do not sample the generic DefaultTexture (which
            # makes the MI editor warn "samples DefaultTexture as Normal").
            default_normal = unreal.load_asset(
                "/Engine/EngineMaterials/DefaultNormal.DefaultNormal"
            )
            if default_normal is not None:
                texture_param.set_editor_property("texture", default_normal)
        switch = _create(
            material, unreal.MaterialExpressionStaticSwitchParameter, -150, y
        )
        switch.set_editor_property("parameter_name", f"Use_{kind}_Tex")
        library.connect_material_expressions(texture_param, tex_pin, switch, "True")
        if kind == "Normal":
            flat = _create(material, unreal.MaterialExpressionConstant3Vector, -600, y + 120)
            flat.set_editor_property(
                "constant", unreal.LinearColor(0.5, 0.5, 1.0, 1.0)
            )
            library.connect_material_expressions(flat, "", switch, "False")
        else:
            if is_vector:
                value_param = _create(
                    material, unreal.MaterialExpressionVectorParameter, -600, y + 120
                )
                value_param.set_editor_property(
                    "default_value", unreal.LinearColor(0.0, 0.0, 0.0, 1.0)
                )
            else:
                value_param = _create(
                    material, unreal.MaterialExpressionScalarParameter, -600, y + 120
                )
                value_param.set_editor_property("default_value", 0.0)
            value_param.set_editor_property("parameter_name", f"{kind}_Val")
            library.connect_material_expressions(value_param, "", switch, "False")
        if prop is None:
            continue
        if kind == "Emissive":
            strength = _create(
                material, unreal.MaterialExpressionScalarParameter, -150, y + 120
            )
            strength.set_editor_property("parameter_name", "Emissive_Strength")
            strength.set_editor_property("default_value", 1.0)
            multiply = _create(material, unreal.MaterialExpressionMultiply, 100, y)
            library.connect_material_expressions(switch, "", multiply, "A")
            library.connect_material_expressions(strength, "", multiply, "B")
            library.connect_material_property(multiply, "", prop)
        else:
            library.connect_material_property(switch, "", prop)
    unreal.MaterialEditingLibrary.recompile_material(material)


def ensure_masters():
    """Create M_uHub3D and M_uHub3D_Masked if needed; return both."""
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    unreal.EditorAssetLibrary.make_directory(CORE_PATH)
    masters = []
    for masked, name in ((False, "M_uHub3D"), (True, "M_uHub3D_Masked")):
        path = f"{CORE_PATH}/{name}"
        if unreal.EditorAssetLibrary.does_asset_exist(path):
            masters.append(unreal.EditorAssetLibrary.load_asset(path))
            continue
        material = tools.create_asset(
            name, CORE_PATH, unreal.Material, unreal.MaterialFactoryNew()
        )
        _build(material, masked)
        unreal.EditorAssetLibrary.save_asset(path)
        masters.append(material)
    return masters[0], masters[1]


def master_for(masked: bool):
    opaque, masked_master = ensure_masters()
    return masked_master if masked else opaque
