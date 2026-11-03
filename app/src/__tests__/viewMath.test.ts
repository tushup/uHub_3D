import { expect, test } from "vitest";

import {
  applyDrag,
  applyWheel,
  framing,
  project,
  type Bounds,
  type View,
} from "../viewer/viewMath";
import goldenJson from "../viewer/view_golden.json";

interface GoldenCase {
  bounds: Bounds;
  view: View;
  size: number;
  points: [number, number][];
}

const golden = goldenJson as { cases: GoldenCase[] };

function corners(b: Bounds): [number, number, number][] {
  return Array.from({ length: 8 }, (_, i) => [
    i & 1 ? b.max[0] : b.min[0],
    i & 2 ? b.max[1] : b.min[1],
    i & 4 ? b.max[2] : b.min[2],
  ]);
}

test("TS projections match the shared golden file", () => {
  expect(golden.cases.length).toBe(6);
  golden.cases.forEach((c, ci) => {
    const f = framing(c.bounds, c.view);
    corners(c.bounds).forEach((corner, pi) => {
      const got = project(f, corner, c.size);
      const want = c.points[pi];
      expect(Math.abs(got[0] - want[0]), `case ${ci} point ${pi} x`).toBeLessThanOrEqual(1e-3);
      expect(Math.abs(got[1] - want[1]), `case ${ci} point ${pi} y`).toBeLessThanOrEqual(1e-3);
    });
  });
});

test("drag and wheel clamp like the Rust side", () => {
  const dragged = applyDrag({ yaw: -30, pitch: 15, zoom: 1 }, 100, 100000);
  expect(dragged.yaw).toBeCloseTo(10);
  expect(dragged.pitch).toBe(89);
  expect(applyWheel({ yaw: 0, pitch: 0, zoom: 1 }, -100000).zoom).toBe(4);
  expect(applyWheel({ yaw: 0, pitch: 0, zoom: 1 }, 100000).zoom).toBe(0.5);
});
