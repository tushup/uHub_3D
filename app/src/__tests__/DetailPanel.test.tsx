import { render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

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
    tags: [],
    created: "2026-01-01T00:00:00Z",
    thumbnail: null,
    model: { file: "Chair.fbx", format: "fbx", meshes: 3, vertices: 5120 },
    materials: [
      {
        originalName: "lambert3",
        name: "Seat",
        attributes: [{ kind: "BaseColor", texture: "Chair_Seat_BaseColor.png" }],
      },
    ],
    hdri: null,
  },
};

beforeEach(() => {
  vi.clearAllMocks();
  useStore.setState({
    tab: "model",
    category: null,
    search: "",
    selectedDir: modelEntry.dir,
    assets: [modelEntry],
    connections: [],
    toasts: [],
  });
});

test("detail_actions_outside_scroll_area", () => {
  const { container } = render(<DetailPanel />);
  const openFolder = screen.getByRole("button", { name: /Open folder/ });
  const deleteButton = screen.getByRole("button", { name: "Delete" });
  const scrollArea = container.querySelector(".overflow-y-auto");
  expect(scrollArea).toBeTruthy();
  expect(scrollArea?.contains(openFolder)).toBe(false);
  expect(scrollArea?.contains(deleteButton)).toBe(false);
  // Both actions sit directly above Export to DCC in the fixed footer.
  const exportLabel = screen.getByText("Export to DCC");
  expect(
    openFolder.compareDocumentPosition(exportLabel) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(
    deleteButton.compareDocumentPosition(exportLabel) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
});
