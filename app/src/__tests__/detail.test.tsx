import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import * as api from "../api";
import type { AssetEntry } from "../api";
import DetailPanel from "../components/DetailPanel";
import { useStore } from "../store";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://localhost/${path}`,
}));

vi.mock("../api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../api")>();
  return {
    ...orig,
    deleteAsset: vi.fn(() => Promise.resolve()),
    revealAsset: vi.fn(() => Promise.resolve()),
    listAssets: vi.fn(() => Promise.resolve([])),
  };
});

const modelEntry: AssetEntry = {
  dir: "/lib/3D/Furniture/Chair",
  asset: {
    schema: 1,
    id: "1",
    type: "model",
    name: "Chair",
    category: "Furniture",
    tags: ["wood", "interior"],
    created: "2026-01-01T00:00:00Z",
    thumbnail: null,
    model: { file: "Chair.fbx", format: "fbx", meshes: 3, vertices: 5120 },
    materials: [
      {
        originalName: "lambert3",
        name: "Seat",
        attributes: [
          { kind: "BaseColor", texture: "Chair_Seat_BaseColor.png" },
          { kind: "Roughness", value: 0.5, texture: null },
          { kind: "Normal", texture: "Chair_Seat_Normal.png", strength: 1, normalY: "OpenGL" },
        ],
      },
    ],
    hdri: null,
  },
};

const hdriEntry: AssetEntry = {
  dir: "/lib/HDRI/Sky/Noon",
  asset: {
    schema: 1,
    id: "2",
    type: "hdri",
    name: "Noon",
    category: "Sky",
    tags: [],
    created: "2026-01-01T00:00:00Z",
    thumbnail: null,
    model: null,
    materials: [],
    hdri: { file: "Noon.hdr", width: 4096, height: 2048, rotation: 45, intensity: 1.5 },
  },
};

beforeEach(() => {
  vi.clearAllMocks();
  useStore.setState({
    tab: "model",
    category: null,
    search: "",
    selectedDir: modelEntry.dir,
    assets: [modelEntry, hdriEntry],
    connections: [],
    toasts: [],
  });
});

test("model details show file, counts, materials with chips", () => {
  render(<DetailPanel />);
  expect(screen.getByText("Chair")).toBeTruthy();
  expect(screen.getByText("Chair.fbx")).toBeTruthy();
  expect(screen.getByText("5,120")).toBeTruthy();
  expect(screen.getByText("Seat")).toBeTruthy();
  expect(screen.getByText("BC")).toBeTruthy();
  expect(screen.getByText("N")).toBeTruthy();
});

test("hdri detail header is 2 to 1", () => {
  const e: AssetEntry = { ...hdriEntry, asset: { ...hdriEntry.asset, thumbnail: "thumbnail.png" } };
  useStore.setState({ assets: [modelEntry, e], selectedDir: e.dir, tab: "hdri" });
  const { container } = render(<DetailPanel />);
  const img = container.querySelector("img");
  expect(img).toBeTruthy();
  expect(img?.className).toContain("object-cover");
  expect(img?.closest("div.relative")?.className).toContain("aspect-[2/1]");
});

test("model detail thumbnail uses contain", () => {
  const e: AssetEntry = {
    ...modelEntry,
    asset: { ...modelEntry.asset, thumbnail: "thumbnail.png" },
  };
  useStore.setState({ assets: [e, hdriEntry], selectedDir: e.dir, tab: "model" });
  const { container } = render(<DetailPanel />);
  const img = container.querySelector("img");
  expect(img).toBeTruthy();
  expect(img?.className).toContain("object-contain");
  expect(img?.closest("div.relative")?.className).toContain("aspect-square");
});

test("hdri details show resolution, rotation, intensity", () => {
  useStore.setState({ selectedDir: hdriEntry.dir, tab: "hdri" });
  render(<DetailPanel />);
  expect(screen.getByText(/4096 × 2048/)).toBeTruthy();
  expect(screen.getByText(/45/)).toBeTruthy();
});

test("delete asks confirmation before calling api", async () => {
  render(<DetailPanel />);
  fireEvent.click(screen.getByRole("button", { name: "Delete" }));
  expect(screen.getByText(/Move 'Chair' to trash\?/)).toBeTruthy();
  expect(api.deleteAsset).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Move to trash" }));
  await waitFor(() => expect(api.deleteAsset).toHaveBeenCalledWith(modelEntry.dir));
  await waitFor(() => expect(useStore.getState().selectedDir).toBeNull());
});

test("export dock placeholder is shown", () => {
  // Task 11b replaced the SOON ghost dock with the live export dock.
  useStore.setState({ connections: [], bridgeStatus: { state: "online", port: 47913 } });
  render(<DetailPanel />);
  expect(screen.getByText("Export to DCC")).toBeTruthy();
  expect(screen.queryByText("SOON")).toBeNull();
  expect(screen.getByText("Blender")).toBeTruthy();
  expect(
    screen.getByText("Open Blender, Maya or Unreal with the uHub3D plugin enabled."),
  ).toBeTruthy();
});
