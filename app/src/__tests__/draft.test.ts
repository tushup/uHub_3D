import { expect, test, vi } from "vitest";

import type { AssetEntry, AttrKind, ParsedModel } from "../api";
import * as api from "../api";
import {
  draftReducer,
  initDraft,
  plannedSave,
  sanitizeName,
  validate,
} from "../components/newasset/draft";

vi.mock("../api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../api")>();
  return { ...orig, planSave: vi.fn() };
});

test("sanitizeName matches backend", () => {
  expect(sanitizeName(" Old Chair! ")).toBe("Old_Chair");
  expect(sanitizeName("!!")).toBe("");
});

test("initDraft material starts with one material, rotation 0, intensity 1", () => {
  const s = initDraft("material");
  expect(s.materials).toHaveLength(1);
  expect(s.rotation).toBe(0);
  expect(s.intensity).toBe(1);
});

test("setModel fills materials from parse with prefilled textures", () => {
  const parsed: ParsedModel = {
    format: "fbx",
    meshes: 1,
    vertices: 8,
    materials: [
      { name: "Red", textures: [["BaseColor", "/tmp/red.png"]] },
      { name: "Blue", textures: [] },
    ],
  };
  const s = draftReducer(initDraft("model"), { type: "setModel", path: "/m/cube.fbx", parsed });
  expect(s.modelSrc).toBe("/m/cube.fbx");
  expect(s.materials).toHaveLength(2);
  expect(s.materials[0].originalName).toBe("Red");
  expect(s.materials[0].name).toBe("Red");
  expect(s.materials[0].attributes[0].kind).toBe("BaseColor");
  expect(s.materials[0].attributes[0].textureSrc).toBe("/tmp/red.png");
});

function addAttr(kind: AttrKind) {
  const withMat = initDraft("material");
  const s = draftReducer(withMat, { type: "addAttribute", index: 0, kind });
  return s.materials[0].attributes[s.materials[0].attributes.length - 1];
}

test("addAttribute uses defaults", () => {
  expect(addAttr("Metallic").value).toBe(0);
  expect(addAttr("Opacity").value).toBe(1);
  expect(addAttr("Roughness").value).toBe(0.5);
  expect(addAttr("BaseColor").value).toEqual([0.8, 0.8, 0.8]);
  const normal = addAttr("Normal");
  expect(normal.normalY).toBe("OpenGL");
  expect(normal.strength).toBe(1);
  expect(addAttr("Height").strength).toBe(0.1);
  const emissive = addAttr("Emissive");
  expect(emissive.value).toEqual([0, 0, 0]);
  expect(emissive.strength).toBe(1);
});

function existingEntry(): AssetEntry {
  return {
    dir: "/lib/3D/Furniture/Chair",
    asset: {
      schema: 1,
      id: "1",
      type: "model",
      name: "Chair",
      category: "Furniture",
      tags: [],
      created: "2026-01-01T00:00:00Z",
      thumbnail: null,
      model: null,
      materials: [],
      hdri: null,
    },
  };
}

test("plannedSave returns the backend plan for a 2-material FBX draft", async () => {
  let s = initDraft("model");
  s = draftReducer(s, { type: "setName", name: "Old Chair" });
  s = draftReducer(s, { type: "setCategory", category: "Furniture" });
  s = draftReducer(s, {
    type: "setModel",
    path: "/m/chair.fbx",
    parsed: {
      format: "fbx",
      meshes: 3,
      vertices: 5120,
      materials: [
        {
          name: "lambert3",
          textures: [
            ["BaseColor", "/t/wood.png"],
            ["Roughness", "/t/rough.PNG"],
            ["Normal", "/t/nrm.jpg"],
          ],
        },
        { name: "lambert4", textures: [] },
      ],
    },
  });
  s = draftReducer(s, { type: "renameMaterial", index: 0, name: "Seat" });
  s = draftReducer(s, { type: "renameMaterial", index: 1, name: "Legs" });
  s = draftReducer(s, { type: "setThumbnail", path: "/t/thumb.png", preview: "data:x" });
  // File naming lives in the Rust backend now; the mock returns the
  // plan `Library::validate` computes for this draft (asserted in
  // the Rust `plan_save_matches_save` test).
  vi.mocked(api.planSave).mockReset();
  vi.mocked(api.planSave).mockResolvedValue({
    dir: "/lib/3D/Furniture/Old_Chair",
    dirName: "Old_Chair",
    relDir: "Database/3D/Furniture/Old_Chair/",
    files: [
      "Old_Chair.fbx",
      "Old_Chair_Seat_BaseColor.png",
      "Old_Chair_Seat_Roughness.png",
      "Old_Chair_Seat_Normal.jpg",
      "thumbnail.png",
      "asset.json",
    ],
  });
  const out = await plannedSave(s);
  expect(out.dir).toBe("/lib/3D/Furniture/Old_Chair");
  expect(out.files).toEqual([
    "Old_Chair.fbx",
    "Old_Chair_Seat_BaseColor.png",
    "Old_Chair_Seat_Roughness.png",
    "Old_Chair_Seat_Normal.jpg",
    "thumbnail.png",
    "asset.json",
  ]);
  // And the draft handed to the backend carries the naming inputs.
  const sent = vi.mocked(api.planSave).mock.calls[0][0];
  expect(sent.name).toBe("Old Chair");
  expect(sent.modelSrc).toBe("/m/chair.fbx");
  expect(sent.materials.map((m) => m.name)).toEqual(["Seat", "Legs"]);
  expect(sent.thumbnailSrc).toBe("/t/thumb.png");
});

test("plannedSave passes duplicate material names to the backend", async () => {
  let s = initDraft("model");
  s = draftReducer(s, { type: "setName", name: "Chair" });
  s = draftReducer(s, { type: "setCategory", category: "Furniture" });
  s = draftReducer(s, {
    type: "setModel",
    path: "/m/chair.obj",
    parsed: {
      format: "obj",
      meshes: 1,
      vertices: 8,
      materials: [
        { name: "Wood", textures: [["BaseColor", "/t/a.png"]] },
        { name: "Wood", textures: [["BaseColor", "/t/b.png"]] },
      ],
    },
  });
  vi.mocked(api.planSave).mockReset();
  vi.mocked(api.planSave).mockResolvedValue({
    dir: "/lib/3D/Furniture/Chair",
    dirName: "Chair",
    relDir: "Database/3D/Furniture/Chair/",
    files: [
      "Chair.obj",
      "Chair.mtl",
      "Chair_Wood_BaseColor.png",
      "Chair_Wood_2_BaseColor.png",
      "thumbnail.png",
      "asset.json",
    ],
  });
  const out = await plannedSave(s);
  expect(out.files).toEqual([
    "Chair.obj",
    "Chair.mtl",
    "Chair_Wood_BaseColor.png",
    "Chair_Wood_2_BaseColor.png",
    "thumbnail.png",
    "asset.json",
  ]);
  const sent = vi.mocked(api.planSave).mock.calls[0][0];
  expect(sent.materials.map((m) => m.name)).toEqual(["Wood", "Wood"]);
});

test("validate flags duplicate name in same category only", () => {
  let s = initDraft("model");
  s = draftReducer(s, { type: "setName", name: "Chair" });
  s = draftReducer(s, { type: "setCategory", category: "Furniture" });
  s = draftReducer(s, {
    type: "setModel",
    path: "/m/cube.fbx",
    parsed: { format: "fbx", meshes: 1, vertices: 8, materials: [] },
  });
  expect(validate(s, [existingEntry()]).name).toContain("already exists");
  const other = draftReducer(s, { type: "setCategory", category: "Rocks" });
  expect(validate(other, [existingEntry()]).name).toBeUndefined();
});

test("validate requires name, category and model file", () => {
  const errors = validate(initDraft("model"), []);
  expect(errors.name).toBeTruthy();
  expect(errors.category).toBeTruthy();
  expect(errors.model).toBeTruthy();
});
