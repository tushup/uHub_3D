import { act, render } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://localhost/${path}`,
  invoke: vi.fn(() => Promise.resolve()),
}));

import * as api from "../api";
import type { AssetEntry } from "../api";
import App from "../App";
import { useStore } from "../store";

vi.mock("../api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../api")>();
  return {
    ...orig,
    listAssets: vi.fn(() => Promise.resolve([])),
    getSettings: vi.fn(() => Promise.resolve({ libraryRoot: "C:/Lib", port: 47913 })),
    onNewAsset: vi.fn(() => Promise.resolve(() => {})),
    onPreferences: vi.fn(() => Promise.resolve(() => {})),
  };
});

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.listAssets).mockResolvedValue([]);
  vi.mocked(api.getSettings).mockResolvedValue({ libraryRoot: "C:/Lib", port: 47913 });
});

const modelEntry: AssetEntry = {
  dir: "/lib/3D/Test/CircuitBoard",
  asset: {
    schema: 1,
    id: "1",
    type: "model",
    name: "CircuitBoard",
    category: "Test",
    tags: [],
    created: "2026-01-01T00:00:00Z",
    thumbnail: "thumbnail.png",
    model: { file: "CircuitBoard.fbx", format: "fbx", meshes: 1, vertices: 24 },
    materials: [],
    hdri: null,
  },
};

test("app_root_clips_to_viewport", async () => {
  vi.mocked(api.listAssets).mockResolvedValueOnce([modelEntry]);
  useStore.setState({
    assets: [modelEntry],
    loaded: true,
    selectedDir: modelEntry.dir,
    tab: "model",
    category: null,
    search: "",
  });
  const { container } = render(<App />);
  await act(async () => {});
  const root = container.firstElementChild;
  expect(root?.className).toContain("h-screen");
  expect(root?.className).toContain("overflow-hidden");
  const sidebarColumn = container.querySelector('nav[aria-label="Categories"]')?.parentElement;
  const mainColumn = container.querySelector("main");
  const detailColumn =
    container.querySelector('section[aria-label="Asset details"]')?.parentElement;
  for (const column of [sidebarColumn, mainColumn, detailColumn]) {
    expect(column, "grid column exists").toBeTruthy();
    expect(column?.className).toContain("min-w-0");
    expect(column?.className).toContain("min-h-0");
  }
});

test("topbar_tabs_not_absolute", async () => {
  const { container } = render(<App />);
  await act(async () => {});
  const header = container.querySelector("header");
  expect(header).toBeTruthy();
  expect(header?.className).toContain("grid");
  expect(header?.style.gridTemplateColumns).toBe("1fr auto 1fr");
  const tabGroup = container.querySelector('nav[aria-label="Asset type"]');
  expect(tabGroup).toBeTruthy();
  expect(tabGroup?.classList.contains("absolute")).toBe(false);
});

test("focus triggers rescan", async () => {
  useStore.setState({ assets: [], loaded: false, selectedDir: null });
  render(<App />);
  await act(async () => {});
  expect(api.listAssets).toHaveBeenCalledTimes(1);
  act(() => {
    window.dispatchEvent(new Event("focus"));
  });
  expect(api.listAssets).toHaveBeenCalledTimes(2);
});
