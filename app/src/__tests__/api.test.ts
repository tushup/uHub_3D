import { expect, test, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `http://asset.localhost/${path}`,
  invoke: vi.fn(),
}));

import { thumbUrl, type AssetEntry } from "../api";

function entry(thumbnail: string | null): AssetEntry {
  return {
    dir: "/lib/3D/Furniture/Old_Chair",
    asset: {
      schema: 1,
      id: "asset-42",
      type: "model",
      name: "Old_Chair",
      category: "Furniture",
      tags: [],
      created: "2026-01-01T00:00:00Z",
      thumbnail,
      model: null,
      materials: [],
      hdri: null,
    },
  };
}

test("thumbUrl appends the asset id as cache buster", () => {
  const url = thumbUrl(entry("thumbnail.png"));
  expect(url).toBe("http://asset.localhost//lib/3D/Furniture/Old_Chair/thumbnail.png?v=asset-42");
});

test("thumbUrl is null without a thumbnail", () => {
  expect(thumbUrl(entry(null))).toBeNull();
});
