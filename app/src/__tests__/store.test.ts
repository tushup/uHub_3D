import { describe, expect, test } from "vitest";

import type { AssetEntry, AssetType } from "../api";
import { categoriesFor, useStore, visibleAssets } from "../store";

function entry(type: AssetType, category: string, name: string, tags: string[]): AssetEntry {
  return {
    dir: `/lib/${category}/${name}`,
    asset: {
      schema: 1,
      id: name,
      type,
      name,
      category,
      tags,
      created: "2026-01-01T00:00:00Z",
      thumbnail: null,
      model: null,
      materials: [],
      hdri: null,
    },
  };
}

function mkState(entries: AssetEntry[]) {
  return { tab: "model" as AssetType, category: null as string | null, search: "", assets: entries };
}

function names(entries: AssetEntry[]): string[] {
  return entries.map((e) => e.asset.name);
}

describe("selectors", () => {
  const assets = [
    entry("model", "Furniture", "Chair", ["wood"]),
    entry("model", "Rocks", "Boulder", []),
    entry("hdri", "Sky", "Noon", []),
  ];

  test("visibleAssets filters tab, category, search on tags", () => {
    const s = mkState(assets);
    expect(names(visibleAssets({ ...s, tab: "model" }))).toEqual(["Boulder", "Chair"]);
    expect(names(visibleAssets({ ...s, tab: "model", category: "Furniture" }))).toEqual(["Chair"]);
    expect(names(visibleAssets({ ...s, tab: "model", search: "WOO" }))).toEqual(["Chair"]);
  });

  test("categoriesFor counts per tab", () => {
    expect(categoriesFor(assets, "model")).toEqual([
      { name: "Furniture", count: 1 },
      { name: "Rocks", count: 1 },
    ]);
  });
});

describe("store actions", () => {
  test("setTab clears category and selection", () => {
    useStore.setState({ tab: "model", category: "Furniture", selectedDir: "/lib/Furniture/Chair" });
    useStore.getState().setTab("hdri");
    const s = useStore.getState();
    expect(s.tab).toBe("hdri");
    expect(s.category).toBeNull();
    expect(s.selectedDir).toBeNull();
  });
});
