import type { CSSProperties } from "react";

/** Structural minimum shared by saved materials and draft materials. */
export interface SphereMaterial {
  attributes: { kind: string; value?: number | [number, number, number] | null }[];
}

function mix(c: number, target: number, amount: number): number {
  return Math.round((c + (target - c) * amount) * 255);
}

function rgb([r, g, b]: [number, number, number]): string {
  return `rgb(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)})`;
}

/** BaseColor of the first material that has a color value, else null. */
export function baseColorOf(materials: SphereMaterial[]): [number, number, number] | null {
  for (const m of materials) {
    for (const a of m.attributes) {
      if (a.kind === "BaseColor" && Array.isArray(a.value)) return a.value;
    }
  }
  return null;
}

/**
 * Material placeholder sphere (STUDIO_DARK §4):
 * radial-gradient(circle at 34% 28%, hi, base 40%, lo 80%).
 */
export function sphereStyle(materials: SphereMaterial[], size: string): CSSProperties {
  const base = baseColorOf(materials) ?? [0.43, 0.43, 0.45];
  const hi: [number, number, number] = [
    mix(base[0], 1, 0.45) / 255,
    mix(base[1], 1, 0.45) / 255,
    mix(base[2], 1, 0.45) / 255,
  ];
  const lo: [number, number, number] = [
    mix(base[0], 0, 0.55) / 255,
    mix(base[1], 0, 0.55) / 255,
    mix(base[2], 0, 0.55) / 255,
  ];
  return {
    display: "block",
    width: size,
    aspectRatio: "1 / 1",
    borderRadius: "50%",
    background: `radial-gradient(circle at 34% 28%, ${rgb(hi)} 0%, ${rgb(base)} 40%, ${rgb(lo)} 80%)`,
    boxShadow: "0 22px 30px -14px rgba(0,0,0,.9)",
  };
}

export function pad2(n: number): string {
  return String(n).padStart(2, "0");
}
