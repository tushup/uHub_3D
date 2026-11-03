import { expect, test } from "vitest";

import { parsePreviewBytes } from "../api";

function buildBytes(): ArrayBuffer {
  const positions = [0, 0, 0, 1, 0, 0, 0, 1, 0];
  const normals = [0, 0, 1, 0, 0, 1, 0, 0, 1];
  const uvs = [0, 0, 1, 0, 0, 1];
  const indices = [0, 1, 2];
  const groupName = new TextEncoder().encode("Red");
  const total = 16 + 9 * 4 + 9 * 4 + 6 * 4 + 3 * 4 + 8 + 2 + groupName.length;
  const buf = new ArrayBuffer(total);
  const v = new DataView(buf);
  let off = 0;
  const u32 = (n: number) => {
    v.setUint32(off, n, true);
    off += 4;
  };
  u32(3);
  u32(3);
  u32(1);
  u32(1);
  for (const f of [...positions, ...normals, ...uvs]) {
    v.setFloat32(off, f, true);
    off += 4;
  }
  for (const i of indices) u32(i);
  u32(0);
  u32(3);
  v.setUint16(off, groupName.length, true);
  off += 2;
  new Uint8Array(buf, off).set(groupName);
  return buf;
}

test("parsePreviewBytes round-trips the Rust layout", () => {
  const mesh = parsePreviewBytes(buildBytes());
  expect(Array.from(mesh.positions)).toEqual([0, 0, 0, 1, 0, 0, 0, 1, 0]);
  expect(Array.from(mesh.normals)).toEqual([0, 0, 1, 0, 0, 1, 0, 0, 1]);
  expect(Array.from(mesh.uvs)).toEqual([0, 0, 1, 0, 0, 1]);
  expect(Array.from(mesh.indices)).toEqual([0, 1, 2]);
  expect(mesh.groups).toEqual([{ material: "Red", start: 0, count: 3 }]);
  expect(mesh.hasUvs).toBe(true);
});
