// Shared view math (spec Amendment A1 §A1.3). Mirrors
// `src-tauri/src/view.rs`; both are checked against `view_golden.json`.

export interface View {
  yaw: number;
  pitch: number;
  zoom: number;
}

export const DEFAULT_VIEW: View = { yaw: -30, pitch: 15, zoom: 1 };

export interface Bounds {
  min: [number, number, number];
  max: [number, number, number];
}

export interface Framing {
  model: [[number, number, number, number], [number, number, number, number], [number, number, number, number], [number, number, number, number]];
  distance: number;
  fovYDeg: number;
}

export function clampedView(v: View): View {
  return {
    yaw: v.yaw,
    pitch: Math.min(89, Math.max(-89, v.pitch)),
    zoom: Math.min(4, Math.max(0.5, v.zoom)),
  };
}

export function applyDrag(v: View, dx: number, dy: number): View {
  return clampedView({ yaw: v.yaw + dx * 0.4, pitch: v.pitch + dy * 0.4, zoom: v.zoom });
}

export function applyWheel(v: View, deltaY: number): View {
  return clampedView({ ...v, zoom: v.zoom * Math.pow(1.1, -deltaY / 100) });
}

type Mat4 = number[][];

function matMul(a: Mat4, b: Mat4): Mat4 {
  const out: Mat4 = Array.from({ length: 4 }, () => [0, 0, 0, 0]);
  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      let sum = 0;
      for (let k = 0; k < 4; k++) sum += a[r][k] * b[k][c];
      out[r][c] = sum;
    }
  }
  return out;
}

function rotX(deg: number): Mat4 {
  const rad = (deg * Math.PI) / 180;
  const s = Math.sin(rad);
  const c = Math.cos(rad);
  return [
    [1, 0, 0, 0],
    [0, c, -s, 0],
    [0, s, c, 0],
    [0, 0, 0, 1],
  ];
}

function rotY(deg: number): Mat4 {
  const rad = (deg * Math.PI) / 180;
  const s = Math.sin(rad);
  const c = Math.cos(rad);
  return [
    [c, 0, s, 0],
    [0, 1, 0, 0],
    [-s, 0, c, 0],
    [0, 0, 0, 1],
  ];
}

export function framing(bounds: Bounds, view: View): Framing {
  const center = [0, 1, 2].map((i) => (bounds.min[i] + bounds.max[i]) / 2);
  const extent = [0, 1, 2].map((i) => bounds.max[i] - bounds.min[i]);
  const maxExtent = Math.max(extent[0], extent[1], extent[2]);
  const s = maxExtent === 0 ? 1 : 1 / maxExtent;
  const len = Math.sqrt(extent[0] ** 2 + extent[1] ** 2 + extent[2] ** 2);
  const r = (s * len) / 2;
  const d0 = (1.08 * r) / Math.sin((15 * Math.PI) / 180);
  const distance = d0 / view.zoom;

  const translate: Mat4 = [
    [1, 0, 0, -center[0]],
    [0, 1, 0, -center[1]],
    [0, 0, 1, -center[2]],
    [0, 0, 0, 1],
  ];
  const scale: Mat4 = [
    [s, 0, 0, 0],
    [0, s, 0, 0],
    [0, 0, s, 0],
    [0, 0, 0, 1],
  ];
  // M = Rx(pitch) · Ry(yaw) · S(s) · T(−c)
  const model = matMul(rotX(view.pitch), matMul(rotY(view.yaw), matMul(scale, translate)));
  return {
    model: model as Framing["model"],
    distance,
    fovYDeg: 30,
  };
}

/** Project a model-space point to pixel coordinates (square image). */
export function project(f: Framing, p: [number, number, number], size: number): [number, number] {
  const m = f.model;
  const q = [
    m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
    m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
    m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
  ];
  const zc = q[2] - f.distance;
  const tanHalf = Math.tan(((f.fovYDeg / 2) * Math.PI) / 180);
  const xNdc = q[0] / (-zc * tanHalf);
  const yNdc = q[1] / (-zc * tanHalf);
  return [((xNdc + 1) / 2) * size, ((1 - yNdc) / 2) * size];
}
