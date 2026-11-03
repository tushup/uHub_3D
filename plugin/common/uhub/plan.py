"""Renderer-neutral material planning, shared by every DCC plugin."""

from __future__ import annotations

import os
import re
from dataclasses import dataclass

SRGB_KINDS = {"BaseColor", "Emissive"}


@dataclass
class Slot:
    kind: str
    texture: str | None = None  # absolute path
    value: float | list[float] | None = None
    strength: float | None = None
    srgb: bool = False
    flip_green: bool = False


def plan_material(material: dict, asset_dir: str, target_normal_y: str) -> list[Slot]:
    """One Slot per attribute, in input order.

    When an attribute has a texture, the texture wins and the value
    is not wired (D-36), exactly as in the path tracer.
    """
    slots: list[Slot] = []
    for attr in material.get("attributes", []):
        kind = attr.get("kind", "")
        texture = attr.get("texture")
        texture_path = os.path.join(asset_dir, texture) if texture else None
        value = None if texture_path else attr.get("value")
        normal_y = attr.get("normalY", "OpenGL")
        slots.append(
            Slot(
                kind=kind,
                texture=texture_path,
                value=value,
                strength=attr.get("strength"),
                srgb=kind in SRGB_KINDS,
                flip_green=kind == "Normal" and normal_y != target_normal_y,
            )
        )
    return slots


def srgb_to_linear(c: list[float]) -> list[float]:
    """Piecewise IEC 61966-2-1 conversion, per channel."""

    def convert(x: float) -> float:
        if x <= 0.04045:
            return x / 12.92
        return ((x + 0.055) / 1.055) ** 2.4

    return [convert(float(x)) for x in c]


def strip_dup_suffix(name: str) -> str:
    """'Red.001' -> 'Red', 'ns:Red' -> 'Red'."""
    base = name.rsplit(":", 1)[-1]
    return re.sub(r"\.\d+$", "", base)
