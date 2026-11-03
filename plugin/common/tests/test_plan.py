import os

import pytest

from uhub.plan import Slot, plan_material, srgb_to_linear, strip_dup_suffix

D = os.path.join("C:", "lib", "Chair")


def mat(*attrs):
    return {"originalName": "Mat", "name": "Seat", "attributes": list(attrs)}


def test_flip_green_only_on_mismatch():
    s = plan_material(
        mat({"kind": "Normal", "texture": "n.png", "normalY": "OpenGL", "strength": 1}),
        D,
        "DirectX",
    )
    assert s[0].flip_green and not s[0].srgb
    assert s[0].texture == os.path.join(D, "n.png")
    s2 = plan_material(
        mat({"kind": "Normal", "texture": "n.png", "normalY": "DirectX", "strength": 1}),
        D,
        "DirectX",
    )
    assert not s2[0].flip_green


def test_srgb_flags():
    s = plan_material(
        mat(
            {"kind": "BaseColor", "value": [0.8, 0.8, 0.8]},
            {"kind": "Roughness", "value": 0.5},
            {"kind": "Emissive", "value": [1.0, 0.0, 0.0]},
        ),
        D,
        "OpenGL",
    )
    assert [slot.srgb for slot in s] == [True, False, True]
    assert isinstance(s[0], Slot)


def test_srgb_to_linear():
    assert srgb_to_linear([1, 0, 0.5]) == pytest.approx([1, 0, 0.2140], abs=1e-4)


def test_strip_dup_suffix():
    assert strip_dup_suffix("Red.001") == "Red"
    assert strip_dup_suffix("ns:Red") == "Red"
    assert strip_dup_suffix("Red") == "Red"


def test_texture_wins_over_value():
    # D-36: with both present, the texture is used and the value is not wired.
    s = plan_material(
        mat({"kind": "BaseColor", "value": [0.8, 0.8, 0.8], "texture": "a.png"}),
        D,
        "OpenGL",
    )
    assert s[0].texture == os.path.join(D, "a.png")
    assert s[0].value is None


def test_value_kept_without_texture():
    s = plan_material(mat({"kind": "Roughness", "value": 0.42}), D, "OpenGL")
    assert s[0].value == 0.42
    assert s[0].texture is None
