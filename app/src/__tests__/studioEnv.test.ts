import { describe, expect, test } from "vitest";

import { bakeStudioEnv } from "../viewer/scene";

const W = 256;
const H = 128;

/** Sample the baked equirect the way THREE's equirectUv maps a direction. */
function sample(data: Float32Array, dir: [number, number, number]): [number, number, number] {
  const len = Math.hypot(dir[0], dir[1], dir[2]);
  const [x, y, z] = [dir[0] / len, dir[1] / len, dir[2] / len];
  const u = Math.atan2(z, x) / (2 * Math.PI) + 0.5;
  const v = Math.asin(Math.min(1, Math.max(-1, y))) / Math.PI + 0.5;
  const tx = Math.min(W - 1, Math.floor(u * W));
  const ty = Math.min(H - 1, Math.floor(v * H));
  const i = (ty * W + tx) * 4;
  return [data[i], data[i + 1], data[i + 2]];
}

describe("bakeStudioEnv", () => {
  const env = bakeStudioEnv(W, H);

  test("env_down_is_floor_radiance", () => {
    const rgb = sample(env, [0, -1, 0]);
    for (const v of rgb) expect(v).toBeCloseTo(0.02, 3);
  });

  test("env_up_is_sky_radiance", () => {
    const rgb = sample(env, [0, 1, 0]);
    for (const v of rgb) expect(v).toBeCloseTo(0.2, 3);
  });

  test("env_key_direction_hits_key", () => {
    // Key light: centre (-2.0, 2.0, 2.4), side 1.0, E 3.0,
    // colour (1.00, 0.96, 0.90). L = E * d^2 / side^2.
    const d2 = 4.0 + 4.0 + 5.76;
    const want: [number, number, number] = [3.0 * d2, 3.0 * d2 * 0.96, 3.0 * d2 * 0.9];
    const rgb = sample(env, [-2.0, 2.0, 2.4]);
    for (let i = 0; i < 3; i++) {
      expect(Math.abs(rgb[i] - want[i]) / want[i]).toBeLessThan(0.01);
    }
  });
});
